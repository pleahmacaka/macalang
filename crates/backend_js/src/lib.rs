use maca_parser::ast::*;
use std::collections::{BTreeMap, BTreeSet};

pub struct JsOut {
    pub js: String,
    pub html: String,
    pub css: String,
    /// Names of top-level functions transpiled to callable JS (for `import`).
    pub exports: Vec<String>,
}

pub fn emit(m: &Module) -> JsOut {
    let mut cx = Cx::default();

    let mut computed: Vec<(String, Expr)> = Vec::new();
    for item in &m.items {
        if let Stmt::Bind(b) = item
            && let Expr::Ident(name) = &b.target
            && sum_variants(&b.value).is_none()
            && !is_record_type(&b.value)
        {
            match js_init(&b.value) {
                Some(literal) => cx.state.push((name.clone(), literal)),
                None => {
                    cx.state.push((name.clone(), "undefined".into()));
                    computed.push((name.clone(), b.value.clone()));
                }
            }
            if b.is_const {
                cx.consts.insert(name.clone());
            }
        }
    }
    let state_names: BTreeSet<String> = cx.state.iter().map(|(n, _)| n.clone()).collect();
    cx.state_names = state_names.clone();
    set_state_names(&state_names);
    set_consts(&cx.consts);

    let variants = collect_variants(m);
    set_variants(&variants);
    set_fns(m);

    for item in &m.items {
        collect_class_strings(item, &mut cx.classes);
    }

    SCOPES.with(|s| s.borrow_mut().push(BTreeSet::new()));
    push_frame(Frame::default());
    let state_init = computed
        .iter()
        .map(|(name, value)| format!("\x20 state.{name} = {};\n", jexpr(value)))
        .collect::<String>();
    pop_frame();
    SCOPES.with(|s| {
        s.borrow_mut().pop();
    });

    let main_fn = m.items.iter().find_map(|it| match it {
        Stmt::Fn(f) if f.name == "main" => Some(f),
        _ => None,
    });
    let main_view = main_fn.filter(|f| is_element_ty(&f.ret));

    SCOPES.with(|s| s.borrow_mut().push(BTreeSet::new()));
    push_frame(main_view.map(view_frame).unwrap_or_default());
    let mut build_body = String::new();
    let root_expr = match main_fn.and_then(|f| f.body.as_ref()) {
        Some(FnBody::Expr(e)) => Some(e.as_ref().clone()),
        Some(FnBody::Block(stmts)) => {
            let root = stmts.iter().rposition(|s| matches!(s, Stmt::Expr(_)));
            if main_view.is_some() {
                let before = root.map(|i| &stmts[..i]).unwrap_or(stmts);
                build_body.push_str(&jblock_sink(before, &Sink::Discard));
            }
            root.map(|i| match &stmts[i] {
                Stmt::Expr(e) => e.clone(),
                _ => Expr::Unit,
            })
        }
        None => None,
    };

    let root_var = match &root_expr {
        Some(e) => cx.element(e, &mut build_body),
        None => {
            build_body.push_str("  const n0 = document.createElement(\"div\");\n");
            "n0".into()
        }
    };
    pop_frame();
    SCOPES.with(|s| {
        s.borrow_mut().pop();
    });

    let mut js = cx.finish(&build_body, &root_var, &state_init);

    let mut exports = Vec::new();
    let mut fn_defs = String::new();
    let mut host_stubs = String::new();
    let mut hosts = Vec::new();
    for item in &m.items {
        if let Stmt::Fn(f) = item {
            if f.name == "main" {
                continue;
            }
            if f.body.is_none() {
                host_stubs.push_str(&host_stub(f));
                hosts.push(f.name.clone());
                continue;
            }
            fn_defs.push_str(&emit_fn(f));
            fn_defs.push('\n');
            exports.push(f.name.clone());
        }
    }
    exports.extend(variants.keys().cloned());
    if !host_stubs.is_empty() {
        js.push_str("\n// ---- host functions (declared here, provided by the host) ----\n");
        js.push_str(&host_stubs);
    }
    if !fn_defs.is_empty() || !variants.is_empty() {
        if !fn_defs.is_empty() {
            js.push_str("\n// ---- transpiled functions ----\n");
            js.push_str(&fn_defs);
        }
        let names = exports.join(", ");
        js.push_str(&format!(
            "if (typeof module !== \"undefined\") Object.assign(module.exports, {{ {names} }});\n"
        ));
    }

    let mut css = cx.css();
    let mut foreign_js = String::new();
    for item in &m.items {
        if let Stmt::Import(Import::Foreign { lang, spec }) = item {
            match lang.as_str() {
                "js" => {
                    foreign_js.push_str("// ---- embedded (import js) ----\n");
                    foreign_js.push_str(spec);
                    foreign_js.push('\n');
                }
                "css" => {
                    css.push_str("\n/* ---- embedded (import css) ---- */\n");
                    css.push_str(spec);
                    css.push('\n');
                }
                _ => {}
            }
        }
    }
    let bridge = cx.bridge(&hosts);
    let js = if foreign_js.is_empty() {
        format!("{bridge}{js}")
    } else {
        format!("{bridge}\n{foreign_js}\n{js}")
    };
    let js = match variant_ctors(&variants) {
        c if c.is_empty() => js,
        c => insert_after_use_strict(&js, &format!("// ---- sum variants ----\n{c}")),
    };
    let js = match method_helpers_for(&js) {
        h if h.is_empty() => js,
        h => insert_after_use_strict(&js, &format!("// ---- method helpers ----\n{h}")),
    };
    let html = HTML.into();
    JsOut {
        js,
        html,
        css,
        exports,
    }
}

/// Splice `block` in just below the `"use strict"` directive, or at the very top when there is none.
fn insert_after_use_strict(js: &str, block: &str) -> String {
    match js.split_once('\n') {
        Some((first, rest)) if first.trim_start().starts_with("\"use strict\"") => {
            format!("{first}\n{block}\n{rest}")
        }
        _ => format!("{block}\n{js}"),
    }
}

/// The reactive core: `state` is a proxy, so writing a declared name marks it dirty and the bound nodes that read it run again.
const REACTIVE_JS: &str = r#"const _binds = [];
const _proxies = new WeakMap();
let _dirty = new Set();
let _depth = 0;
function _reactive(name, v) {
  if (v === null || typeof v !== "object") return v;
  if (!Array.isArray(v) && Object.getPrototypeOf(v) !== Object.prototype) return v;
  let p = _proxies.get(v);
  if (p === undefined) {
    p = new Proxy(v, {
      get(t, k) { return _reactive(name, Reflect.get(t, k)); },
      set(t, k, x) { Reflect.set(t, k, x); _touch(name); return true; },
      deleteProperty(t, k) { Reflect.deleteProperty(t, k); _touch(name); return true; },
    });
    _proxies.set(v, p);
  }
  return p;
}
const state = new Proxy(_state, {
  get(t, k) { return _reactive(k, Reflect.get(t, k)); },
  set(t, k, v) {
    const was = Reflect.get(t, k);
    Reflect.set(t, k, v);
    if (!Object.is(was, v)) _touch(k);
    return true;
  },
  deleteProperty(t, k) { Reflect.deleteProperty(t, k); _touch(k); return true; },
});
let _cells = 0;
// A local a nested definition writes: state the view instance owns. Each call
// makes a fresh one, so two instances of the same view share nothing.
function _cell(v) {
  const k = "@" + (_cells += 1);
  return new Proxy({ v, k }, {
    get(t, p) { return p === "v" ? _reactive(k, Reflect.get(t, p)) : Reflect.get(t, p); },
    set(t, p, x) {
      const was = Reflect.get(t, p);
      Reflect.set(t, p, x);
      if (!Object.is(was, x)) _touch(k);
      return true;
    },
  });
}
function _bind(deps, run) { _binds.push({ deps, run }); }
function _touch(name) {
  _dirty.add(name);
  if (_depth === 0) _flush();
}
// One turn: everything a handler assigns is collected, and the view is repainted
// once, when the handler returns.
function _turn(f) {
  return function () {
    _depth += 1;
    try {
      return f.apply(this, arguments);
    } finally {
      _depth -= 1;
      if (_depth === 0) _flush();
    }
  };
}
function _run(bs) {
  _depth += 1;
  try {
    for (const b of bs) b.run();
  } finally {
    _depth -= 1;
  }
}
function _flush() {
  for (let pass = 0; _dirty.size > 0; pass += 1) {
    if (pass === 8) {
      _dirty = new Set();
      throw new Error("maca: a bound node keeps writing state; a view reads state, it does not assign it");
    }
    const names = _dirty;
    _dirty = new Set();
    _run(_binds.filter((b) => b.deps === null || b.deps.some((n) => names.has(n))));
  }
}
function update() {
  _dirty = new Set();
  _run(_binds);
}
"#;

/// The bridge itself: the fixed half of what `Cx::bridge` emits, above the three generated name lists it reads (`_vars`, `_consts`, `_declared`).
const BRIDGE_JS: &str = r#"// The documented boundary between this program and any `import js` block:
//
//   maca.get(name)           read a name the program declared
//   maca.set(name, value)    write it, then refresh the view
//   maca.set({ a, b })       write several, refresh once
//   maca.refresh()           re-sync the bound nodes after something else moved
//   maca.provide({ f })      supply a function the Maca side declared
//
// Nothing else in this file is an interface.
const _hosts = Object.create(null);
function _declaredState() { return _vars.concat(_consts).join(", ") || "(none)"; }
function _readable(name) {
  if (_vars.indexOf(name) < 0 && _consts.indexOf(name) < 0) {
    throw new Error(`maca.get: \`${name}\` is not state in this program; declared: ${_declaredState()}`);
  }
}
function _writable(name) {
  if (_vars.indexOf(name) >= 0) return;
  if (_consts.indexOf(name) >= 0) {
    throw new Error(`maca.set: \`${name}\` is a constant`);
  }
  throw new Error(`maca.set: \`${name}\` is not state in this program; declared: ${_declaredState()}`);
}
const maca = {
  get(name) { _readable(name); return state[name]; },
  // Every name is checked before any is written: a typo in one field of a
  // `set({…})` must not leave the other fields half applied.
  set(name, value) {
    const pairs = typeof name === "string" ? [[name, value]] : Object.entries(name);
    for (const pair of pairs) _writable(pair[0]);
    _turn(() => { for (const pair of pairs) state[pair[0]] = pair[1]; })();
  },
  refresh() { update(); },
  // The other direction: hand back the functions Maca declared without a body.
  // They are ordinary calls over there, so this is what makes them resolve.
  provide(table) {
    for (const key of Object.keys(table)) {
      if (_declared.indexOf(key) < 0) {
        throw new Error(`maca.provide: \`${key}\` is not declared in this program; declared: ${_declared.join(", ") || "(none)"}`);
      }
      if (typeof table[key] !== "function") {
        throw new TypeError(`maca.provide: \`${key}\` must be a function`);
      }
      _hosts[key] = table[key];
    }
  },
};
// One call to a declared-but-not-defined function. A missing implementation
// names itself here, rather than reaching the view as `undefined is not a
// function` from wherever the value ended up being used.
function _host(name, args) {
  const f = _hosts[name];
  if (typeof f !== "function") {
    throw new Error(`maca: \`${name}\` is declared in Maca but nothing implements it; call maca.provide({ ${name}: … }) from the import js block`);
  }
  return f.apply(null, args);
}
"#;

/// A signature with no body → a stub that routes the call through the bridge.
fn host_stub(f: &FnDef) -> String {
    let params = f
        .params
        .iter()
        .map(|p| p.name.clone())
        .collect::<Vec<_>>()
        .join(", ");
    let name = &f.name;
    format!(
        "// declared in Maca as `{name}({params})`; \
         supply it with `maca.provide({{ {name} }})`\n\
         function {name}(...args) {{ return _host(\"{name}\", args); }}\n"
    )
}

/// How often a function binds each of its own names, and which names something other than that straight line writes.
#[derive(Default)]
struct Written {
    own: BTreeMap<String, usize>,
    nested: BTreeSet<String>,
}

impl Written {
    fn wrote(&mut self, n: &str, inside: bool) {
        match inside {
            true => {
                self.nested.insert(n.to_string());
            }
            false => *self.own.entry(n.to_string()).or_default() += 1,
        }
    }
}

/// What a function being emitted holds beyond ordinary locals: its instance's reactive cells, and the locals a bind may recompute.
#[derive(Default)]
struct Frame {
    cells: BTreeSet<String>,
    single: BTreeSet<String>,
}

/// Does this function hand back nodes, so its locals are a view instance's state rather than a calculation's scratch space?
fn builds_elements(f: &FnDef) -> bool {
    is_element_ty(&f.ret)
        || match &f.body {
            Some(FnBody::Expr(e)) => contributes_elements(e),
            Some(FnBody::Block(stmts)) => tail_contributes(stmts),
            None => false,
        }
}

/// What a view's body owns: the locals a nested definition writes, and the locals one binding computes and nothing rewrites.
fn view_frame(f: &FnDef) -> Frame {
    let Some(FnBody::Block(stmts)) = &f.body else {
        return Frame::default();
    };
    if !builds_elements(f) {
        return Frame::default();
    }
    let mut w = Written::default();
    scan_stmts(stmts, false, &mut w);
    for p in &f.params {
        w.own.entry(p.name.clone()).or_default();
    }
    Frame {
        cells: w
            .own
            .keys()
            .filter(|n| w.nested.contains(*n))
            .cloned()
            .collect(),
        single: w
            .own
            .iter()
            .filter(|(n, count)| **count == 1 && !w.nested.contains(*n))
            .map(|(n, _)| n.clone())
            .collect(),
    }
}

fn scan_stmts(stmts: &[Stmt], inside: bool, w: &mut Written) {
    for s in stmts {
        match s {
            Stmt::Fn(f) => match &f.body {
                Some(FnBody::Expr(e)) => scan_expr(e, true, w),
                Some(FnBody::Block(b)) => scan_stmts(b, true, w),
                None => {}
            },
            Stmt::Bind(b) => {
                if let Expr::Ident(n) = &b.target {
                    w.wrote(n, inside);
                }
                scan_expr(&b.value, inside, w);
            }
            Stmt::Expr(e) => scan_expr(e, inside, w),
            _ => {}
        }
    }
}

fn scan_expr(e: &Expr, inside: bool, w: &mut Written) {
    if let Expr::Assign { target, .. } = e
        && let Expr::Ident(n) = target.as_ref()
    {
        w.wrote(n, inside);
    }
    let mut es: Vec<&Expr> = Vec::new();
    let mut blocks: Vec<&[Stmt]> = Vec::new();
    let mut lambda = false;
    match e {
        Expr::Str(parts) => {
            for p in parts {
                if let StrPart::Interp(x) = p {
                    es.push(x);
                }
            }
        }
        Expr::List(xs) => es.extend(xs),
        Expr::Record(fields) | Expr::Ctor { fields, .. } => es.extend(field_values(fields)),
        Expr::With { base, fields } => {
            es.push(base);
            es.extend(field_values(fields));
        }
        Expr::Call { callee, args } => {
            es.push(callee);
            for a in args {
                if let Some(n) = two_way_target(a) {
                    w.nested.insert(n.to_string());
                }
                es.push(arg_expr(a));
            }
        }
        Expr::Field { base: x, .. } | Expr::Unary { expr: x, .. } => es.push(x),
        Expr::Try(x) | Expr::Fail(x) | Expr::Reify(x) | Expr::Await(x) | Expr::Spawn(x) => {
            es.push(x)
        }
        Expr::Return(x) => es.extend(x.as_deref()),
        Expr::Index { base: a, index: b } | Expr::Range { lo: a, hi: b } => {
            es.push(a);
            es.push(b);
        }
        Expr::Binary { lhs, rhs, .. } => {
            es.push(lhs);
            es.push(rhs);
        }
        Expr::Assign { target, value } => {
            es.push(target);
            es.push(value);
        }
        Expr::Ternary { cond, then, els } => {
            es.push(cond);
            es.push(then);
            es.push(els);
        }
        Expr::If { cond, then, els } => {
            es.push(cond);
            blocks.push(then);
            blocks.extend(els.as_deref());
        }
        Expr::Match { scrut, arms } => {
            es.push(scrut);
            for a in arms {
                es.extend(a.guard.as_ref());
                es.push(&a.body);
            }
        }
        Expr::While { cond, body } => {
            es.push(cond);
            blocks.push(body);
        }
        Expr::For { iter, body, .. } => {
            es.push(iter);
            blocks.push(body);
        }
        Expr::Block(stmts) => blocks.push(stmts),
        Expr::Lambda { body, .. } => {
            es.push(body);
            lambda = true;
        }
        _ => {}
    }
    let inside = inside || lambda;
    for x in es {
        scan_expr(x, inside, w);
    }
    for b in blocks {
        scan_stmts(b, inside, w);
    }
}

/// The name a `value=` argument writes when the user types, which is a write the view never spells out.
fn two_way_target(a: &Arg) -> Option<&str> {
    let value = match a {
        Arg::Named { name, value } if name == "value" => value,
        Arg::Directive {
            kind: Dir::Bind,
            prop,
            value,
        } if prop == "value" => value,
        _ => return None,
    };
    let target = match value {
        Expr::Lambda { body, .. } => match body.as_ref() {
            Expr::Assign { target, .. } => target.as_ref(),
            _ => return None,
        },
        e => e,
    };
    match target {
        Expr::Ident(n) => Some(n),
        _ => None,
    }
}

fn field_values(fields: &[Field]) -> Vec<&Expr> {
    fields
        .iter()
        .filter_map(|f| match f {
            Field::Value { value, .. } | Field::Bare(value) => Some(value),
            _ => None,
        })
        .collect()
}

/// Transpile one top-level function to a JS function declaration.
fn emit_fn(f: &FnDef) -> String {
    let params = f
        .params
        .iter()
        .map(|p| p.name.clone())
        .collect::<Vec<_>>()
        .join(", ");
    declare(&f.name);
    SCOPES.with(|s| {
        s.borrow_mut()
            .push(f.params.iter().map(|p| p.name.clone()).collect())
    });
    let frame = view_frame(f);
    let mut body = String::new();
    for p in &f.params {
        if frame.cells.contains(&p.name) {
            body.push_str(&format!("  var {0} = _cell({0});\n", p.name));
        }
    }
    push_frame(frame);
    body.push_str(&match &f.body {
        Some(FnBody::Expr(e)) => format!("  return {};", jexpr(e)),
        Some(FnBody::Block(stmts)) => jblock(stmts),
        None => String::new(),
    });
    pop_frame();
    SCOPES.with(|s| {
        s.borrow_mut().pop();
    });
    format!("function {}({params}) {{\n{body}\n}}", f.name)
}

/// Where the value of a statement-position expression has to end up.
#[derive(Clone)]
enum Sink {
    /// The value is not wanted: emit it as a bare statement.
    Discard,
    /// The value is the function's result.
    Return,
    /// The value is written to this JS lvalue.
    Assign(String),
}

/// A block of statements → JS, returning the value of the final expression.
fn jblock(stmts: &[Stmt]) -> String {
    jblock_ret(stmts, true)
}

/// `ret` = whether the final expression is `return`ed (function body) or emitted as a bare statement (loop body).
fn jblock_ret(stmts: &[Stmt], ret: bool) -> String {
    jblock_sink(stmts, &if ret { Sink::Return } else { Sink::Discard })
}

/// A block whose final expression goes to `sink`; everything before it is evaluated for its effect.
fn jblock_sink(stmts: &[Stmt], sink: &Sink) -> String {
    let mut out = String::new();
    for (i, s) in stmts.iter().enumerate() {
        let last = i + 1 == stmts.len();
        let here = if last { sink.clone() } else { Sink::Discard };
        match s {
            Stmt::Bind(b) => out.push_str(&jbind(b)),
            Stmt::Expr(e) => out.push_str(&jstmt(e, &here)),
            Stmt::Fn(f) => out.push_str(&emit_fn(f)),
            _ => {}
        }
    }
    out
}

/// A binding statement.
fn jbind(b: &Bind) -> String {
    match &b.target {
        Expr::Ident(n) => {
            let is_state = STATE.with(|s| s.borrow().contains(n));
            let fresh = !is_state && !declared(n);
            if fresh {
                declare(n);
            }
            if fresh && !is_state && derivable(n) && reads_state(&b.value) {
                let (value, watched) = (jexpr(&b.value), dep_list(&b.value));
                declare_cell(n);
                return format!(
                    "  var {n} = _cell({value});\n  _bind({watched}, () => {{ {n}.v = {value}; }});\n"
                );
            }
            if is_branching(&b.value) {
                let decl = match (fresh, is_cell(n)) {
                    (false, _) => String::new(),
                    (true, false) => format!("  var {n};\n"),
                    (true, true) => format!("  var {n} = _cell(null);\n"),
                };
                format!("{decl}{}", jstmt(&b.value, &Sink::Assign(jname(n))))
            } else if is_state || !fresh {
                format!("  {} = {};\n", jname(n), jexpr(&b.value))
            } else if is_cell(n) {
                format!("  var {n} = _cell({});\n", jexpr(&b.value))
            } else {
                format!("  var {n} = {};\n", jexpr(&b.value))
            }
        }
        t if is_branching(&b.value) => jstmt(&b.value, &Sink::Assign(jexpr(t))),
        t => format!("  {} = {};\n", jexpr(t), jexpr(&b.value)),
    }
}

/// Does this expression choose between blocks, so that lowering it as a value would put those blocks inside an IIFE?
fn is_branching(e: &Expr) -> bool {
    matches!(e, Expr::If { .. } | Expr::Match { .. } | Expr::Block(_))
}

/// Hand one already-lowered JS expression to its sink.
fn deliver(js: &str, sink: &Sink) -> String {
    match sink {
        Sink::Discard => format!("  {js};\n"),
        Sink::Return => format!("  return {js};\n"),
        Sink::Assign(t) => format!("  {t} = {js};\n"),
    }
}

/// One expression in statement position → JS statements.
fn jstmt(e: &Expr, sink: &Sink) -> String {
    match e {
        Expr::If { cond, then, els } => {
            let mut out = format!("  if ({}) {{\n{}  }}", jexpr(cond), jblock_sink(then, sink));
            match els {
                Some(b) => out.push_str(&format!(" else {{\n{}  }}\n", jblock_sink(b, sink))),
                None => out.push_str(&match sink {
                    Sink::Discard => "\n".to_string(),
                    s => format!(" else {{\n{}  }}\n", deliver("null", s)),
                }),
            }
            out
        }
        Expr::Match { scrut, arms } => jmatch_stmt(scrut, arms, sink),
        Expr::Block(stmts) => format!("  {{\n{}  }}\n", jblock_sink(stmts, sink)),
        Expr::While { cond, body } => format!(
            "  while ({}) {{\n{}  }}\n",
            jexpr(cond),
            jblock_sink(body, &Sink::Discard)
        ),
        Expr::For { pat, iter, body } => jfor(pat, iter, body),
        Expr::Break => "  break;\n".into(),
        Expr::Continue => "  continue;\n".into(),
        Expr::Return(v) => match v {
            Some(x) => format!("  return {};\n", jexpr(x)),
            None => "  return;\n".into(),
        },
        _ => deliver(&jexpr(e), sink),
    }
}

/// `for pat in iter { … }` → a `for … of` loop.
fn jfor(pat: &Pattern, iter: &Expr, body: &[Stmt]) -> String {
    let (name, binds) = match pat {
        Pattern::Bind(n) if !is_variant(n) => (n.clone(), String::new()),
        p => ("_it".to_string(), jpattern(p, "_it").1),
    };
    declare(&name);
    let mut out = format!("  for (var {name} of {}) {{\n", jexpr(iter));
    if !binds.is_empty() {
        out.push_str(&format!("  {binds}\n"));
    }
    out.push_str(&jblock_sink(body, &Sink::Discard));
    out.push_str("  }\n");
    out
}

/// `match` in statement position → a real `if`/`else if` chain.
fn jmatch_stmt(scrut: &Expr, arms: &[Arm], sink: &Sink) -> String {
    let sv = "_s$";
    let mut out = format!("  {{\n  const {sv} = {};\n", jexpr(scrut));
    let mut chained = false;
    for a in arms {
        let (cond, binds) = jpattern(&a.pat, sv);
        let (test, pre) = match &a.guard {
            Some(g) => (
                format!("{cond} && (() => {{ {binds}return {}; }})()", jexpr(g)),
                binds.clone(),
            ),
            None => (cond, binds),
        };
        let kw = if chained { "  } else if" } else { "  if" };
        chained = true;
        out.push_str(&format!("{kw} ({test}) {{\n"));
        if !pre.is_empty() {
            out.push_str(&format!("  {pre}\n"));
        }
        out.push_str(&jstmt(&a.body, sink));
    }
    let no_match = "  throw new Error(\"no match\");\n";
    if chained {
        out.push_str(&format!("  }} else {{\n{no_match}  }}\n"));
    } else {
        out.push_str(no_match);
    }
    out.push_str("  }\n");
    out
}

thread_local! {
    /// Top-level reactive-state names, consulted so a reference to `x` lowers to `state.x` everywhere (view text, attributes, and transpiled functions).
    static STATE: std::cell::RefCell<BTreeSet<String>> = const { std::cell::RefCell::new(BTreeSet::new()) };
    /// Sum-type variant names and their payload arity.
    static VARIANTS: std::cell::RefCell<BTreeMap<String, usize>> = const { std::cell::RefCell::new(BTreeMap::new()) };
    /// One frame per function being emitted, holding the names it has already declared, so a write from a nested definition assigns rather than shadows.
    static SCOPES: std::cell::RefCell<Vec<BTreeSet<String>>> = const { std::cell::RefCell::new(Vec::new()) };
    /// One frame per function being emitted, holding the locals that are that view instance's own reactive state.
    static CELLS: std::cell::RefCell<Vec<Frame>> = const { std::cell::RefCell::new(Vec::new()) };
    /// Top-level state bound with `const`, which a `value=` reads but never writes.
    static CONSTS: std::cell::RefCell<BTreeSet<String>> = const { std::cell::RefCell::new(BTreeSet::new()) };
    /// The module's own top-level functions and what each says it returns, so a definition shadows the tag of the same name and a view declares itself.
    static FNS: std::cell::RefCell<BTreeMap<String, Option<Type>>> = const { std::cell::RefCell::new(BTreeMap::new()) };
}

fn set_fns(m: &Module) {
    let mut out = BTreeMap::new();
    for item in &m.items {
        if let Stmt::Fn(f) = item {
            out.insert(f.name.clone(), f.ret.clone());
        }
    }
    FNS.with(|s| *s.borrow_mut() = out);
}

/// Is `n` a name some enclosing function already declared?
fn declared(n: &str) -> bool {
    SCOPES.with(|s| s.borrow().iter().any(|f| f.contains(n)))
}

/// Record that this function declares `n`.
fn declare(n: &str) {
    SCOPES.with(|s| {
        if let Some(top) = s.borrow_mut().last_mut() {
            top.insert(n.to_string());
        }
    });
}
fn set_state_names(names: &BTreeSet<String>) {
    STATE.with(|s| *s.borrow_mut() = names.clone());
}
fn set_consts(names: &BTreeSet<String>) {
    CONSTS.with(|s| *s.borrow_mut() = names.clone());
}
/// Is `n` a local held in a reactive cell by some function being emitted?
fn is_cell(n: &str) -> bool {
    CELLS.with(|s| s.borrow().iter().any(|f| f.cells.contains(n)))
}
/// Is `n` a local of the view being emitted that one binding computes, so a bind may recompute it?
fn derivable(n: &str) -> bool {
    CELLS.with(|s| s.borrow().last().is_some_and(|f| f.single.contains(n)))
}
/// Record that this view keeps `n` in a cell after all, because what it is computed from can change.
fn declare_cell(n: &str) {
    CELLS.with(|s| {
        if let Some(top) = s.borrow_mut().last_mut() {
            top.cells.insert(n.to_string());
        }
    });
}
fn push_frame(f: Frame) {
    CELLS.with(|s| s.borrow_mut().push(f));
}
fn pop_frame() {
    CELLS.with(|s| {
        s.borrow_mut().pop();
    });
}
fn set_variants(vs: &BTreeMap<String, usize>) {
    VARIANTS.with(|s| *s.borrow_mut() = vs.clone());
}
fn is_variant(n: &str) -> bool {
    VARIANTS.with(|s| s.borrow().contains_key(n))
}
/// A bare identifier → `state.x` when it names reactive state, `x.v` when it is a view's own cell, else itself.
fn jname(n: &str) -> String {
    if STATE.with(|s| s.borrow().contains(n)) {
        format!("state.{n}")
    } else if is_cell(n) {
        format!("{n}.v")
    } else {
        n.to_string()
    }
}

/// What a `_bind` records for one name it reads: a state name is a literal, a cell is the key its instance was given.
fn dep_key(n: &str) -> String {
    match is_cell(n) {
        true => format!("{n}.k"),
        false => format!("{n:?}"),
    }
}
/// The state names an expression reads, or `None` when a call puts them out of reach.
fn deps(e: &Expr) -> Option<BTreeSet<String>> {
    let none = || Some(BTreeSet::new());
    match e {
        Expr::Int(_)
        | Expr::Float(_)
        | Expr::Bool(_)
        | Expr::Unit
        | Expr::Path(_)
        | Expr::Break
        | Expr::Continue => none(),
        Expr::Ident(n) => Some(match STATE.with(|s| s.borrow().contains(n)) || is_cell(n) {
            true => BTreeSet::from([n.clone()]),
            false => BTreeSet::new(),
        }),
        Expr::Str(parts) => deps_all(parts.iter().filter_map(|p| match p {
            StrPart::Interp(x) => Some(x),
            StrPart::Text(_) => None,
        })),
        Expr::Binary { lhs, rhs, .. } => deps_all([lhs.as_ref(), rhs.as_ref()].into_iter()),
        Expr::Unary { expr, .. } | Expr::Field { base: expr, .. } => deps(expr),
        Expr::Index { base: a, index: b } | Expr::Range { lo: a, hi: b } => {
            deps_all([a.as_ref(), b.as_ref()].into_iter())
        }
        Expr::Ternary { cond, then, els } => {
            deps_all([cond.as_ref(), then.as_ref(), els.as_ref()].into_iter())
        }
        Expr::List(xs) => deps_all(xs.iter()),
        Expr::Record(fields) | Expr::Ctor { fields, .. } => {
            deps_all(field_values(fields).into_iter())
        }
        Expr::With { base, fields } => {
            deps_all(std::iter::once(base.as_ref()).chain(field_values(fields)))
        }
        Expr::Match { scrut, arms } => deps_all(
            std::iter::once(scrut.as_ref())
                .chain(arms.iter().flat_map(|a| a.guard.iter().chain([&a.body]))),
        ),
        _ => None,
    }
}

/// The union of what several expressions read, or `None` as soon as one of them is out of reach.
fn deps_all<'a>(xs: impl Iterator<Item = &'a Expr>) -> Option<BTreeSet<String>> {
    let mut out = BTreeSet::new();
    for x in xs {
        out.extend(deps(x)?);
    }
    Some(out)
}

/// Does an expression read reactive state and nothing else, so recomputing it is both worthwhile and free of consequence?
fn reads_state(e: &Expr) -> bool {
    matches!(deps(e), Some(names) if !names.is_empty())
}

/// Does an expression read reactive state or call a function (so a text/attr node reading it has to be re-run)?
fn is_dynamic(e: &Expr) -> bool {
    match deps(e) {
        None => true,
        Some(names) => !names.is_empty(),
    }
}

/// The dependency list a `_bind` is registered with: the names it reads, or `null` for "whenever anything changed".
fn dep_list(e: &Expr) -> String {
    match deps(e) {
        None => "null".into(),
        Some(names) => format!(
            "[{}]",
            names
                .iter()
                .map(|n| dep_key(n))
                .collect::<Vec<_>>()
                .join(", ")
        ),
    }
}

/// Lower a Maca expression to a JS expression (the functional core).
fn jexpr(e: &Expr) -> String {
    match e {
        Expr::Int(n) => n.to_string(),
        Expr::Float(f) => format!("{f}"),
        Expr::Bool(b) => b.to_string(),
        Expr::Unit => "null".into(),
        Expr::Str(parts) => jstr(parts),
        Expr::Path(p) => format!("{p:?}"),
        Expr::Ident(n) => jname(n),
        Expr::Unary { op, expr } => {
            let o = if matches!(op, UnOp::Not) { "!" } else { "-" };
            format!("{o}({})", jexpr(expr))
        }
        Expr::Binary { op, lhs, rhs } => jbinary(*op, lhs, rhs),
        Expr::Ternary { cond, then, els } => {
            format!("({} ? {} : {})", jexpr(cond), jexpr(then), jexpr(els))
        }
        Expr::Call { callee, args } => jcall(callee, args),
        Expr::Field { base, name } => format!("{}.{name}", jexpr(base)),
        Expr::Index { base, index } => format!("{}[{}]", jexpr(base), jexpr(index)),
        Expr::Record(fields) | Expr::Ctor { fields, .. } => jrecord(fields),
        Expr::List(es) => {
            format!("[{}]", es.iter().map(jexpr).collect::<Vec<_>>().join(", "))
        }
        Expr::Range { lo, hi } => format!(
            "Array.from({{length: Math.max(0, ({}) - ({}) + 1)}}, (_, _i) => _i + ({}))",
            jexpr(hi),
            jexpr(lo),
            jexpr(lo)
        ),
        Expr::If { cond, then, els } => {
            let e = els
                .as_ref()
                .map(|s| jblock_expr(s))
                .unwrap_or_else(|| "null".into());
            format!("({} ? {} : {})", jexpr(cond), jblock_expr(then), e)
        }
        Expr::Block(stmts) => jblock_expr(stmts),
        Expr::Try(x) => jexpr(x),
        Expr::With { base, fields } => {
            let upd = jrecord(fields);
            format!("{{ ...{}, ...{} }}", jexpr(base), upd)
        }
        Expr::Assign { target, value } => format!("({} = {})", jexpr(target), jexpr(value)),
        Expr::Await(x) | Expr::Spawn(x) => jexpr(x),
        Expr::Lambda { params, body, .. } => {
            let ps = params
                .iter()
                .map(|p| p.name.clone())
                .collect::<Vec<_>>()
                .join(", ");
            format!("(({ps}) => {})", arrow_body(body))
        }
        Expr::Fail(x) => format!("(() => {{ throw new Error(String({})); }})()", jexpr(x)),
        Expr::Reify(x) => format!(
            "(() => {{ try {{ {}; return \"\"; }} catch (_e) {{ return String(_e.message); }} }})()",
            jexpr(x)
        ),
        Expr::Match { scrut, arms } => jmatch(scrut, arms),
        Expr::Return(_) => "(() => { throw new Error(\"`return` is a statement, \
             so it has no value here\"); })()"
            .into(),
        _ => "null".into(),
    }
}

/// The body of an arrow function, parenthesised when it opens with a brace, which JS would otherwise read as a block.
fn arrow_body(body: &Expr) -> String {
    let code = jexpr(body);

    if code.starts_with('{') {
        return format!("({code})");
    }

    code
}

/// `match` in expression position → an IIFE with an if-chain.
fn jmatch(scrut: &Expr, arms: &[Arm]) -> String {
    let mut body = format!("(() => {{ const _s = {};", jexpr(scrut));
    for a in arms {
        let (cond, binds) = jpattern(&a.pat, "_s");
        let (test, pre) = match &a.guard {
            Some(g) => (
                format!("{cond} && (() => {{ {binds}return {}; }})()", jexpr(g)),
                binds.clone(),
            ),
            None => (cond, binds),
        };
        body.push_str(&format!(
            " if ({test}) {{ {pre}return {}; }}",
            jexpr(&a.body)
        ));
    }
    body.push_str(" throw new Error(\"no match\"); })()");
    body
}

/// (condition, binding-statements) for matching `sv` against `pat`.
fn jpattern(pat: &Pattern, sv: &str) -> (String, String) {
    match pat {
        Pattern::Wild => ("true".into(), String::new()),
        Pattern::Bind(n) if is_variant(n) => (format!("{sv}.$ === {n:?}"), String::new()),
        Pattern::Bind(n) => {
            declare(n);
            ("true".into(), format!("var {n} = {sv}; "))
        }
        Pattern::Int(n) => (format!("{sv} === {n}"), String::new()),
        Pattern::Float(f) => (format!("{sv} === {f}"), String::new()),
        Pattern::Bool(b) => (format!("{sv} === {b}"), String::new()),
        Pattern::Str(s) => (format!("{sv} === {s:?}"), String::new()),
        Pattern::Ctor { name, args } => {
            let mut conds = vec![format!("{sv}.$ === {name:?}")];
            let mut binds = String::new();
            for (i, a) in args.iter().enumerate() {
                let (c, b) = jpattern(a, &format!("{sv}._{i}"));
                if c != "true" {
                    conds.push(c);
                }
                binds.push_str(&b);
            }
            (format!("({})", conds.join(" && ")), binds)
        }
        Pattern::Record(fields) => {
            let mut conds = Vec::new();
            let mut binds = String::new();
            for (fname, sub) in fields {
                match sub {
                    None => {
                        declare(fname);
                        binds.push_str(&format!("var {fname} = {sv}.{fname}; "));
                    }
                    Some(p) => {
                        let (c, b) = jpattern(p, &format!("{sv}.{fname}"));
                        if c != "true" {
                            conds.push(c);
                        }
                        binds.push_str(&b);
                    }
                }
            }
            let cond = if conds.is_empty() {
                "true".into()
            } else {
                format!("({})", conds.join(" && "))
            };
            (cond, binds)
        }
        Pattern::List { elems, rest } => {
            let n = elems.len();
            let op = if rest.is_some() { ">=" } else { "===" };
            let mut conds = vec![format!("Array.isArray({sv}) && {sv}.length {op} {n}")];
            let mut binds = String::new();
            for (i, e) in elems.iter().enumerate() {
                let (c, b) = jpattern(e, &format!("{sv}[{i}]"));
                if c != "true" {
                    conds.push(c);
                }
                binds.push_str(&b);
            }
            if let Some(Pattern::Bind(rn)) = rest.as_deref() {
                declare(rn);
                binds.push_str(&format!("var {rn} = {sv}.slice({n}); "));
            }
            (format!("({})", conds.join(" && ")), binds)
        }
        Pattern::Or(alts) => {
            let conds: Vec<String> = alts.iter().map(|p| jpattern(p, sv).0).collect();
            (format!("({})", conds.join(" || ")), String::new())
        }
    }
}

/// A block used in expression position → an IIFE returning its value.
fn jblock_expr(stmts: &[Stmt]) -> String {
    format!("(() => {{\n{}\n}})()", jblock(stmts))
}

/// Clone `e`, replacing every free reference to `from` with `Ident(to)`.
fn subst_ident(e: &Expr, from: &str, to: &str) -> Expr {
    let go = |x: &Expr| Box::new(subst_ident(x, from, to));
    let field = |f: &Field| match f {
        Field::Value { name, value } => Field::Value {
            name: name.clone(),
            value: subst_ident(value, from, to),
        },
        Field::Bare(v) => Field::Bare(subst_ident(v, from, to)),
        other => other.clone(),
    };
    match e {
        Expr::Ident(n) if n == from => Expr::Ident(to.to_string()),
        Expr::Str(parts) => Expr::Str(
            parts
                .iter()
                .map(|p| match p {
                    StrPart::Interp(x) => StrPart::Interp(subst_ident(x, from, to)),
                    t => t.clone(),
                })
                .collect(),
        ),
        Expr::Unary { op, expr } => Expr::Unary {
            op: *op,
            expr: go(expr),
        },
        Expr::Binary { op, lhs, rhs } => Expr::Binary {
            op: *op,
            lhs: go(lhs),
            rhs: go(rhs),
        },
        Expr::Ternary { cond, then, els } => Expr::Ternary {
            cond: go(cond),
            then: go(then),
            els: go(els),
        },
        Expr::Call { callee, args } => Expr::Call {
            callee: go(callee),
            args: args
                .iter()
                .map(|a| match a {
                    Arg::Pos(x) => Arg::Pos(subst_ident(x, from, to)),
                    Arg::Named { name, value } => Arg::Named {
                        name: name.clone(),
                        value: subst_ident(value, from, to),
                    },
                    Arg::Directive { kind, prop, value } => Arg::Directive {
                        kind: *kind,
                        prop: prop.clone(),
                        value: subst_ident(value, from, to),
                    },
                })
                .collect(),
        },
        Expr::Field { base, name } => Expr::Field {
            base: go(base),
            name: name.clone(),
        },
        Expr::Index { base, index } => Expr::Index {
            base: go(base),
            index: go(index),
        },
        Expr::Assign { target, value } => Expr::Assign {
            target: go(target),
            value: go(value),
        },
        Expr::List(es) => Expr::List(es.iter().map(|x| subst_ident(x, from, to)).collect()),
        Expr::Record(fs) => Expr::Record(fs.iter().map(field).collect()),
        Expr::Ctor { name, fields } => Expr::Ctor {
            name: name.clone(),
            fields: fields.iter().map(field).collect(),
        },
        Expr::With { base, fields } => Expr::With {
            base: go(base),
            fields: fields.iter().map(field).collect(),
        },
        Expr::Range { lo, hi } => Expr::Range {
            lo: go(lo),
            hi: go(hi),
        },
        Expr::Try(x) => Expr::Try(go(x)),
        Expr::Fail(x) => Expr::Fail(go(x)),
        Expr::Reify(x) => Expr::Reify(go(x)),
        Expr::Await(x) => Expr::Await(go(x)),
        Expr::Spawn(x) => Expr::Spawn(go(x)),
        Expr::Lambda { params, ret, body } if !params.iter().any(|p| p.name == from) => {
            Expr::Lambda {
                params: params.clone(),
                ret: ret.clone(),
                body: go(body),
            }
        }
        other => other.clone(),
    }
}

fn jbinary(op: BinOp, lhs: &Expr, rhs: &Expr) -> String {
    use BinOp::*;
    let (l, r) = (jexpr(lhs), jexpr(rhs));
    let o = match op {
        Add => "+",
        Sub => "-",
        Mul => "*",
        Div => "/",
        Mod => "%",
        Shl => "<<",
        Shr => ">>",
        Eq => "===",
        Ne => "!==",
        Lt => "<",
        Gt => ">",
        Le => "<=",
        Ge => ">=",
        And => "&&",
        Or => "||",
        Concat => return format!("({l}).concat({r})"),
        Union | Pipe => return l,
    };
    format!("({l} {o} {r})")
}

fn jcall(callee: &Expr, args: &[Arg]) -> String {
    let a: Vec<String> = args.iter().map(|x| jexpr(arg_expr(x))).collect();
    match callee {
        Expr::Ident(f) if f == "int" => format!(
            "Math.trunc(Number({}))",
            a.first().cloned().unwrap_or_default()
        ),
        Expr::Ident(f) if f == "float" => {
            format!("Number({})", a.first().cloned().unwrap_or_default())
        }
        Expr::Ident(f) if f == "str" => {
            format!("String({})", a.first().cloned().unwrap_or_default())
        }
        Expr::Ident(f) if f == "len" => {
            format!("({}).length", a.first().cloned().unwrap_or_default())
        }
        Expr::Ident(f) if f == "chr" => format!(
            "((_b)=>_b>0&&_b<256?String.fromCharCode(_b):\"\")({})",
            a.first().cloned().unwrap_or_default()
        ),
        Expr::Ident(f) if f == "ord" => format!(
            "((_s)=>_s&&_s.length?_s.charCodeAt(0):-1)({})",
            a.first().cloned().unwrap_or_default()
        ),
        Expr::Ident(t) if is_html_tag(t) => jelement(&format!("{t:?}"), args),
        Expr::Ident(t) if t == "element" && !FNS.with(|f| f.borrow().contains_key(t)) => {
            match args.split_first() {
                Some((Arg::Pos(tag), rest)) => jelement(&jexpr(tag), rest),
                _ => "document.createElement(\"div\")".into(),
            }
        }
        Expr::Ident(f) => format!("{f}({})", a.join(", ")),
        Expr::Field { base, name } if FNS.with(|m| m.borrow().contains_key(name)) => {
            let mut all = vec![jexpr(base)];
            all.extend(a.clone());
            format!("{name}({})", all.join(", "))
        }
        Expr::Field { base, name } => match method(name, &jexpr(base), &a) {
            Some(js) => js,
            None => format!("{}.{name}({})", jexpr(base), a.join(", ")),
        },
        _ => format!("{}({})", jexpr(callee), a.join(", ")),
    }
}

/// One of Maca's UFCS methods → JS that computes the same value.
fn method(name: &str, recv: &str, args: &[String]) -> Option<String> {
    let arg = |i: usize| args.get(i).cloned().unwrap_or_else(|| "undefined".into());
    let out = match name {
        "length" => format!("({recv}).length"),
        "slice" => format!("({recv}).slice({}, {})", arg(0), arg(1)),
        "index_of" => format!("({recv}).indexOf({})", arg(0)),
        "contains" => format!("_mhas({recv}, {})", arg(0)),

        "upper" => format!("({recv}).toUpperCase()"),
        "lower" => format!("({recv}).toLowerCase()"),
        "trim" => format!("({recv}).trim()"),
        "starts_with" => format!("({recv}).startsWith({})", arg(0)),
        "ends_with" => format!("({recv}).endsWith({})", arg(0)),
        "replace" => format!("({recv}).split({}).join({})", arg(0), arg(1)),
        "substr" => format!("_msubstr({recv}, {}, {})", arg(0), arg(1)),
        "repeat" => format!("({recv}).repeat({})", arg(0)),
        "pad_start" => format!("_mpad({recv}, {}, {}, 0)", arg(0), arg(1)),
        "pad_end" => format!("_mpad({recv}, {}, {}, 1)", arg(0), arg(1)),
        "pad_center" => format!("_mpad({recv}, {}, {}, 2)", arg(0), arg(1)),
        "split" => format!("({recv}).split({})", arg(0)),
        "chars" => format!("Array.from({recv})"),
        "at" => format!("(({recv})[{}] ?? \"\")", arg(0)),
        "is_whitespace" => format!("_mclass({recv}, 0)"),
        "is_ascii_digit" => format!("_mclass({recv}, 1)"),
        "is_alpha" => format!("_mclass({recv}, 2)"),

        "map" => format!("({recv}).map({})", arg(0)),
        "filter" => format!("({recv}).filter({})", arg(0)),
        "reduce" | "fold" => format!("_mfold({recv}, {}, {})", arg(0), arg(1)),
        "sort" => format!("_msort({recv})"),
        "reverse" => format!("[...({recv})].reverse()"),
        "push" => format!("[...({recv}), {}]", arg(0)),
        "pop" => format!("({recv}).slice(0, -1)"),
        "set" => format!("_mset({recv}, {}, {})", arg(0), arg(1)),
        "insert" => format!("_mins({recv}, {}, {})", arg(0), arg(1)),
        "remove" => format!("_mrem({recv}, {})", arg(0)),
        "index_of_by" => format!("({recv}).findIndex({})", arg(0)),
        "enumerate" => format!("({recv}).map((v, i) => ({{ index: i, value: v }}))"),
        "sort_by" => format!("_msortby({recv}, {})", arg(0)),
        "sum" => format!("({recv}).reduce((_a, _b) => _a + _b, 0)"),
        "min" => format!("_mpick({recv}, -1)"),
        "max" => format!("_mpick({recv}, 1)"),
        "first" => format!("({recv})[0]"),
        "last" => format!("_mlast({recv})"),
        "get" => format!("({recv})[{}]", arg(0)),
        "join" => format!("({recv}).join({})", arg(0)),
        "parallel" => format!("({recv}).map({})", arg(0)),
        _ => return None,
    };
    Some(out)
}

/// Helpers the method lowerings call, emitted only when one is used.
const METHOD_HELPERS: &[(&str, &str)] = &[
    (
        "_mhas",
        "function _mhas(x, v) { return typeof x === \"string\" ? x.includes(v) : x.indexOf(v) >= 0; }",
    ),
    (
        "_msubstr",
        "function _msubstr(s, a, n) { return s.slice(a, a + n); }",
    ),
    (
        "_mpad",
        "function _mpad(s, w, p, mode) {\n  const n = w - s.length;\n  if (n <= 0) return s;\n  const fill = (c) => p.repeat(Math.ceil(c / p.length)).slice(0, c);\n  if (mode === 0) return fill(n) + s;\n  if (mode === 1) return s + fill(n);\n  const l = Math.floor(n / 2);\n  return fill(l) + s + fill(n - l);\n}",
    ),
    (
        "_mclass",
        "function _mclass(s, kind) {\n  const c = s[0] ?? \"\";\n  if (kind === 0) return /\\s/.test(c);\n  if (kind === 1) return c >= \"0\" && c <= \"9\";\n  return /[A-Za-z]/.test(c);\n}",
    ),
    (
        "_mfold",
        "function _mfold(xs, init, f) { let a = init; for (const x of xs) a = f(a, x); return a; }",
    ),
    (
        "_mcmp",
        "function _mcmp(a, b) { return typeof a === \"number\" ? a - b : (a < b ? -1 : a > b ? 1 : 0); }",
    ),
    (
        "_msort",
        "function _msort(xs) { return [...xs].sort(_mcmp); }",
    ),
    (
        "_mpick",
        "function _mpick(xs, dir) { return xs.reduce((a, b) => (_mcmp(b, a) * dir > 0 ? b : a)); }",
    ),
    (
        "_mlast",
        "function _mlast(xs) { return xs[xs.length - 1]; }",
    ),
    (
        "_msortby",
        "function _msortby(xs, key) {\n  return xs.map((v) => [key(v), v]).sort((a, b) => _mcmp(a[0], b[0])).map((p) => p[1]);\n}",
    ),
    (
        "_mset",
        "function _mset(xs, i, v) { const r = xs.slice(); if (i >= 0 && i < r.length) r[i] = v; return r; }",
    ),
    (
        "_mins",
        "function _mins(xs, i, v) { const r = xs.slice(); r.splice(Math.max(0, Math.min(i, r.length)), 0, v); return r; }",
    ),
    (
        "_mrem",
        "function _mrem(xs, i) { const r = xs.slice(); if (i >= 0 && i < r.length) r.splice(i, 1); return r; }",
    ),
];

/// The helper definitions `js` actually calls, in dependency order.
fn method_helpers_for(js: &str) -> String {
    let mut out = String::new();
    for (name, def) in METHOD_HELPERS {
        let needed = js.contains(&format!("{name}("))
            || (*name == "_mcmp"
                && (js.contains("_msort(") || js.contains("_mpick(") || js.contains("_msortby(")));
        if needed && !out.contains(def) {
            out.push_str(def);
            out.push('\n');
        }
    }
    out
}

fn jrecord(fields: &[Field]) -> String {
    let mut out = Vec::new();
    for f in fields {
        match f {
            Field::Value { name, value } => out.push(format!("{name}: {}", jexpr(value))),
            Field::Shorthand(n) => out.push(format!("{n}: {n}")),
            _ => {}
        }
    }
    format!("{{ {} }}", out.join(", "))
}

/// String with interpolation → a JS template literal.
fn jstr(parts: &[StrPart]) -> String {
    let mut out = String::from("`");
    for p in parts {
        match p {
            StrPart::Text(t) => out.push_str(
                &t.replace('\\', "\\\\")
                    .replace('`', "\\`")
                    .replace('$', "\\$"),
            ),
            StrPart::Interp(e) => out.push_str(&format!("${{{}}}", jexpr(e))),
        }
    }
    out.push('`');
    out
}

#[derive(Default)]
struct Cx {
    state: Vec<(String, String)>,
    /// The subset of `state` bound with `const` (or a Capitalized name).
    consts: BTreeSet<String>,
    state_names: BTreeSet<String>,
    classes: BTreeSet<String>,
    n: usize,
}

impl Cx {
    fn fresh(&mut self) -> String {
        let v = format!("n{}", self.n);
        self.n += 1;
        v
    }

    /// Emit code that builds `expr` (an element call), returning its var name.
    fn element(&mut self, e: &Expr, out: &mut String) -> String {
        let is_element = is_tag_call(e);
        if !is_element {
            let v = self.fresh();
            let expr = jexpr(e);
            if contributes_elements(e) {
                out.push_str(&format!("  const {v} = _node({expr});\n"));
                return v;
            }
            out.push_str(&format!("  const {v} = document.createTextNode({expr});\n"));
            if is_dynamic(e) {
                out.push_str(&format!(
                    "  _bind({}, () => {{ {v}.textContent = {expr}; }});\n",
                    dep_list(e)
                ));
            }
            return v;
        }
        let Expr::Call { callee, args } = e else {
            unreachable!()
        };
        let tag = match callee.as_ref() {
            Expr::Ident(t) => t.clone(),
            _ => "div".into(),
        };
        let v = self.fresh();
        out.push_str(&format!(
            "  const {v} = document.createElement(\"{tag}\");\n"
        ));

        for a in args {
            match a {
                Arg::Named { name, value } if name == "html" => {
                    let expr = jexpr(value);
                    out.push_str(&format!("  {v}.innerHTML = {expr};\n"));
                    if is_dynamic(value) {
                        out.push_str(&format!(
                            "  _bind({}, () => {{ {v}.innerHTML = {expr}; }});\n",
                            dep_list(value)
                        ));
                    }
                }
                Arg::Named { name, value } if name == "class" => {
                    if let Expr::Str(parts) = value {
                        for c in plain_text(parts).split_whitespace() {
                            self.classes.insert(c.to_string());
                        }
                    }
                    let expr = jexpr(value);
                    out.push_str(&format!("  {v}.className = {expr};\n"));
                    if is_dynamic(value) {
                        out.push_str(&format!(
                            "  _bind({}, () => {{ {v}.className = {expr}; }});\n",
                            dep_list(value)
                        ));
                    }
                }
                Arg::Named { name, value } if event_name(name).is_some() => {
                    self.handler(&v, event_name(name).unwrap(), value, out);
                }
                Arg::Named { name, value } if name == "value" && writable(value).is_some() => {
                    self.two_way_bind(&v, name, value, out);
                }
                Arg::Named { name, value } => {
                    let expr = jexpr(value);
                    out.push_str(&format!("  _attr({v}, \"{name}\", {expr});\n"));
                    if is_dynamic(value) {
                        out.push_str(&format!(
                            "  _bind({}, () => {{ _attr({v}, \"{name}\", {expr}); }});\n",
                            dep_list(value)
                        ));
                    }
                }
                Arg::Directive {
                    kind: Dir::Bind,
                    prop,
                    value,
                } => self.two_way_bind(&v, prop, value, out),
                Arg::Directive {
                    kind: Dir::On,
                    prop,
                    value,
                } => self.handler(&v, prop, value, out),
                Arg::Pos(child) if !is_tag_call(child) && contributes_elements(child) => {
                    out.push_str(&format!("  _append({v}, {});\n", jexpr(child)));
                }
                Arg::Pos(child) => {
                    let cv = self.element(child, out);
                    out.push_str(&format!("  {v}.appendChild({cv});\n"));
                }
            }
        }
        v
    }

    /// An event handler: the whole call is one turn, so whatever it assigns repaints once, when it returns.
    fn handler(&self, v: &str, event: &str, value: &Expr, out: &mut String) {
        out.push_str(&format!(
            "  {v}.addEventListener(\"{event}\", _turn({}));\n",
            jexpr(value)
        ));
    }

    /// `value=name` (or the older `bind:value=`): the property follows the state, and typing writes it back.
    fn two_way_bind(&self, v: &str, prop: &str, value: &Expr, out: &mut String) {
        let Some((key, get, set)) = writable(value) else {
            out.push_str(&format!("  {v}.{prop} = {};\n", jexpr(value)));
            return;
        };
        out.push_str(&format!("  {v}.{prop} = {get};\n"));
        out.push_str(&format!(
            "  {v}.addEventListener(\"input\", _turn((e) => {{ {} ; }}));\n",
            set.replace("$v", "e.target.value")
        ));
        out.push_str(&format!(
            "  _bind([{key}], () => {{ if (document.activeElement !== {v}) {v}.{prop} = {get}; }});\n"
        ));
    }

    /// The half of the module that has to exist before an `import js` block runs.
    fn bridge(&self, hosts: &[String]) -> String {
        let state = self
            .state
            .iter()
            .map(|(n, v)| format!("{n}: {v}"))
            .collect::<Vec<_>>()
            .join(", ");
        let names = |ns: Vec<&String>| {
            ns.iter()
                .map(|n| format!("{n:?}"))
                .collect::<Vec<_>>()
                .join(", ")
        };
        let vars = names(
            self.state
                .iter()
                .map(|(n, _)| n)
                .filter(|n| !self.consts.contains(*n))
                .collect(),
        );
        let consts = names(
            self.state
                .iter()
                .map(|(n, _)| n)
                .filter(|n| self.consts.contains(*n))
                .collect(),
        );
        let declared = names(hosts.iter().collect());
        format!(
            "\"use strict\";\n\
             const _state = {{ {state} }};\n\
             // ---- assignment is the update ----\n\
             {REACTIVE_JS}\
             // ---- the maca bridge ----\n\
             const _vars = [{vars}];\n\
             const _consts = [{consts}];\n\
             const _declared = [{declared}];\n\
             {BRIDGE_JS}"
        )
    }

    fn finish(&self, build_body: &str, root: &str, state_init: &str) -> String {
        format!(
            "// ---- the app ----\n\
             function _attr(el, k, v) {{\n\
             \x20 if (v === true) el.setAttribute(k, \"\");\n\
             \x20 else if (v === false || v == null) el.removeAttribute(k);\n\
             \x20 else el.setAttribute(k, v);\n\
             }}\n\
             function _node(x) {{\n\
             \x20 return x && x.nodeType ? x : document.createTextNode(String(x));\n\
             }}\n\
             function _append(p, c) {{\n\
             \x20 if (c === null || c === undefined) return;\n\
             \x20 if (Array.isArray(c)) {{ for (const x of c) _append(p, x); return; }}\n\
             \x20 p.appendChild(_node(c));\n\
             }}\n\
             function _dyn(deps, read) {{ return {{ $d: read, deps }}; }}\n\
             function _two(dep, read, write) {{ return {{ $t: dep, read, write }}; }}\n\
             function _marked(x, key) {{\n\
             \x20 return x !== null && typeof x === \"object\" && x[key] !== undefined;\n\
             }}\n\
             function _put(n, k, v) {{\n\
             \x20 if (k === \"class\") n.className = v;\n\
             \x20 else if (k === \"html\") n.innerHTML = v;\n\
             \x20 else _attr(n, k, v);\n\
             }}\n\
             function _live(n, k, b) {{\n\
             \x20 n[k] = b.read();\n\
             \x20 n.addEventListener(\"input\", _turn((e) => {{ b.write(e.target.value); }}));\n\
             \x20 _bind([b.$t], () => {{\n\
             \x20   if (document.activeElement !== n) n[k] = b.read();\n\
             \x20 }});\n\
             }}\n\
             function _kid(n, c) {{\n\
             \x20 const first = c.$d();\n\
             \x20 if (first === null || typeof first === \"object\") {{ _append(n, first); return; }}\n\
             \x20 const t = document.createTextNode(first);\n\
             \x20 _bind(c.deps, () => {{ t.textContent = c.$d(); }});\n\
             \x20 n.appendChild(t);\n\
             }}\n\
             function _el(tag, props, kids) {{\n\
             \x20 const n = document.createElement(tag);\n\
             \x20 for (const k of Object.keys(props)) {{\n\
             \x20   const v = props[k];\n\
             \x20   if (k.startsWith(\"on:\")) n.addEventListener(k.slice(3), _turn(v));\n\
             \x20   else if (_marked(v, \"$t\")) _live(n, k, v);\n\
             \x20   else if (_marked(v, \"$d\")) {{\n\
             \x20     _put(n, k, v.$d());\n\
             \x20     _bind(v.deps, () => {{ _put(n, k, v.$d()); }});\n\
             \x20   }} else _put(n, k, v);\n\
             \x20 }}\n\
             \x20 for (const c of kids) {{\n\
             \x20   if (_marked(c, \"$d\")) _kid(n, c);\n\
             \x20   else _append(n, c);\n\
             \x20 }}\n\
             \x20 return n;\n\
             }}\n\
             function build() {{\n\
             \x20 _depth += 1;\n\
             \x20 try {{\n{build_body}    return {root};\n\
             \x20 }} finally {{\n\
             \x20   _depth -= 1;\n\
             \x20   if (_depth === 0) _flush();\n\
             \x20 }}\n\
             }}\n\
             let _root = null;\n\
             function mount(target) {{ _root = build(); target.appendChild(_root); return _root; }}\n\
             function _start() {{\n{state_init}}}\n\
             _start();\n\
             if (typeof document !== \"undefined\" && document.getElementById) {{\n\
             \x20 const app = document.getElementById(\"app\");\n\
             \x20 if (app) mount(app);\n\
             }}\n\
             if (typeof module !== \"undefined\") module.exports = {{ state, mount, build, update, maca }};\n"
        )
    }

    fn css(&self) -> String {
        let mut out = String::from(
            "/* generated Tailwind subset (tree-shaken) */\n\
             *,*::before,*::after{box-sizing:border-box}\nhtml,body{margin:0}\n",
        );
        let mut sorted: Vec<&String> = self.classes.iter().collect();
        sorted.sort_by_key(|c| (order(c), (*c).clone()));
        for c in sorted {
            if let Some(r) = rule(c) {
                out.push_str(&r);
                out.push('\n');
            }
        }
        out
    }
}

/// The DOM event a named argument attaches to: `onclick` is `click`, and so is every other `on` followed by lowercase letters.
fn event_name(attr: &str) -> Option<&str> {
    let event = attr.strip_prefix("on")?;
    let vanilla = !event.is_empty() && event.bytes().all(|b| b.is_ascii_lowercase());
    vanilla.then_some(event)
}

/// Is `name` an HTML element tag (so `name(...)` in a view builds a DOM node), as opposed to a text-returning function call used as a child?
fn is_html_tag(name: &str) -> bool {
    maca_parser::is_ui_element_tag(name)
        && !FNS.with(|f| f.borrow().contains_key(name))
        && !declared(name)
        && !STATE.with(|s| s.borrow().contains(name))
}

/// `div(…)`: a call the view builds a node from directly, with its attributes bound.
fn is_tag_call(e: &Expr) -> bool {
    matches!(e, Expr::Call { callee, .. } if matches!(callee.as_ref(), Expr::Ident(t) if is_html_tag(t)))
}

/// A view built out of tags, so a call in child position contributes the nodes it made rather than its text.
fn contributes_elements(e: &Expr) -> bool {
    match e {
        Expr::List(_) => true,
        Expr::Binary {
            op: BinOp::Concat,
            lhs,
            rhs,
        } => contributes_elements(lhs) || contributes_elements(rhs),
        Expr::Call { callee, args } => match callee.as_ref() {
            Expr::Ident(t) if is_html_tag(t) => true,
            Expr::Ident(t) if t == "element" => true,
            Expr::Ident(f) => FNS.with(|m| m.borrow().get(f).is_some_and(is_element_ty)),
            Expr::Field { name, .. } if name == "map" => {
                args.first().is_some_and(|a| match arg_expr(a) {
                    Expr::Lambda { body, .. } => contributes_elements(body),
                    _ => false,
                })
            }
            _ => false,
        },
        Expr::If { then, els, .. } => {
            tail_contributes(then) || els.as_deref().is_some_and(tail_contributes)
        }
        Expr::Block(stmts) => tail_contributes(stmts),
        Expr::Match { arms, .. } => arms.iter().any(|a| contributes_elements(&a.body)),
        _ => false,
    }
}

/// Does the value a block hands back come out of tags?
fn tail_contributes(stmts: &[Stmt]) -> bool {
    stmts.iter().any(|s| match s {
        Stmt::Expr(Expr::Return(Some(e))) => contributes_elements(e),
        Stmt::Expr(e) => contributes_elements(e),
        _ => false,
    })
}

/// `Element` or `Element[]`: the signature that says a function hands back nodes.
fn is_element_ty(t: &Option<Type>) -> bool {
    match t {
        Some(Type::Name(segs)) => segs.last().is_some_and(|n| n == "Element"),
        Some(Type::Array(inner)) | Some(Type::Paren(inner)) => {
            is_element_ty(&Some((**inner).clone()))
        }
        _ => false,
    }
}

/// One named attribute as a value: a two-way binding when `value=` names writable state, a `_dyn` when it reads state, else the value itself.
fn jprop(name: &str, value: &Expr) -> String {
    if name == "value"
        && let Some(t) = two_way(value)
    {
        return t;
    }
    match is_dynamic(value) {
        true => format!("_dyn({}, () => {})", dep_list(value), jexpr(value)),
        false => jexpr(value),
    }
}

/// `bind:value=` → the same marker, falling back to a one-way read when nothing writable was named.
fn two_way_prop(value: &Expr) -> String {
    two_way(value).unwrap_or_else(|| jprop("", value))
}

/// The marker `_el` turns into a two-way binding: the key it watches, how to read the value, how to write it back.
fn two_way(value: &Expr) -> Option<String> {
    let (key, get, set) = writable(value)?;
    Some(format!("_two({key}, () => {get}, ($v) => {{ {set}; }})"))
}

/// What a `value=` writes, as (the key a bind watches, the JS that reads it, the JS that writes `$v` to it).
fn writable(target: &Expr) -> Option<(String, String, String)> {
    match target {
        Expr::Ident(n) => {
            let state = STATE.with(|s| s.borrow().contains(n));
            let konst = CONSTS.with(|s| s.borrow().contains(n));
            let get = jname(n);
            let set = format!("{get} = $v");
            (is_cell(n) || (state && !konst)).then(|| (dep_key(n), get, set))
        }
        Expr::Lambda { params, body, .. } => {
            let Expr::Assign { target: t, value } = body.as_ref() else {
                return None;
            };
            let (key, get, _) = writable(t)?;
            let pv = params.first().map(|p| p.name.as_str()).unwrap_or("v");
            let set = format!("{get} = {}", jexpr(&subst_ident(value, pv, "$v")));
            Some((key, get, set))
        }
        _ => None,
    }
}

/// `tag(attr=value, …, child, …)` as a value: one call that builds the node.
fn jelement(tag: &str, args: &[Arg]) -> String {
    let mut props: Vec<String> = Vec::new();
    let mut kids: Vec<String> = Vec::new();
    for a in args {
        match a {
            Arg::Named { name, value } => match event_name(name) {
                Some(ev) => props.push(format!("\"on:{ev}\": {}", jexpr(value))),
                None => props.push(format!("{name:?}: {}", jprop(name, value))),
            },
            Arg::Directive {
                kind: Dir::On,
                prop,
                value,
            } => props.push(format!("\"on:{prop}\": {}", jexpr(value))),
            Arg::Directive {
                kind: Dir::Bind,
                prop,
                value,
            } => props.push(format!("{prop:?}: {}", two_way_prop(value))),
            Arg::Pos(e) => kids.push(match !contributes_elements(e) && is_dynamic(e) {
                true => format!("_dyn({}, () => {})", dep_list(e), jexpr(e)),
                false => jexpr(e),
            }),
        }
    }
    format!(
        "_el({tag}, {{ {} }}, [{}])",
        props.join(", "),
        kids.join(", ")
    )
}

const HTML: &str = "<!doctype html>\n\
<html>\n<head>\n<meta charset=\"utf-8\">\n<title>maca app</title>\n\
<link rel=\"stylesheet\" href=\"app.css\">\n</head>\n\
<body>\n<div id=\"app\"></div>\n<script src=\"app.js\"></script>\n</body>\n</html>\n";

/// The literal a top-level binding can be written into the state object with, or `None` when it has to be computed once the program's functions exist.
fn js_init(e: &Expr) -> Option<String> {
    match e {
        Expr::Str(parts) if parts.iter().all(|p| matches!(p, StrPart::Text(_))) => {
            Some(format!("{:?}", plain_text(parts)))
        }
        Expr::Int(n) => Some(n.to_string()),
        Expr::Float(f) => Some(format!("{f}")),
        Expr::Bool(b) => Some(b.to_string()),
        _ => None,
    }
}

/// Where a class's rule belongs in the sheet.
pub fn order(class: &str) -> (u8, i32) {
    let (variants, _) = split_variants(class);
    if variants.is_empty() {
        return (0, 0);
    }
    let mut layer = 1u8;
    let mut width = 0i32;
    for v in &variants {
        match *v {
            "sm" => (layer, width) = (2, 40),
            "md" => (layer, width) = (2, 48),
            "lg" => (layer, width) = (2, 64),
            "xl" => (layer, width) = (2, 80),
            "max-lg" => (layer, width) = (3, -64),
            "max-md" => (layer, width) = (3, -48),
            "max-sm" => (layer, width) = (3, -40),
            "dark" => layer = layer.max(2),
            _ => {}
        }
    }
    (layer, width)
}

/// A whole CSS rule for one utility class, variants included.
pub fn rule(class: &str) -> Option<String> {
    let (variants, base) = split_variants(class);
    let body = tailwind(base)?;
    let mut selector = format!(".{}", css_escape(class));
    let mut media: Vec<&str> = Vec::new();
    for v in &variants {
        match *v {
            "hover" => selector.push_str(":hover"),
            "focus" => selector.push_str(":focus"),
            "active" => selector.push_str(":active"),
            "first" => selector.push_str(":first-child"),
            "last" => selector.push_str(":last-child"),
            "open" => selector.push_str("[open]"),
            "before" => selector.push_str("::before"),
            "after" => selector.push_str("::after"),
            "marker" => selector.push_str("::marker"),
            "details-marker" => selector.push_str("::-webkit-details-marker"),
            "placeholder" => selector.push_str("::placeholder"),
            "dark" => media.push("(prefers-color-scheme:dark)"),
            "sm" => media.push("(min-width:40rem)"),
            "md" => media.push("(min-width:48rem)"),
            "lg" => media.push("(min-width:64rem)"),
            "xl" => media.push("(min-width:80rem)"),
            "max-sm" => media.push("(max-width:40rem)"),
            "max-md" => media.push("(max-width:48rem)"),
            "max-lg" => media.push("(max-width:64rem)"),
            _ => return None,
        }
    }
    let rule = format!("{selector} {{ {body} }}");
    Some(if media.is_empty() {
        rule
    } else {
        format!("@media{} {{ {rule} }}", media.join(" and "))
    })
}

/// Split `dark:hover:bg-x` into its variants and the utility they modify.
fn split_variants(class: &str) -> (Vec<&str>, &str) {
    let mut variants = Vec::new();
    let mut rest = class;
    while let Some(i) = rest.find(':') {
        if rest[..i].contains('[') {
            break;
        }
        variants.push(&rest[..i]);
        rest = &rest[i + 1..];
    }
    (variants, rest)
}

/// Maca's integrated Tailwind: turn a utility class into its CSS body.
fn tailwind(class: &str) -> Option<String> {
    let fixed = match class {
        "flex" => "display:flex",
        "inline-flex" => "display:inline-flex",
        "grid" => "display:grid",
        "block" => "display:block",
        "inline" => "display:inline",
        "inline-block" => "display:inline-block",
        "hidden" => "display:none",
        "flex-col" => "flex-direction:column",
        "flex-row" => "flex-direction:row",
        "flex-wrap" => "flex-wrap:wrap",
        "flex-1" => "flex:1 1 0%",
        "flex-auto" => "flex:1 1 auto",
        "flex-none" => "flex:none",
        "items-center" => "align-items:center",
        "items-start" => "align-items:flex-start",
        "items-end" => "align-items:flex-end",
        "items-baseline" => "align-items:baseline",
        "items-stretch" => "align-items:stretch",
        "justify-center" => "justify-content:center",
        "justify-between" => "justify-content:space-between",
        "justify-start" => "justify-content:flex-start",
        "justify-end" => "justify-content:flex-end",
        "justify-around" => "justify-content:space-around",
        "self-start" => "align-self:flex-start",
        "self-end" => "align-self:flex-end",
        "self-center" => "align-self:center",
        "self-stretch" => "align-self:stretch",
        "text-center" => "text-align:center",
        "text-left" => "text-align:left",
        "text-right" => "text-align:right",
        "font-bold" => "font-weight:700",
        "font-semibold" => "font-weight:600",
        "font-medium" => "font-weight:500",
        "font-normal" => "font-weight:400",
        "font-sans" => {
            "font-family:'Pretendard Variable','Pretendard',ui-sans-serif,system-ui,\
             -apple-system,'Segoe UI',sans-serif"
        }
        "font-mono" => "font-family:ui-monospace,'SF Mono','JetBrains Mono',Menlo,monospace",
        "uppercase" => "text-transform:uppercase",
        "lowercase" => "text-transform:lowercase",
        "italic" => "font-style:italic",
        "underline" => "text-decoration-line:underline",
        "whitespace-pre" => "white-space:pre",
        "whitespace-pre-wrap" => "white-space:pre-wrap",
        "whitespace-nowrap" => "white-space:nowrap",
        "whitespace-normal" => "white-space:normal",
        "truncate" => "overflow:hidden;text-overflow:ellipsis;white-space:nowrap",
        "break-words" => "overflow-wrap:break-word",
        "break-all" => "word-break:break-all",
        "break-keep" => "word-break:keep-all",
        "overflow-auto" => "overflow:auto",
        "overflow-hidden" => "overflow:hidden",
        "overflow-x-auto" => "overflow-x:auto",
        "overflow-y-auto" => "overflow-y:auto",
        "relative" => "position:relative",
        "absolute" => "position:absolute",
        "fixed" => "position:fixed",
        "sticky" => "position:sticky",
        "static" => "position:static",
        "list-none" => "list-style:none",
        "list-disc" => "list-style:disc",
        "list-decimal" => "list-style:decimal",
        "border-collapse" => "border-collapse:collapse",
        "table" => "display:table",
        "table-auto" => "table-layout:auto",
        "table-fixed" => "table-layout:fixed",
        "text-inherit" => "color:inherit",
        "border-separate" => "border-collapse:separate",
        "align-top" => "vertical-align:top",
        "align-middle" => "vertical-align:middle",
        "align-baseline" => "vertical-align:baseline",
        "font-serif" => "font-family:ui-serif,Georgia,serif",
        "no-underline" => "text-decoration-line:none",
        "shrink-0" => "flex-shrink:0",
        "grow" => "flex-grow:1",
        "content-none" => "content:\"\"",
        "shadow" => "box-shadow:0 1px 3px rgba(0,0,0,.1)",
        "shadow-md" => "box-shadow:0 4px 8px rgba(0,0,0,.12)",
        "shadow-lg" => "box-shadow:0 4px 14px rgba(0,0,0,.14)",
        "shadow-none" => "box-shadow:none",
        "overscroll-contain" => "overscroll-behavior:contain",
        "appearance-none" => "appearance:none",
        "inset-0" => "top:0;right:0;bottom:0;left:0",
        "cursor-pointer" => "cursor:pointer",
        "cursor-default" => "cursor:default",
        "resize-none" => "resize:none",
        "outline-none" => "outline:none",
        "pointer-events-none" => "pointer-events:none",
        "pointer-events-auto" => "pointer-events:auto",
        "select-none" => "user-select:none",
        "box-border" => "box-sizing:border-box",
        "box-content" => "box-sizing:content-box",
        "tabular-nums" => "font-variant-numeric:tabular-nums",
        "w-full" => "width:100%",
        "h-full" => "height:100%",
        "w-screen" => "width:100vw",
        "h-screen" => "height:100vh",
        "w-auto" => "width:auto",
        "h-auto" => "height:auto",
        "min-h-0" => "min-height:0",
        "min-w-0" => "min-width:0",
        "max-w-full" => "max-width:100%",
        "rounded-none" => "border-radius:0",
        "rounded" => "border-radius:0.25rem",
        "rounded-md" => "border-radius:0.375rem",
        "rounded-lg" => "border-radius:0.5rem",
        "rounded-xl" => "border-radius:0.75rem",
        "rounded-full" => "border-radius:9999px",
        "border" => "border-width:1px;border-style:solid",
        "border-0" => "border-width:0",
        "border-t" => "border-top-width:1px;border-top-style:solid",
        "border-b" => "border-bottom-width:1px;border-bottom-style:solid",
        "border-l" => "border-left-width:1px;border-left-style:solid",
        "border-r" => "border-right-width:1px;border-right-style:solid",
        "border-x" => {
            "border-left-width:1px;border-right-width:1px;\
                       border-left-style:solid;border-right-style:solid"
        }
        "border-y" => {
            "border-top-width:1px;border-bottom-width:1px;\
                       border-top-style:solid;border-bottom-style:solid"
        }
        _ => "",
    };
    if !fixed.is_empty() {
        return Some(format!("{fixed};"));
    }

    if let Some(v) = class.strip_prefix("min-h-") {
        return Some(format!("min-height:{};", size(v)?));
    }
    if let Some(v) = class.strip_prefix("min-w-") {
        return Some(format!("min-width:{};", size(v)?));
    }
    if let Some(open) = class.find("[")
        && class.ends_with(']')
    {
        let prefix = class[..open].trim_end_matches('-');
        let raw = &class[open + 1..class.len() - 1].replace('_', " ");
        if let Some(sides) = border_sides(prefix) {
            return Some(
                sides
                    .iter()
                    .map(|s| format!("border-{s}-width:{raw};border-{s}-style:solid;"))
                    .collect(),
            );
        }
        if let Some(prop) = arbitrary_property(prefix) {
            return Some(format!("{prop}:{raw};"));
        }
    }
    if let Some(v) = class.strip_prefix("underline-offset-")
        && let Ok(n) = v.parse::<u32>()
    {
        return Some(format!("text-underline-offset:{n}px;"));
    }
    if let Some(v) = class.strip_prefix("leading-") {
        if let Some(n) = line_height(v) {
            return Some(format!("line-height:{n};"));
        }
        return Some(format!("line-height:{};", space(v)?));
    }
    if let Some(v) = class.strip_prefix("decoration-") {
        return Some(format!("text-decoration-color:{};", color(v)?));
    }
    if let Some(at) = class.rfind('-')
        && let Some(sides) = border_sides(&class[..at])
        && let Ok(n) = class[at + 1..].parse::<u32>()
    {
        return Some(
            sides
                .iter()
                .map(|s| format!("border-{s}-width:{n}px;border-{s}-style:solid;"))
                .collect(),
        );
    }
    for (p, prop) in [
        ("scroll-mt-", "scroll-margin-top"),
        ("scroll-mb-", "scroll-margin-bottom"),
    ] {
        if let Some(v) = class.strip_prefix(p) {
            return Some(format!("{prop}:{};", space(v)?));
        }
    }
    for (p, prop) in [
        ("top-", "top"),
        ("right-", "right"),
        ("bottom-", "bottom"),
        ("left-", "left"),
    ] {
        if let Some(v) = class.strip_prefix(p) {
            return Some(format!("{prop}:{};", space(v)?));
        }
    }
    if let Some(v) = class.strip_prefix("max-w-") {
        if let Some(w) = container_width(v) {
            return Some(format!("max-width:{w};"));
        }
        return Some(format!("max-width:{};", size(v)?));
    }
    if let Some(v) = class.strip_prefix("max-h-") {
        return Some(format!("max-height:{};", size(v)?));
    }
    if let Some(v) = class.strip_prefix("gap-x-") {
        return Some(format!("column-gap:{};", space(v)?));
    }
    if let Some(v) = class.strip_prefix("gap-y-") {
        return Some(format!("row-gap:{};", space(v)?));
    }
    if let Some(v) = class.strip_prefix("grid-cols-") {
        return Some(format!(
            "grid-template-columns:repeat({},minmax(0,1fr));",
            v.parse::<u32>().ok()?
        ));
    }

    let (prefix, val) = class.split_once('-')?;
    let css = match prefix {
        "p" => format!("padding:{}", space(val)?),
        "px" => format!("padding-left:{v};padding-right:{v}", v = space(val)?),
        "py" => format!("padding-top:{v};padding-bottom:{v}", v = space(val)?),
        "pt" => format!("padding-top:{}", space(val)?),
        "pr" => format!("padding-right:{}", space(val)?),
        "pb" => format!("padding-bottom:{}", space(val)?),
        "pl" => format!("padding-left:{}", space(val)?),
        "m" => format!("margin:{}", space(val)?),
        "mx" => format!("margin-left:{v};margin-right:{v}", v = space(val)?),
        "my" => format!("margin-top:{v};margin-bottom:{v}", v = space(val)?),
        "mt" => format!("margin-top:{}", space(val)?),
        "mr" => format!("margin-right:{}", space(val)?),
        "mb" => format!("margin-bottom:{}", space(val)?),
        "ml" => format!("margin-left:{}", space(val)?),
        "gap" => format!("gap:{}", space(val)?),
        "w" => format!("width:{}", size(val)?),
        "h" => format!("height:{}", size(val)?),
        "basis" => format!("flex-basis:{}", size(val)?),
        "text" => {
            if let Some(sz) = text_size(val) {
                format!("font-size:{sz}")
            } else {
                format!("color:{}", color(val)?)
            }
        }
        "leading" => format!("line-height:{}", leading(val)?),
        "tracking" => format!("letter-spacing:{}", tracking(val)?),
        "opacity" => format!("opacity:{}", val.parse::<f32>().ok()? / 100.0),
        "z" => format!("z-index:{}", val.parse::<i32>().ok()?),
        "bg" => format!("background-color:{}", color(val)?),
        "border" => format!("border-color:{}", color(val)?),
        "caret" => format!("caret-color:{}", color(val)?),
        "rounded" if val == "sm" => "border-radius:0.125rem".into(),
        _ => return None,
    };
    Some(format!("{css};"))
}

/// Tailwind spacing scale → a rem/px length (`4` → `1rem`, `0.5` → `0.125rem`).
fn space(v: &str) -> Option<String> {
    Some(match v {
        "0" => "0".into(),
        "px" => "1px".into(),
        "auto" => "auto".into(),
        _ => {
            let n: f32 = v.parse().ok()?;
            format!("{}rem", n * 0.25)
        }
    })
}

/// Tailwind's named container widths, for `max-w-*`.
fn container_width(v: &str) -> Option<&'static str> {
    Some(match v {
        "xs" => "20rem",
        "sm" => "24rem",
        "md" => "28rem",
        "lg" => "32rem",
        "xl" => "36rem",
        "2xl" => "42rem",
        "3xl" => "48rem",
        "4xl" => "56rem",
        "5xl" => "64rem",
        "6xl" => "72rem",
        "7xl" => "80rem",
        "prose" => "65ch",
        "none" => "none",
        _ => return None,
    })
}

/// The edges a `border-…` utility draws on, or `None` if it isn't one.
fn border_sides(prefix: &str) -> Option<&'static [&'static str]> {
    Some(match prefix {
        "border-l" => &["left"],
        "border-r" => &["right"],
        "border-t" => &["top"],
        "border-b" => &["bottom"],
        "border-x" => &["left", "right"],
        "border-y" => &["top", "bottom"],
        _ => return None,
    })
}

/// The CSS property an arbitrary-value class sets.
fn arbitrary_property(prefix: &str) -> Option<&'static str> {
    Some(match prefix {
        "text" => "font-size",
        "w" => "width",
        "h" => "height",
        "min-w" => "min-width",
        "min-h" => "min-height",
        "max-w" => "max-width",
        "max-h" => "max-height",
        "bg" => "background-color",
        "border" => "border-color",
        "p" => "padding",
        "px" => "padding-inline",
        "py" => "padding-block",
        "pt" => "padding-top",
        "pb" => "padding-bottom",
        "pl" => "padding-left",
        "pr" => "padding-right",
        "m" => "margin",
        "mx" => "margin-inline",
        "my" => "margin-block",
        "mt" => "margin-top",
        "mb" => "margin-bottom",
        "ml" => "margin-left",
        "mr" => "margin-right",
        "gap" => "gap",
        "gap-x" => "column-gap",
        "gap-y" => "row-gap",
        "top" => "top",
        "right" => "right",
        "bottom" => "bottom",
        "left" => "left",
        "inset" => "inset",
        "scroll-mt" => "scroll-margin-top",
        "scroll-mb" => "scroll-margin-bottom",
        "leading" => "line-height",
        "font" => "font-family",
        "content" => "content",
        "shadow" => "box-shadow",
        "grid-cols" => "grid-template-columns",
        _ => return None,
    })
}

/// Tailwind's named line heights.
fn line_height(v: &str) -> Option<&'static str> {
    Some(match v {
        "none" => "1",
        "tight" => "1.25",
        "snug" => "1.375",
        "normal" => "1.5",
        "relaxed" => "1.625",
        "loose" => "2",
        _ => return None,
    })
}

/// Width/height value: spacing scale, plus `full`/`screen`/`auto`/fractions.
fn size(v: &str) -> Option<String> {
    Some(match v {
        "none" => "none".into(),
        "full" => "100%".into(),
        "screen" => "100vh".into(),
        "auto" => "auto".into(),
        "min" => "min-content".into(),
        "max" => "max-content".into(),
        "fit" => "fit-content".into(),
        _ if v.contains('/') => {
            let (a, b) = v.split_once('/')?;
            let (a, b): (f32, f32) = (a.parse().ok()?, b.parse().ok()?);
            format!("{:.4}%", a / b * 100.0)
        }
        _ => space(v)?,
    })
}

fn text_size(v: &str) -> Option<&'static str> {
    Some(match v {
        "xs" => "0.75rem",
        "sm" => "0.875rem",
        "base" => "1rem",
        "lg" => "1.125rem",
        "xl" => "1.25rem",
        "2xl" => "1.5rem",
        "3xl" => "1.875rem",
        _ => return None,
    })
}

fn leading(v: &str) -> Option<&'static str> {
    Some(match v {
        "none" => "1",
        "tight" => "1.25",
        "snug" => "1.375",
        "normal" => "1.5",
        "relaxed" => "1.625",
        "loose" => "2",
        _ => return None,
    })
}

fn tracking(v: &str) -> Option<&'static str> {
    Some(match v {
        "tight" => "-0.025em",
        "normal" => "0",
        "wide" => "0.025em",
        "wider" => "0.05em",
        "widest" => "0.1em",
        _ => return None,
    })
}

/// A compact color palette (name → hex).
fn color(v: &str) -> Option<&'static str> {
    Some(match v {
        "white" => "#ffffff",
        "black" => "#000000",
        "transparent" => "transparent",
        "current" => "currentColor",
        "zinc-50" => "#fafafa",
        "zinc-100" => "#f4f4f5",
        "zinc-200" => "#e4e4e7",
        "zinc-300" => "#d4d4d8",
        "zinc-400" => "#a1a1aa",
        "zinc-500" => "#71717a",
        "zinc-600" => "#52525b",
        "zinc-700" => "#3f3f46",
        "zinc-800" => "#27272a",
        "zinc-850" => "#1f1f23",
        "zinc-900" => "#18181b",
        "zinc-950" => "#0d0d0f",
        "violet-300" => "#c4b5fd",
        "violet-400" => "#a78bfa",
        "violet-500" => "#8b5cf6",
        "violet-600" => "#7c3aed",
        "cyan-400" => "#22d3ee",
        "emerald-400" => "#34d399",
        "emerald-500" => "#10b981",
        "rose-400" => "#fb7185",
        "rose-500" => "#f43f5e",
        "amber-400" => "#fbbf24",
        "amber-500" => "#f59e0b",
        "sky-400" => "#38bdf8",
        _ => return None,
    })
}

/// Escape a class name for use in a CSS selector.
pub fn css_escape(c: &str) -> String {
    let mut out = String::with_capacity(c.len() + 8);
    for ch in c.chars() {
        if matches!(
            ch,
            '/' | '.'
                | ':'
                | '['
                | ']'
                | '('
                | ')'
                | ','
                | '#'
                | '%'
                | '\''
                | '"'
                | '!'
                | '$'
                | '&'
                | '*'
                | '+'
                | ';'
                | '<'
                | '='
                | '>'
                | '?'
                | '@'
                | '^'
                | '`'
                | '{'
                | '|'
                | '}'
                | '~'
        ) {
            out.push('\\');
        }
        out.push(ch);
    }
    out
}

/// Every whitespace-separated token of every string literal reachable from an item, as Tailwind class candidates.
pub fn collect_class_strings(s: &Stmt, out: &mut BTreeSet<String>) {
    match s {
        Stmt::Fn(f) => match &f.body {
            Some(FnBody::Expr(e)) => collect_class_expr(e, out),
            Some(FnBody::Block(stmts)) => stmts.iter().for_each(|s| collect_class_strings(s, out)),
            None => {}
        },
        Stmt::Bind(b) => collect_class_expr(&b.value, out),
        Stmt::Expr(e) => collect_class_expr(e, out),
        _ => {}
    }
}
fn collect_class_expr(e: &Expr, out: &mut BTreeSet<String>) {
    match e {
        Expr::Str(parts) => {
            for c in plain_text(parts).split_whitespace() {
                out.insert(c.to_string());
            }
            for p in parts {
                if let StrPart::Interp(x) = p {
                    collect_class_expr(x, out);
                }
            }
        }
        Expr::Call { callee, args } => {
            collect_class_expr(callee, out);
            for a in args {
                collect_class_expr(arg_expr(a), out);
            }
        }
        Expr::Ternary { cond, then, els } => {
            collect_class_expr(cond, out);
            collect_class_expr(then, out);
            collect_class_expr(els, out);
        }
        Expr::Binary { lhs, rhs, .. } => {
            collect_class_expr(lhs, out);
            collect_class_expr(rhs, out);
        }
        Expr::Unary { expr, .. }
        | Expr::Field { base: expr, .. }
        | Expr::Try(expr)
        | Expr::Fail(expr)
        | Expr::Reify(expr) => collect_class_expr(expr, out),
        Expr::Index { base, index } => {
            collect_class_expr(base, out);
            collect_class_expr(index, out);
        }
        Expr::If { cond, then, els } => {
            collect_class_expr(cond, out);
            then.iter().for_each(|s| collect_class_strings(s, out));
            if let Some(e) = els {
                e.iter().for_each(|s| collect_class_strings(s, out));
            }
        }
        Expr::Match { scrut, arms } => {
            collect_class_expr(scrut, out);
            for a in arms {
                collect_class_expr(&a.body, out);
            }
        }
        Expr::Block(stmts) | Expr::For { body: stmts, .. } | Expr::While { body: stmts, .. } => {
            stmts.iter().for_each(|s| collect_class_strings(s, out))
        }
        Expr::Lambda { body, .. } => collect_class_expr(body, out),
        Expr::List(es) => es.iter().for_each(|x| collect_class_expr(x, out)),
        Expr::With { base, fields } => {
            collect_class_expr(base, out);
            for f in fields {
                if let Field::Value { value, .. } = f {
                    collect_class_expr(value, out);
                }
            }
        }
        Expr::Record(fields) | Expr::Ctor { fields, .. } => {
            for f in fields {
                if let Field::Value { value, .. } = f {
                    collect_class_expr(value, out);
                }
            }
        }
        Expr::Assign { target, value } => {
            collect_class_expr(target, out);
            collect_class_expr(value, out);
        }
        _ => {}
    }
}

fn sum_variants(e: &Expr) -> Option<()> {
    matches!(
        e,
        Expr::Binary {
            op: BinOp::Union,
            ..
        }
    )
    .then_some(())
}

/// Flatten a `A | B(int) | C` union into each variant's name and payload arity.
fn union_arms(e: &Expr, out: &mut BTreeMap<String, usize>) {
    match e {
        Expr::Binary {
            op: BinOp::Union,
            lhs,
            rhs,
        } => {
            union_arms(lhs, out);
            union_arms(rhs, out);
        }
        Expr::Ident(n) => {
            out.insert(n.clone(), 0);
        }
        Expr::Call { callee, args } => {
            if let Expr::Ident(n) = callee.as_ref() {
                out.insert(n.clone(), args.len());
            }
        }
        _ => {}
    }
}

/// Every sum variant declared in the module, with its payload arity.
fn collect_variants(m: &Module) -> BTreeMap<String, usize> {
    let mut out = BTreeMap::new();
    for item in &m.items {
        if let Stmt::Bind(b) = item
            && sum_variants(&b.value).is_some()
        {
            union_arms(&b.value, &mut out);
        }
    }
    out
}

/// A tagged object per variant, so `match` can test the tag and read payloads.
fn variant_ctors(vs: &BTreeMap<String, usize>) -> String {
    let mut out = String::new();
    for (name, arity) in vs {
        if *arity == 0 {
            out.push_str(&format!("const {name} = {{ $: {name:?} }};\n"));
        } else {
            let ps: Vec<String> = (0..*arity).map(|i| format!("_{i}")).collect();
            out.push_str(&format!(
                "function {name}({}) {{ return {{ $: {name:?}, {} }}; }}\n",
                ps.join(", "),
                ps.join(", ")
            ));
        }
    }
    out
}
