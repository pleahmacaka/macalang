mod ownership;

use maca_parser::ast::*;
use ownership::{Fresh, Tail};
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

pub fn emit(m: &Module) -> String {
    let mut cx = Cx::new(m);
    cx.collect();
    cx.emit_all();
    cx.finish()
}

/// Emit C, or the list of codegen limitations the module hit.
pub fn emit_checked(m: &Module) -> Result<String, Vec<String>> {
    let mut cx = Cx::new(m);
    cx.collect();
    cx.emit_all();
    if cx.problems.is_empty() {
        Ok(cx.finish())
    } else {
        Err(cx.problems)
    }
}

/// Whether the generated C uses the async runtime (so the driver links it).
pub fn needs_async(c_src: &str) -> bool {
    c_src.contains("maca_parallel_i64")
        || c_src.contains("maca_cancel")
        || c_src.contains("maca_spawn")
        || c_src.contains("maca_await")
        || c_src.contains("maca_sleep_ms")
}

/// Header names from `import c "header.h"` (FFI).
pub fn c_imports(m: &Module) -> Vec<String> {
    m.items
        .iter()
        .filter_map(|it| match it {
            Stmt::Import(Import::Foreign { lang, spec }) if lang == "c" => Some(spec.clone()),
            _ => None,
        })
        .collect()
}

/// Backend-facing type.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
enum CTy {
    Int,
    Float,
    F32,
    Str,
    Bool,
    Unit,
    Rec(String),
    Sum(String),
    Arr(Box<CTy>),
    /// `Map str V`: a string-keyed hash map, monomorphized on its value type the same way an array is on its element type.
    Map(Box<CTy>),
    /// SIMD vector, e.g. `f32x8` → { name: "f32x8", scalar_c: "float", lanes: 8 }.
    Vec {
        name: String,
        scalar_c: String,
        lanes: usize,
    },
    /// A closure value, carrying its arity.
    Closure2(Box<CTy>),
    /// A concurrent computation handle (`spawn e`), awaited with `await`.
    Future,
    /// A first-class function value (a lambda).
    Closure(Box<CTy>),
    Unknown,
}

/// `f32x8` → (C scalar type, lanes) for `ext_vector_type` typedefs.
fn parse_vec_c(name: &str) -> Option<(String, usize)> {
    let (elem, lanes) = name.split_once('x')?;
    let lanes: usize = lanes.parse().ok()?;
    let scalar = match elem {
        "f32" => "float",
        "f64" => "double",
        "i8" => "int8_t",
        "u8" => "uint8_t",
        "i16" => "int16_t",
        "u16" => "uint16_t",
        "i32" => "int32_t",
        "u32" => "uint32_t",
        "i64" => "int64_t",
        "u64" => "uint64_t",
        _ => return None,
    };
    Some((scalar.into(), lanes))
}

struct Cx<'a> {
    m: &'a Module,
    out: String,
    /// The parameter types a lambda about to be lowered will be called with, set for the duration of lowering that one lambda.
    lambda_hint: Option<Vec<CTy>>,
    sums: BTreeMap<String, Vec<String>>,
    variant_of: HashMap<String, String>,
    variant_payloads: HashMap<String, Vec<CTy>>,
    records: BTreeMap<String, Vec<(String, CTy)>>,
    /// Which of `records` were synthesized from an anonymous literal's shape rather than written by the author.
    anon_records: HashSet<String>,
    /// `Rec.field` -> the parameter types of a field declared `(T, U) -> R`.
    record_fn_params: HashMap<String, Vec<CTy>>,
    rec_order: Vec<String>,
    modules: HashSet<String>,
    lets: Vec<(String, CTy, Expr)>,
    let_names: HashSet<String>,
    /// The top-level names some function writes, which is what makes them variables rather than values.
    written_lets: HashSet<String>,
    fns: HashMap<String, (Vec<CTy>, CTy)>,
    /// Variadic fn name -> how many parameters come before the `...` one.
    variadics: HashMap<String, usize>,
    arr_elems: HashSet<CTy>,
    map_vals: HashSet<CTy>,
    /// `(fn name, param index)` pairs that hold a function value.
    closure_params: HashSet<(String, usize)>,
    vecs: BTreeSet<(String, String, usize)>,
    tmp: usize,
    hoisted_decls: Vec<String>,
    hoisted_defs: Vec<String>,
    lambda_count: usize,
    /// top-level fns already wrapped as a closure value (`f` passed by name).
    fn_thunks: HashSet<String>,
    generics: HashMap<String, FnDef>,
    spec_pending: Vec<(String, Vec<CTy>)>,
    /// C container types already defined, and the point in the output where more can still go.
    emitted_containers: HashSet<String>,
    /// Record structs already written out, so one discovered while lowering still gets a definition.
    emitted_structs: HashSet<String>,
    /// The type variables of the specialization being emitted, if any.
    type_subst: HashMap<String, CTy>,
    spec_done: HashSet<(String, Vec<CTy>)>,
    problems: Vec<String>,
    classes: BTreeSet<String>,
    /// Which expressions produce a string the binding becomes the only owner of.
    fresh: Fresh,
    /// The flattened pieces of the concatenation just lowered, if it was one.
    concat_pieces: Option<Vec<Piece>>,
    /// Names the function being lowered may append to in place.
    appendable: HashSet<String>,
    /// What each local was bound to, while the array types are being collected.
    local_tys: HashMap<String, CTy>,
    /// Locals currently holding a string this scope will release.
    owned_strs: HashSet<String>,
    /// How a `return` leaves the function being lowered: its C result type, and whether that result crosses the closure ABI boxed.
    returns: (CTy, bool),
    /// Locals of the function being lowered that a nested definition assigns, so they live in a heap cell both can reach.
    cells: HashSet<String>,
    /// The declared result type of the nested definition about to be lowered as a closure.
    closure_ret: Option<CTy>,
    /// Local names bound to a closure that takes no arguments.
    nullary: HashSet<String>,
}

impl<'a> Cx<'a> {
    fn new(m: &'a Module) -> Self {
        Cx {
            m,
            out: String::new(),
            lambda_hint: None,
            sums: BTreeMap::new(),
            variant_of: HashMap::new(),
            variant_payloads: HashMap::new(),
            records: BTreeMap::new(),
            anon_records: HashSet::new(),
            record_fn_params: HashMap::new(),
            rec_order: Vec::new(),
            modules: HashSet::new(),
            lets: Vec::new(),
            let_names: HashSet::new(),
            written_lets: HashSet::new(),
            fns: HashMap::new(),
            variadics: HashMap::new(),
            arr_elems: HashSet::new(),
            map_vals: HashSet::new(),
            closure_params: HashSet::new(),
            vecs: BTreeSet::new(),
            tmp: 0,
            hoisted_decls: Vec::new(),
            hoisted_defs: Vec::new(),
            lambda_count: 0,
            fn_thunks: HashSet::new(),
            generics: HashMap::new(),
            spec_pending: Vec::new(),
            emitted_containers: HashSet::new(),
            emitted_structs: HashSet::new(),
            type_subst: HashMap::new(),
            spec_done: HashSet::new(),
            problems: Vec::new(),
            classes: BTreeSet::new(),
            appendable: HashSet::new(),
            local_tys: HashMap::new(),
            fresh: Fresh::of(m),
            concat_pieces: None,
            owned_strs: HashSet::new(),
            returns: (CTy::Unit, false),
            cells: HashSet::new(),
            closure_ret: None,
            nullary: HashSet::new(),
        }
    }

    /// The finished translation unit.
    fn finish(mut self) -> String {
        if !self.out.contains("MACA_STYLES") {
            return self.out;
        }
        let mut css =
            String::from("*,*::before,*::after{box-sizing:border-box}\\nhtml,body{margin:0}\\n");
        let mut sorted: Vec<&String> = self.classes.iter().collect();
        sorted.sort_by_key(|c| (maca_backend_js::order(c), (*c).clone()));
        for c in sorted {
            if let Some(r) = maca_backend_js::rule(c) {
                css.push_str(&r.replace('\\', "\\\\"));
                css.push_str("\\n");
            }
        }
        let def = format!("#define MACA_STYLES \"{css}\"\n");
        match self.out.find("\n\n") {
            Some(i) => {
                self.out.insert_str(i + 2, &def);
                self.out
            }
            None => def + &self.out,
        }
    }

    /// Record a codegen limitation.
    fn problem(&mut self, msg: impl Into<String>) {
        self.problems.push(msg.into());
    }

    /// Record the utility names in a `class=` value.
    fn note_classes(&mut self, value: &Expr) {
        if let Expr::Str(parts) = value {
            for p in parts {
                if let StrPart::Text(t) = p {
                    for c in t.split_whitespace() {
                        self.classes.insert(c.to_string());
                    }
                }
            }
        }
    }

    fn note_vec(&mut self, t: &CTy) {
        if let CTy::Vec {
            name,
            scalar_c,
            lanes,
        } = t
        {
            self.vecs.insert((name.clone(), scalar_c.clone(), *lanes));
        }
    }
    fn is_simd_fn(&self, name: &str) -> bool {
        self.fns.get(name).is_some_and(|(ps, r)| {
            ps.iter().any(|t| matches!(t, CTy::Vec { .. })) || matches!(r, CTy::Vec { .. })
        })
    }

    /// A name no source program can collide with, for a value the lowering needs to hold on to for a statement.
    fn temp(&mut self) -> String {
        self.tmp += 1;
        format!("_t{}", self.tmp)
    }

    /// Emit a payload-bearing sum as `{ tag; union { … } as; }` plus a constructor `Sum_Variant(payload…)` per variant.
    fn emit_tagged_sum(&mut self, name: &str, vars: &[String]) {
        let recursive = self.sum_is_recursive(name);
        let tags = vars
            .iter()
            .map(|v| format!("{name}_tag_{v}"))
            .collect::<Vec<_>>()
            .join(", ");
        self.push(&format!("typedef enum {{ {tags} }} {name}_tag;"));
        if recursive {
            self.push(&format!("struct {name} {{ {name}_tag tag; union {{"));
        } else {
            self.push(&format!("typedef struct {{ {name}_tag tag; union {{"));
        }
        for v in vars {
            let p = self.variant_payloads.get(v).cloned().unwrap_or_default();
            if !p.is_empty() {
                let fields = p
                    .iter()
                    .enumerate()
                    .map(|(i, t)| {
                        let cty = c_type(t);
                        if self.is_boxed(name, t) {
                            format!("{cty}* _{i};")
                        } else {
                            format!("{cty} _{i};")
                        }
                    })
                    .collect::<Vec<_>>()
                    .join(" ");
                self.push(&format!("    struct {{ {fields} }} {v};"));
            }
        }
        if recursive {
            self.push("} as; };");
        } else {
            self.push(&format!("}} as; }} {name};"));
        }
        for v in vars {
            let p = self.variant_payloads.get(v).cloned().unwrap_or_default();
            let params = if p.is_empty() {
                "void".to_string()
            } else {
                p.iter()
                    .enumerate()
                    .map(|(i, t)| format!("{} _{i}", c_type(t)))
                    .collect::<Vec<_>>()
                    .join(", ")
            };
            self.push(&format!("static {name} {name}_{v}({params}) {{"));
            self.push(&format!("    {name} _v; _v.tag = {name}_tag_{v};"));
            for (i, t) in p.iter().enumerate() {
                if self.is_boxed(name, t) {
                    let cty = c_type(t);
                    self.push(&format!(
                        "    _v.as.{v}._{i} = ({cty}*)maca_alloc(sizeof({cty}));"
                    ));
                    self.push(&format!("    *_v.as.{v}._{i} = _{i};"));
                } else {
                    self.push(&format!("    _v.as.{v}._{i} = _{i};"));
                }
            }
            self.push("    return _v;");
            self.push("}");
        }
    }

    /// A sum with at least one payload-carrying variant → a tagged struct/union (as opposed to a plain C enum).
    fn is_tagged(&self, sum: &str) -> bool {
        self.sums.get(sum).is_some_and(|vs| {
            vs.iter()
                .any(|v| self.variant_payloads.get(v).is_some_and(|p| !p.is_empty()))
        })
    }

    fn collect(&mut self) {
        for item in &self.m.items {
            maca_backend_js::collect_class_strings(item, &mut self.classes);
        }
        for item in &self.m.items {
            if let Stmt::Bind(b) = item
                && let Expr::Ident(name) = &b.target
                && is_record_type(&b.value)
            {
                self.records.entry(name.clone()).or_default();
            }
        }
        for item in &self.m.items {
            match item {
                Stmt::Import(im) => {
                    if let Some(n) = import_name(im) {
                        self.modules.insert(n);
                    }
                }
                Stmt::Bind(b) => {
                    if let Expr::Ident(name) = &b.target {
                        if let Some(vars) = sum_variants(&b.value) {
                            let names: Vec<String> = vars.iter().map(|(n, _)| n.clone()).collect();
                            for (v, _) in &vars {
                                self.variant_of.insert(v.clone(), name.clone());
                            }
                            self.sums.insert(name.clone(), names);
                        } else if is_record_type(&b.value) {
                            self.records.insert(name.clone(), Vec::new());
                        }
                    }
                }
                _ => {}
            }
        }
        for item in &self.m.items {
            if let Stmt::Bind(b) = item
                && let Expr::Ident(name) = &b.target
            {
                if let Some(vars) = sum_variants(&b.value) {
                    for (v, ptys) in &vars {
                        let ctys: Vec<CTy> = ptys.iter().map(|t| self.cty(t)).collect();
                        self.variant_payloads.insert(v.clone(), ctys);
                    }
                    let _ = name;
                } else if let Some(fields) = self.record_fields(&b.value) {
                    self.note_fn_fields(name, &b.value);
                    self.records.insert(name.clone(), fields);
                }
            }
        }
        self.closure_params = closure_params(&self.m.items);
        for t in [CTy::Int, CTy::Str, CTy::Float, CTy::Bool] {
            self.arr_elems.insert(t);
        }
        for item in &self.m.items {
            match item {
                Stmt::Fn(f) => {
                    if let Some(at) = f.params.iter().position(|p| p.variadic) {
                        self.variadics.insert(f.name.clone(), at);
                        if is_fn_type(f.params[at].ty.as_ref()) {
                            self.problem(format!(
                                "`{}`: a variadic of function type is not \
                                 supported. Take a record with a function \
                                 field instead",
                                f.name
                            ));
                        }
                    }
                    if fn_is_generic(f) {
                        self.generics.insert(f.name.clone(), f.clone());
                    } else {
                        let params = f
                            .params
                            .iter()
                            .enumerate()
                            .map(|(i, p)| {
                                if p.ty.is_none()
                                    && self.closure_params.contains(&(f.name.clone(), i))
                                {
                                    CTy::Closure(Box::new(CTy::Int))
                                } else {
                                    self.param_cty(p)
                                }
                            })
                            .collect::<Vec<_>>();
                        let ret = match (&f.ret, &f.body) {
                            (Some(t), _) => self.cty(t),
                            (None, Some(FnBody::Expr(e))) => self.infer_ret(e),
                            _ => CTy::Unit,
                        };
                        for p in &params {
                            self.note_arr(p);
                            self.note_vec(p);
                        }
                        self.note_arr(&ret);
                        self.note_vec(&ret);
                        self.fns.insert(f.name.clone(), (params, ret));
                    }
                }
                Stmt::Bind(b) => {
                    if let Expr::Ident(name) = &b.target {
                        let is_type = sum_variants(&b.value).is_some()
                            || self.record_fields(&b.value).is_some();
                        if !is_type {
                            let cty = b.tys.first().map_or(CTy::Unknown, |t| self.cty(t));
                            self.let_names.insert(name.clone());
                            self.lets.push((name.clone(), cty, b.value.clone()));
                        }
                    }
                }
                _ => {}
            }
        }
        let fields: Vec<CTy> = self
            .records
            .values()
            .flatten()
            .map(|(_, t)| t.clone())
            .collect();
        for t in fields {
            self.note_arr(&t);
        }
        self.collect_list_arrays();
        self.topo_records();
        self.collect_written_lets();
    }

    /// A top-level name a function assigns is the program's own state, so it is a variable rather than the value it started as.
    fn collect_written_lets(&mut self) {
        let mut written = HashSet::new();
        for item in &self.m.items {
            let Stmt::Fn(f) = item else { continue };
            let taken: HashSet<&str> = f.params.iter().map(|p| p.name.as_str()).collect();
            let mut note = |name: &str| {
                if self.let_names.contains(name) && !taken.contains(name) {
                    written.insert(name.to_string());
                }
            };
            maca_parser::ast::walk_stmt(item, &mut |e| {
                if let Expr::Assign { target, .. } = e
                    && let Expr::Ident(n) = &**target
                {
                    note(n);
                }
            });
            for_each_stmt(f, &mut |s| {
                if let Stmt::Bind(b) = s
                    && let Expr::Ident(n) = &b.target
                {
                    note(n);
                }
            });
        }
        self.written_lets = written;
    }

    /// Instantiate array types for list literals that don't come from a record field (e.g. `let xs = 1, 2, 3`).
    fn collect_list_arrays(&mut self) {
        let items = self.m.items.clone();
        for item in &items {
            match item {
                Stmt::Fn(f) => {
                    self.local_tys.clear();
                    for p in &f.params {
                        if p.ty.is_some() {
                            let cty = self.param_cty(p);
                            self.local_tys.insert(p.name.clone(), cty);
                        }
                    }
                    match &f.body {
                        Some(FnBody::Block(s)) => self.walk_stmts(s),
                        Some(FnBody::Expr(e)) => self.walk_expr(e),
                        None => {}
                    }
                }
                Stmt::Bind(b) => self.walk_expr(&b.value),
                _ => {}
            }
        }
    }
    fn walk_stmts(&mut self, stmts: &[Stmt]) {
        for s in stmts {
            match s {
                Stmt::Bind(b) => {
                    for t in &b.tys {
                        let cty = self.cty(t);
                        self.note_arr(&cty);
                    }
                    self.walk_expr(&b.value);
                    if let Expr::Ident(n) = &b.target {
                        let cty = match b.tys.first() {
                            Some(t) => self.cty(t),
                            None => self.shallow_cty(&b.value),
                        };
                        if cty != CTy::Unknown {
                            self.local_tys.insert(n.clone(), cty);
                        }
                    }
                }
                Stmt::Expr(e) => self.walk_expr(e),
                _ => {}
            }
        }
    }
    fn walk_expr(&mut self, e: &Expr) {
        match e {
            Expr::List(es) => {
                if let Some(first) = es.first() {
                    let elem = self.shallow_cty(first);
                    if elem != CTy::Unknown {
                        self.note_arr(&CTy::Arr(Box::new(elem)));
                    }
                }
                es.iter().for_each(|x| self.walk_expr(x));
            }
            Expr::Call { callee, args } => {
                if let Expr::Field { base, name } = callee.as_ref() {
                    if name == "splat"
                        && let Expr::Ident(v) = base.as_ref()
                        && let Some((sc, ln)) = parse_vec_c(v)
                    {
                        self.vecs.insert((v.clone(), sc, ln));
                    }
                    if name == "split" || name == "chars" {
                        self.note_arr(&CTy::Arr(Box::new(CTy::Str)));
                    }
                }
                if let Expr::Ident(f) = callee.as_ref()
                    && f == "list_dir"
                {
                    self.note_arr(&CTy::Arr(Box::new(CTy::Str)));
                }
                self.walk_expr(callee);
                for a in args {
                    match a {
                        Arg::Pos(x)
                        | Arg::Named { value: x, .. }
                        | Arg::Directive { value: x, .. } => self.walk_expr(x),
                    }
                }
            }
            Expr::Ctor { fields, .. } | Expr::Record(fields) => {
                if let Expr::Record(fs) = e {
                    let shape = self.anon_shape(fs);
                    for (_, t) in &shape {
                        self.note_arr(t);
                    }
                    let anon = anon_record_name(&shape);
                    self.anon_records.insert(anon.clone());
                    self.records.insert(anon, shape);
                }
                for f in fields {
                    if let Field::Value { value, .. } | Field::Bare(value) = f {
                        self.walk_expr(value);
                    }
                }
            }
            Expr::Field { base, .. }
            | Expr::Unary { expr: base, .. }
            | Expr::Try(base)
            | Expr::Fail(base)
            | Expr::Reify(base) => self.walk_expr(base),
            Expr::Binary { lhs, rhs, .. }
            | Expr::Assign {
                target: lhs,
                value: rhs,
            } => {
                self.walk_expr(lhs);
                self.walk_expr(rhs);
            }
            Expr::Range { lo, hi } => {
                self.note_arr(&CTy::Arr(Box::new(CTy::Int)));
                self.walk_expr(lo);
                self.walk_expr(hi);
            }
            Expr::Ternary { cond, then, els } => {
                self.walk_expr(cond);
                self.walk_expr(then);
                self.walk_expr(els);
            }
            Expr::If { cond, then, els } => {
                self.walk_expr(cond);
                self.walk_stmts(then);
                if let Some(e) = els {
                    self.walk_stmts(e);
                }
            }
            Expr::Match { scrut, arms } => {
                self.walk_expr(scrut);
                for a in arms {
                    self.walk_expr(&a.body);
                }
            }
            Expr::For { iter, body, .. } => {
                self.walk_expr(iter);
                self.walk_stmts(body);
            }
            Expr::While { cond, body } => {
                self.walk_expr(cond);
                self.walk_stmts(body);
            }
            Expr::Lambda { body, .. } => self.walk_expr(body),
            Expr::With { base, fields } => {
                self.walk_expr(base);
                for f in fields {
                    if let Field::Value { value, .. } | Field::Bare(value) = f {
                        self.walk_expr(value);
                    }
                }
            }
            Expr::Block(stmts) => self.walk_stmts(stmts),
            Expr::Str(parts) => {
                for p in parts {
                    if let StrPart::Interp(x) = p {
                        self.walk_expr(x);
                    }
                }
            }
            _ => {}
        }
    }
    /// The type an arrow body yields, for a function that declared no `-> T`.
    fn infer_ret(&self, e: &Expr) -> CTy {
        match e {
            Expr::Binary { op, lhs, rhs } => match op {
                BinOp::Eq
                | BinOp::Ne
                | BinOp::Lt
                | BinOp::Gt
                | BinOp::Le
                | BinOp::Ge
                | BinOp::And
                | BinOp::Or => CTy::Bool,
                BinOp::Concat => CTy::Str,
                _ => {
                    let l = self.infer_ret(lhs);
                    if l != CTy::Unknown {
                        l
                    } else {
                        self.infer_ret(rhs)
                    }
                }
            },
            Expr::Unary { op, expr } => match op {
                UnOp::Not => CTy::Bool,
                UnOp::Neg => self.infer_ret(expr),
            },
            Expr::Ternary { then, els, .. } => {
                let t = self.infer_ret(then);
                if t != CTy::Unknown {
                    t
                } else {
                    self.infer_ret(els)
                }
            }
            _ => self.shallow_cty(e),
        }
    }

    fn shallow_cty(&self, e: &Expr) -> CTy {
        match e {
            Expr::Int(_) => CTy::Int,
            Expr::Float(_) => CTy::Float,
            Expr::Bool(_) => CTy::Bool,
            Expr::Str(_) | Expr::Path(_) => CTy::Str,
            Expr::Ctor { name, .. } => CTy::Rec(name.clone()),
            Expr::List(es) => es
                .first()
                .map(|x| CTy::Arr(Box::new(self.shallow_cty(x))))
                .unwrap_or(CTy::Unknown),
            Expr::Record(fs) => CTy::Rec(anon_record_name(&self.anon_shape(fs))),
            Expr::Ident(n) => self
                .variant_of
                .get(n)
                .map(|s| CTy::Sum(s.clone()))
                .or_else(|| self.local_tys.get(n).cloned())
                .unwrap_or(CTy::Unknown),
            Expr::Call { callee, .. } => match &**callee {
                Expr::Ident(n) => self
                    .variant_of
                    .get(n)
                    .map(|s| CTy::Sum(s.clone()))
                    .or_else(|| self.fns.get(n).map(|(_, r)| r.clone()))
                    .unwrap_or(CTy::Unknown),
                _ => CTy::Unknown,
            },
            _ => CTy::Unknown,
        }
    }

    /// The field list of an anonymous record literal, sorted by name.
    fn anon_shape(&self, fs: &[Field]) -> Vec<(String, CTy)> {
        let mut out: Vec<(String, CTy)> = fs
            .iter()
            .filter_map(|f| match f {
                Field::Value { name, value } => Some((name.clone(), self.shallow_cty(value))),
                Field::Shorthand(name) => {
                    Some((name.clone(), self.shallow_cty(&Expr::Ident(name.clone()))))
                }
                _ => None,
            })
            .collect();
        out.sort_by(|a, b| a.0.cmp(&b.0));
        out
    }

    /// The declared record a literal is being written into, if the context names one and the literal only writes fields that record has.
    fn expected_record(&mut self, expected: Option<&CTy>, fields: &[Field]) -> Option<String> {
        let CTy::Rec(name) = expected? else {
            return None;
        };
        let decl = self.records.get(name)?.clone();
        let named: Vec<String> = fields
            .iter()
            .map(|f| match f {
                Field::Value { name, .. } | Field::Shorthand(name) => Some(name.clone()),
                _ => None,
            })
            .collect::<Option<Vec<_>>>()?;
        let stray: Vec<&String> = named
            .iter()
            .filter(|n| !decl.iter().any(|(d, _)| d == *n))
            .collect();
        let missing: Vec<&String> = decl
            .iter()
            .map(|(d, _)| d)
            .filter(|d| !named.contains(d))
            .collect();
        if self.anon_records.contains(name) {
            return stray.is_empty().then(|| name.clone());
        }
        if stray.is_empty() && missing.is_empty() {
            return Some(name.clone());
        }
        let name = name.clone();
        if stray.is_empty() {
            for k in missing {
                self.problem(format!(
                    "this `{{ … }}` never writes `{name}`'s field `{k}`, so it \
                     is not the `{name}` this position wants"
                ));
            }
        } else {
            for k in stray {
                self.problem(format!(
                    "`{name}` has no field `{k}`, so this `{{ … }}` is not the \
                     `{name}` this position wants"
                ));
            }
        }
        None
    }

    fn note_arr(&mut self, t: &CTy) {
        match t {
            CTy::Arr(e) => {
                self.arr_elems.insert((**e).clone());
                self.note_arr(e);
            }
            CTy::Map(v) => {
                self.map_vals.insert((**v).clone());
                self.arr_elems.insert(CTy::Str);
                self.note_arr(v);
            }
            _ => {}
        }
    }

    fn record_fields(&self, e: &Expr) -> Option<Vec<(String, CTy)>> {
        let Expr::Record(fs) = e else { return None };
        if fs.is_empty() || !fs.iter().all(|f| matches!(f, Field::Type { .. })) {
            return None;
        }
        Some(
            fs.iter()
                .filter_map(|f| match f {
                    Field::Type { name, ty } => Some((name.clone(), self.cty(ty))),
                    _ => None,
                })
                .collect(),
        )
    }

    /// Remember the parameter types of every field declared as a function.
    fn note_fn_fields(&mut self, rec: &str, decl: &Expr) {
        let Expr::Record(fs) = decl else { return };
        for f in fs {
            if let Field::Type {
                name,
                ty: Type::Fn(ps, _),
            } = f
            {
                if ps.iter().any(|t| matches!(t, Type::Fn(_, _))) {
                    self.problem(format!(
                        "`{rec}.{name}` takes a function as an argument, which \
                         this back end cannot carry. Pass it as a parameter of \
                         the enclosing function instead"
                    ));
                }
                let ctys: Vec<CTy> = ps.iter().map(|t| self.cty(t)).collect();
                self.record_fn_params.insert(format!("{rec}.{name}"), ctys);
            }
        }
    }

    /// Register the container types a body's local annotations name, now that the type variables in them are known.
    fn note_local_containers(&mut self, stmts: &[Stmt]) {
        for st in stmts {
            if let Stmt::Bind(b) = st {
                for t in &b.tys {
                    let cty = self.cty(t);
                    self.note_arr(&cty);
                }
            }
            let mut inner: Vec<Vec<Stmt>> = Vec::new();
            walk_stmt(st, &mut |e| match e {
                Expr::If { then, els, .. } => {
                    inner.push(then.clone());
                    if let Some(e) = els {
                        inner.push(e.clone());
                    }
                }
                Expr::For { body, .. } | Expr::While { body, .. } | Expr::Block(body) => {
                    inner.push(body.clone())
                }
                _ => {}
            });
            for ss in inner {
                self.note_local_containers(&ss);
            }
        }
    }

    fn topo_records(&mut self) {
        let mut seen = HashSet::new();
        let names: Vec<String> = self.records.keys().cloned().collect();
        for n in names {
            self.topo_visit(&n, &mut seen);
        }
    }

    /// Combined dependency order over both records and tagged sums.
    fn struct_order(&self) -> Vec<String> {
        let mut seen = HashSet::new();
        let mut order = Vec::new();
        for n in self.records.keys() {
            self.struct_visit(n, &mut seen, &mut order);
        }
        for n in self.sums.keys() {
            if self.is_tagged(n) {
                self.struct_visit(n, &mut seen, &mut order);
            }
        }
        order
    }
    fn struct_visit(&self, n: &str, seen: &mut HashSet<String>, order: &mut Vec<String>) {
        if seen.contains(n) {
            return;
        }
        seen.insert(n.to_string());
        for d in self.struct_deps(n) {
            if self.records.contains_key(&d) || self.is_tagged(&d) {
                self.struct_visit(&d, seen, order);
            }
        }
        order.push(n.to_string());
    }
    /// Names referenced strictly *by value* (arrays are heap pointers and so break value cycles).
    fn value_deps(&self, n: &str) -> Vec<String> {
        let mut deps = Vec::new();
        if let Some(fields) = self.records.get(n) {
            for (_, t) in fields {
                if let Some(d) = value_dep(t) {
                    deps.push(d);
                }
            }
        }
        if let Some(vars) = self.sums.get(n) {
            for v in vars {
                if let Some(p) = self.variant_payloads.get(v) {
                    for t in p {
                        if let Some(d) = value_dep(t) {
                            deps.push(d);
                        }
                    }
                }
            }
        }
        deps
    }
    /// Can `from` reach `to` following only by-value type references?
    fn reaches(&self, from: &str, to: &str) -> bool {
        let mut stack = self.value_deps(from);
        let mut seen = HashSet::new();
        while let Some(n) = stack.pop() {
            if n == to {
                return true;
            }
            if seen.insert(n.clone()) {
                stack.extend(self.value_deps(&n));
            }
        }
        false
    }
    /// A tagged sum is recursive when it can reach itself by value.
    fn sum_is_recursive(&self, name: &str) -> bool {
        self.reaches(name, name)
    }
    /// A record is recursive when it can reach itself through struct-shaped references (which recurse through arrays).
    fn rec_is_recursive(&self, name: &str) -> bool {
        if !self.records.contains_key(name) {
            return false;
        }
        let mut stack = self.struct_deps(name);
        let mut seen = HashSet::new();
        while let Some(n) = stack.pop() {
            if n == name {
                return true;
            }
            if seen.insert(n.clone()) {
                stack.extend(self.struct_deps(&n));
            }
        }
        false
    }
    /// A payload slot is boxed (stored behind a pointer) when it is a sum that can reach the enclosing sum, the value cycle that would make the struct infinitely sized.
    fn is_boxed(&self, sum: &str, t: &CTy) -> bool {
        matches!(t, CTy::Sum(n) if self.reaches(n, sum))
    }

    /// Names of records / tagged sums referenced by value in a node's fields (record) or variant payloads (tagged sum).
    fn struct_deps(&self, n: &str) -> Vec<String> {
        let mut deps = Vec::new();
        if let Some(fields) = self.records.get(n) {
            for (_, t) in fields {
                if let Some(d) = struct_dep(t) {
                    deps.push(d);
                }
            }
        }
        if let Some(vars) = self.sums.get(n) {
            for v in vars {
                if let Some(p) = self.variant_payloads.get(v) {
                    for t in p {
                        if self.is_boxed(n, t) {
                            continue;
                        }
                        if let Some(d) = struct_dep(t) {
                            deps.push(d);
                        }
                    }
                }
            }
        }
        deps
    }
    fn topo_visit(&mut self, n: &str, seen: &mut HashSet<String>) {
        if seen.contains(n) {
            return;
        }
        seen.insert(n.to_string());
        let deps: Vec<String> = self
            .records
            .get(n)
            .map(|fs| fs.iter().filter_map(|(_, t)| rec_dep(t)).collect())
            .unwrap_or_default();
        for d in deps {
            if self.records.contains_key(&d) {
                self.topo_visit(&d, seen);
            }
        }
        self.rec_order.push(n.to_string());
    }

    fn cty(&self, t: &Type) -> CTy {
        match t {
            Type::Name(segs) if segs.len() == 1 => {
                let n = segs[0].as_str();
                if let Some(t) = self.type_subst.get(n) {
                    return t.clone();
                }
                if let Some((scalar_c, lanes)) = parse_vec_c(n) {
                    return CTy::Vec {
                        name: n.into(),
                        scalar_c,
                        lanes,
                    };
                }
                match n {
                    "int" | "i8" | "i16" | "i32" | "i64" | "u8" | "u16" | "u32" | "u64" => CTy::Int,
                    "f32" => CTy::F32,
                    "float" | "f64" => CTy::Float,
                    "str" | "bytes" => CTy::Str,
                    "bool" => CTy::Bool,
                    "unit" | "()" => CTy::Unit,
                    _ if self.records.contains_key(n) => CTy::Rec(n.into()),
                    _ if self.sums.contains_key(n) => CTy::Sum(n.into()),
                    "Path" => CTy::Str,
                    "Element" => CTy::Str,
                    _ => CTy::Unknown,
                }
            }
            Type::Array(t) => CTy::Arr(Box::new(self.cty(t))),
            Type::Opt(t) => self.cty(t),
            Type::Paren(t) => self.cty(t),
            Type::Fn(ps, r) => closure_ty(ps.len(), self.cty(r)),
            Type::Apply(base, args) if matches!(&**base, Type::Name(s) if s.last().is_some_and(|n| n == "Map")) => {
                CTy::Map(Box::new(args.last().map_or(CTy::Unknown, |t| self.cty(t))))
            }
            _ => CTy::Unknown,
        }
    }
    fn cty_opt(&self, t: &Option<Type>) -> CTy {
        t.as_ref().map_or(CTy::Unknown, |t| self.cty(t))
    }

    /// The C type a parameter is *declared* with.
    fn param_cty(&self, p: &Param) -> CTy {
        let t = self.cty_opt(&p.ty);
        if p.variadic { CTy::Arr(Box::new(t)) } else { t }
    }
}

impl<'a> Cx<'a> {
    fn push(&mut self, s: &str) {
        self.out.push_str(s);
        self.out.push('\n');
    }

    fn emit_all(&mut self) {
        self.push("#include \"maca_runtime.h\"");
        self.push("#include \"maca_async.h\"");
        self.push("#include <stdint.h>");
        self.push("#include <stdbool.h>");
        self.push("");

        let vecs: Vec<_> = self.vecs.iter().cloned().collect();
        for (name, scalar, lanes) in &vecs {
            self.push(&format!(
                "typedef {scalar} {name} __attribute__((ext_vector_type({lanes})));"
            ));
        }
        if !vecs.is_empty() {
            self.push("");
        }

        let sums: Vec<(String, Vec<String>)> = self
            .sums
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();
        for (name, vars) in &sums {
            if !self.is_tagged(name) {
                let variants = vars
                    .iter()
                    .map(|v| format!("{name}_{v}"))
                    .collect::<Vec<_>>()
                    .join(", ");
                self.push(&format!("typedef enum {{ {variants} }} {name};"));
            }
        }
        for (name, _) in &sums {
            if self.is_tagged(name) && self.sum_is_recursive(name) {
                self.push(&format!("typedef struct {name} {name};"));
            }
        }
        self.push("");

        let mut emitted_arr: HashSet<String> = HashSet::new();
        let mut elems: Vec<CTy> = self.arr_elems.iter().cloned().collect();
        elems.sort_by_key(|e| (arr_depth(e), arr_name(e)));
        for e in &elems {
            if !matches!(e, CTy::Rec(_) | CTy::Sum(_)) && !emitted_arr.contains(&arr_name(e)) {
                self.push(&format!(
                    "MACA_DEFINE_ARRAY({}, {})",
                    arr_name(e),
                    c_type(e)
                ));
                emitted_arr.insert(arr_name(e));
                self.emitted_containers.insert(arr_name(e));
            }
        }
        let mut vals: Vec<CTy> = self.map_vals.iter().cloned().collect();
        vals.sort_by_key(map_name);
        for v in &vals {
            if !matches!(v, CTy::Rec(_) | CTy::Sum(_)) {
                self.push(&format!("MACA_DEFINE_MAP({}, {})", map_name(v), c_type(v)));
                self.emitted_containers.insert(map_name(v));
            }
        }
        self.push("");

        let cyclic: HashSet<String> = self
            .records
            .keys()
            .filter(|n| self.rec_is_recursive(n))
            .cloned()
            .collect();
        let mut cyclic_arr_elems: Vec<CTy> = Vec::new();
        if !cyclic.is_empty() {
            for n in &cyclic {
                self.push(&format!("typedef struct {n} {n};"));
            }
            let mut want: Vec<CTy> = Vec::new();
            let consider = |e: &CTy, want: &mut Vec<CTy>| {
                if let CTy::Rec(r) = e
                    && cyclic.contains(r)
                    && !want.contains(e)
                {
                    want.push(e.clone());
                }
            };
            for fs in self.records.values() {
                for (_, t) in fs {
                    if let CTy::Arr(e) = t {
                        consider(e, &mut want);
                    }
                }
            }
            for e in &elems {
                if let CTy::Arr(inner) = e {
                    consider(inner, &mut want);
                }
                consider(e, &mut want);
            }
            for e in &want {
                self.push(&format!(
                    "MACA_ARRAY_STRUCT({}, {})",
                    arr_name(e),
                    c_type(e)
                ));
                emitted_arr.insert(arr_name(e));
                self.emitted_containers.insert(arr_name(e));
            }
            cyclic_arr_elems = want;
            self.push("");
        }

        let struct_order = self.struct_order();
        self.emitted_structs.extend(struct_order.iter().cloned());
        for name in &struct_order {
            let field_ctys: Vec<CTy> = if let Some(fs) = self.records.get(name) {
                fs.iter().map(|(_, t)| t.clone()).collect()
            } else {
                self.sums[name]
                    .iter()
                    .flat_map(|v| self.variant_payloads.get(v).cloned().unwrap_or_default())
                    .collect()
            };
            for t in &field_ctys {
                if let CTy::Arr(e) = t
                    && matches!(**e, CTy::Rec(_) | CTy::Sum(_))
                    && !emitted_arr.contains(&arr_name(e))
                {
                    self.push(&format!(
                        "MACA_DEFINE_ARRAY({}, {})",
                        arr_name(e),
                        c_type(e)
                    ));
                    emitted_arr.insert(arr_name(e));
                    self.emitted_containers.insert(arr_name(e));
                }
            }
            if self.records.contains_key(name) {
                let fields = self.records[name].clone();
                if cyclic.contains(name) {
                    self.push(&format!("struct {name} {{"));
                } else {
                    self.push("typedef struct {");
                }
                for (fname, t) in &fields {
                    self.push(&format!("    {} {};", c_type(t), cid(fname)));
                }
                if cyclic.contains(name) {
                    self.push("};");
                } else {
                    self.push(&format!("}} {name};"));
                }
            } else {
                let vars = self.sums[name].clone();
                self.emit_tagged_sum(name, &vars);
            }
        }
        for e in &cyclic_arr_elems {
            self.push(&format!("MACA_ARRAY_OPS({}, {})", arr_name(e), c_type(e)));
        }
        for e in &elems {
            if matches!(e, CTy::Rec(_) | CTy::Sum(_)) && !emitted_arr.contains(&arr_name(e)) {
                self.push(&format!(
                    "MACA_DEFINE_ARRAY({}, {})",
                    arr_name(e),
                    c_type(e)
                ));
                emitted_arr.insert(arr_name(e));
                self.emitted_containers.insert(arr_name(e));
            }
        }
        self.push("");

        for (name, vars) in &sums {
            let tagged = self.is_tagged(name);
            self.push(&format!("static maca_str {name}_to_str({name} v) {{"));
            self.push(&format!(
                "    switch (v{}) {{",
                if tagged { ".tag" } else { "" }
            ));
            for v in vars {
                let tag = if tagged {
                    format!("{name}_tag_{v}")
                } else {
                    format!("{name}_{v}")
                };
                self.push(&format!("        case {tag}: return \"{v}\";"));
            }
            self.push("    }");
            self.push("    return \"\";");
            self.push("}");
            self.push(&format!("static {name} {name}_from_str(maca_str s) {{"));
            if tagged {
                for v in vars {
                    if self.variant_payloads.get(v).is_none_or(|p| p.is_empty()) {
                        self.push(&format!(
                            "    if (maca_str_eq(s, \"{v}\")) return {name}_{v}();"
                        ));
                    }
                }
                let first = &vars[0];
                let zeros = self
                    .variant_payloads
                    .get(first)
                    .map(|p| {
                        p.iter()
                            .map(|t| format!("({}){{0}}", c_type(t)))
                            .collect::<Vec<_>>()
                            .join(", ")
                    })
                    .unwrap_or_default();
                self.push(&format!("    (void)s; return {name}_{first}({zeros});"));
            } else {
                for v in vars {
                    self.push(&format!(
                        "    if (maca_str_eq(s, \"{v}\")) return {name}_{v};"
                    ));
                }
                self.push(&format!("    return {name}_{};", vars[0]));
            }
            self.push("}");
            self.emit_sum_json_name(name, vars, tagged);
        }
        self.push("");

        let order = self.rec_order.clone();
        for name in &order {
            self.push(&format!("static maca_str {name}_to_json({name} v);"));
            self.push(&format!("static {name} {name}_from_json(maca_json* j);"));
        }
        self.push("");
        for name in &order {
            self.emit_to_json(name);
            self.emit_from_json(name);
        }

        let lets = self.lets.clone();
        for (name, cty, init) in &lets {
            let ret = self.let_ty(cty, init);
            if self.written_lets.contains(name) {
                self.push(&format!("static {} mv_{name};", c_type(&ret)));
            } else {
                self.push(&format!("static {} mv_{name}(void);", c_type(&ret)));
            }
        }
        self.push("static void maca_module_init(void);");
        self.push("");

        self.push(LATE_CONTAINERS);
        self.push("");

        for item in &self.m.items {
            if let Stmt::Fn(f) = item {
                if f.name == "main" || self.generics.contains_key(&f.name) {
                    continue;
                }
                if f.body.is_none() || self.is_simd_fn(&f.name) {
                    self.push(&format!("extern {};", self.fn_sig(f)));
                } else {
                    self.push(&format!("static {};", self.fn_sig(f)));
                }
            }
        }
        self.push("");

        let saved = std::mem::take(&mut self.out);
        let mut starts: Vec<String> = Vec::new();
        for (name, cty, init) in &lets {
            let ret = self.let_ty(cty, init);
            let mut env: Env = Vec::new();
            let (code, _) = self.expr(&mut env, init, None);
            if self.written_lets.contains(name) {
                starts.push(format!("    mv_{name} = {code};"));
            } else {
                self.push(&format!(
                    "static {} mv_{name}(void) {{ return {code}; }}",
                    c_type(&ret)
                ));
            }
        }
        self.push("static void maca_module_init(void) {");
        for line in &starts {
            self.push(line);
        }
        self.push("}");
        self.push("");
        for item in self.m.items.clone() {
            if let Stmt::Fn(f) = &item
                && f.body.is_some()
                && !self.is_simd_fn(&f.name)
                && !self.generics.contains_key(&f.name)
            {
                self.emit_fn(f);
                self.push("");
            }
        }
        while let Some((name, ctys)) = self.spec_pending.pop() {
            if !self.spec_done.insert((name.clone(), ctys.clone())) {
                continue;
            }
            self.emit_specialization(&name, &ctys);
        }
        let late = self.late_containers();
        let mut saved = saved;
        match saved.find(LATE_CONTAINERS) {
            Some(at) => saved.replace_range(at..at + LATE_CONTAINERS.len(), &late),
            None => saved.push_str(&late),
        }
        let defs = std::mem::replace(&mut self.out, saved);
        for d in self.hoisted_decls.clone() {
            self.push(&d);
        }
        for d in self.hoisted_defs.clone() {
            self.out.push_str(&d);
        }
        self.out.push_str(&defs);
    }

    /// Definitions for containers nobody knew about until a generic was specialized, shallowest first so an outer array names a complete inner one.
    fn late_containers(&mut self) -> String {
        let mut out = String::new();
        let late_recs: Vec<String> = self
            .records
            .keys()
            .filter(|n| !self.emitted_structs.contains(*n))
            .cloned()
            .collect();
        for name in &late_recs {
            let fields = self.records[name].clone();
            out.push_str("typedef struct {\n");
            for (fname, t) in &fields {
                out.push_str(&format!("    {} {};\n", c_type(t), cid(fname)));
            }
            out.push_str(&format!("}} {name};\n"));
            self.emitted_structs.insert(name.clone());
        }
        let mut elems: Vec<CTy> = self
            .arr_elems
            .iter()
            .filter(|e| !self.emitted_containers.contains(&arr_name(e)))
            .cloned()
            .collect();
        elems.sort_by_key(|e| (arr_depth(e), arr_name(e)));
        for e in &elems {
            out.push_str(&format!(
                "MACA_DEFINE_ARRAY({}, {})\n",
                arr_name(e),
                c_type(e)
            ));
            self.emitted_containers.insert(arr_name(e));
        }
        let mut vals: Vec<CTy> = self
            .map_vals
            .iter()
            .filter(|v| !self.emitted_containers.contains(&map_name(v)))
            .cloned()
            .collect();
        vals.sort_by_key(map_name);
        for v in &vals {
            out.push_str(&format!(
                "MACA_DEFINE_MAP({}, {})\n",
                map_name(v),
                c_type(v)
            ));
            self.emitted_containers.insert(map_name(v));
        }
        out
    }

    /// The C type of a top-level constant: its annotation, or what its initialiser says.
    fn let_ty(&self, cty: &CTy, init: &Expr) -> CTy {
        if *cty != CTy::Unknown {
            return cty.clone();
        }
        if let Some(sum) = self.sum_named(init) {
            return CTy::Sum(sum);
        }
        if let Expr::Call { callee, .. } = init
            && let Some(f) = called_name(callee)
            && let Some((_, ret)) = self.fns.get(f)
            && !matches!(ret, CTy::Unknown | CTy::Unit)
        {
            return ret.clone();
        }
        infer_cty_shallow(init)
    }

    /// The sum type this expression constructs, written as a bare variant or as one applied to its payload.
    fn sum_named(&self, e: &Expr) -> Option<String> {
        let name = match e {
            Expr::Ident(n) => n,
            Expr::Call { callee, .. } => match callee.as_ref() {
                Expr::Ident(n) => n,
                _ => return None,
            },
            _ => return None,
        };

        self.variant_of.get(name).cloned()
    }

    /// A user function's C signature.
    fn fn_sig(&self, f: &FnDef) -> String {
        let (params, ret) = &self.fns[&f.name];
        let ps: Vec<String> = f
            .params
            .iter()
            .zip(params)
            .map(|(p, t)| format!("{} {}", c_type(t), cid(&p.name)))
            .collect();
        format!(
            "{} {}({})",
            c_type(ret),
            cid(&f.name),
            if ps.is_empty() {
                "void".into()
            } else {
                ps.join(", ")
            }
        )
    }

    /// A parameter a nested definition assigns gets a cell too, so the write is the one the enclosing body reads.
    fn move_params_into_cells(&mut self, env: &Env) {
        let taken: Vec<(String, CTy)> = env
            .iter()
            .filter(|(n, _)| self.cells.contains(n))
            .cloned()
            .collect();
        for (n, t) in taken {
            let ct = c_type(&t);
            self.push(&format!(
                "    {ct}* {c} = ({ct}*)maca_alloc(sizeof({ct})); *{c} = {};",
                cid(&n),
                c = cell_of(&n)
            ));
        }
    }

    fn emit_fn(&mut self, f: &FnDef) {
        self.appendable = ownership::appendable_names(f, self.fresh.defined());
        let (params, ret) = self.fns[&f.name].clone();
        self.cells = captured_writes(f.body.as_ref());
        self.appendable.retain(|n| !self.cells.contains(n.as_str()));
        self.returns = (
            if f.name == "main" {
                CTy::Int
            } else {
                ret.clone()
            },
            false,
        );
        let mut env: Env = f
            .params
            .iter()
            .zip(&params)
            .map(|(p, t)| (p.name.clone(), t.clone()))
            .collect();

        if f.name == "main" {
            self.push("int main(int _maca_argc, char** _maca_argv) {");
            self.push("    maca_init();");
            self.push("    maca_module_init();");
            self.move_params_into_cells(&env);
            if let Some((pname, _)) = f.params.first().map(|p| (p.name.clone(), ())) {
                env.push((pname.clone(), CTy::Arr(Box::new(CTy::Str))));
                let pc = cid(&pname);
                self.push(&format!(
                    "    {} {pc} = {}_new();",
                    arr_name(&CTy::Str),
                    arr_name(&CTy::Str)
                ));
                self.push(&format!(
                    "    for (int _i = 1; _i < _maca_argc; _i++) \
                     {}_push(&{pc}, _maca_argv[_i]);",
                    arr_name(&CTy::Str)
                ));
            }
            match &f.body {
                Some(FnBody::Block(stmts)) => {
                    self.block(&mut env, stmts, &Sink::Return(CTy::Int), 1)
                }
                Some(FnBody::Expr(e)) if is_control(e) => {
                    self.stmt_expr(&mut env, e, &Sink::Return(CTy::Int), 1);
                }
                Some(FnBody::Expr(e)) => {
                    let (c, _) = self.expr(&mut env, e, Some(&CTy::Int));
                    self.push(&format!("    return {c};"));
                }
                None => {}
            }
            self.push("    return 0;");
            self.push("}");
            return;
        }

        self.push(&format!("static {} {{", self.fn_sig(f)));
        self.move_params_into_cells(&env);
        match &f.body {
            Some(FnBody::Block(stmts)) => {
                let sink = if ret == CTy::Unit {
                    Sink::Discard
                } else {
                    Sink::Return(ret.clone())
                };
                self.block(&mut env, stmts, &sink, 1);
            }
            Some(FnBody::Expr(e)) if is_control(e) => {
                let sink = if ret == CTy::Unit {
                    Sink::Discard
                } else {
                    Sink::Return(ret.clone())
                };
                self.stmt_expr(&mut env, e, &sink, 1);
            }
            Some(FnBody::Expr(e)) => {
                let (c, _) = self.expr(&mut env, e, Some(&ret));
                self.push(&format!("    return {c};"));
            }
            None => {}
        }
        if ret == CTy::Unit && matches!(&f.body, Some(FnBody::Block(_))) {
            self.push("    return 0;");
        }
        self.push("}");
    }
}

type Env = Vec<(String, CTy)>;

fn lookup(env: &Env, n: &str) -> Option<CTy> {
    env.iter()
        .rev()
        .find(|(k, _)| k == n)
        .map(|(_, t)| t.clone())
}

/// Where the value of a block / control-flow expression goes.
#[derive(Clone)]
enum Sink {
    Discard,
    Return(CTy),
    Assign(String, CTy),
}

impl Sink {
    fn cty(&self) -> Option<&CTy> {
        match self {
            Sink::Discard => None,
            Sink::Return(t) | Sink::Assign(_, t) => Some(t),
        }
    }
}

impl<'a> Cx<'a> {
    fn indent(&mut self, n: usize) {
        for _ in 0..n {
            self.out.push_str("    ");
        }
    }

    /// Declare a local with nothing in it yet: a heap cell when a nested definition writes it, so both scopes write the one value.
    fn open_cell(&mut self, name: &str, ty: &CTy) {
        let t = c_type(ty);
        if self.cells.contains(name) {
            self.push(&format!(
                "{t}* {c} = ({t}*)maca_alloc(sizeof({t}));",
                c = cell_of(name)
            ));
        } else {
            self.push(&format!("{t} {};", cid(name)));
        }
    }

    /// How the C reaches a local: through its cell when a nested definition writes it, and by name otherwise.
    fn local_ref(&self, n: &str) -> String {
        if self.cells.contains(n) {
            format!("(*{})", cell_of(n))
        } else {
            cid(n)
        }
    }

    fn block(&mut self, env: &mut Env, stmts: &[Stmt], sink: &Sink, ind: usize) {
        let base = env.len();
        let escaping = escaping_names(stmts);
        let retained = self.fresh.retained(stmts, Tail::Flows);
        let kept_apart_from_result = self.fresh.retained(stmts, Tail::Read);
        let mut owned: Vec<(String, CTy)> = Vec::new();
        for (i, s) in stmts.iter().enumerate() {
            let last = i + 1 == stmts.len();
            match s {
                Stmt::Bind(b)
                    if matches!(&b.target, Expr::Ident(n)
                        if lookup(env, n).is_none() && !self.written_lets.contains(n)) =>
                {
                    if let Expr::Ident(name) = &b.target {
                        let ann = b.tys.first().map(|t| self.cty(t));
                        if is_control(&b.value) {
                            let ty = ann
                                .clone()
                                .unwrap_or_else(|| self.result_cty(env, &b.value));
                            self.indent(ind);
                            self.open_cell(name, &ty);
                            env.push((name.clone(), ty.clone()));
                            let slot = self.local_ref(name);
                            self.stmt_expr(env, &b.value, &Sink::Assign(slot, ty), ind);
                        } else if self.cells.contains(name) {
                            let (code, cty) = self.expr(env, &b.value, ann.as_ref());
                            let ty = ann.unwrap_or(cty);
                            self.indent(ind);
                            self.open_cell(name, &ty);
                            self.indent(ind);
                            self.push(&format!("{} = {code};", self.local_ref(name)));
                            env.push((name.clone(), ty));
                        } else {
                            let (code, cty) = self.expr(env, &b.value, ann.as_ref());
                            let ty = ann.unwrap_or(cty);
                            self.indent(ind);
                            self.push(&format!("{} {} = {code};", c_type(&ty), cid(name)));
                            let mine = match &ty {
                                CTy::Str => {
                                    if self.owns_str(name, stmts, &kept_apart_from_result) {
                                        self.owned_strs.insert(name.clone());
                                    }
                                    self.owns_str(name, stmts, &retained)
                                }
                                t => {
                                    owns_heap(t)
                                        && !escaping.contains(name)
                                        && !matches!(&b.value, Expr::Ident(_))
                                }
                            };
                            if mine {
                                owned.push((name.clone(), ty.clone()));
                            }
                            env.push((name.clone(), ty));
                        }
                    }
                }
                Stmt::Bind(b) => {
                    if let Expr::Ident(name) = &b.target {
                        if let Some(code) = self.accumulating_push(
                            env,
                            name,
                            &b.value,
                            stmts,
                            &kept_apart_from_result,
                        ) {
                            self.indent(ind);
                            self.push(&code);
                            continue;
                        }
                        if lookup(env, name).is_none() && self.written_lets.contains(name) {
                            let (slot, ty) = self.ident(env, name);
                            let (code, _) = self.expr(env, &b.value, Some(&ty));
                            self.indent(ind);
                            self.push(&format!("{slot} = {code};"));
                        } else if let Some(ty) = lookup(env, name) {
                            let (code, _) = self.expr(env, &b.value, Some(&ty));
                            self.indent(ind);
                            if self.owned_strs.contains(name) {
                                let old = self.temp();
                                self.push(&format!(
                                    "{{ maca_str {old} = {n}; {n} = {code}; \
                                     maca_drop_str({old}); }}",
                                    n = self.local_ref(name)
                                ));
                            } else {
                                self.push(&format!("{} = {code};", self.local_ref(name)));
                            }
                        }
                    } else if let Some((lv, ty)) = self.lvalue(env, &b.target) {
                        let (code, _) = self.expr(env, &b.value, ty.as_ref());
                        self.indent(ind);
                        self.push(&format!("{lv} = {code};"));
                    }
                }
                Stmt::Expr(e) if last && !matches!(sink, Sink::Discard) && !owned.is_empty() => {
                    let ty = sink.cty().cloned().unwrap_or(CTy::Unknown);
                    let held = self.temp();
                    self.indent(ind);
                    self.push(&format!("{} {held};", c_type(&ty)));
                    let hold = Sink::Assign(held.clone(), ty);
                    if is_control(e) {
                        self.stmt_expr(env, e, &hold, ind);
                    } else {
                        let (code, _) = self.expr(env, e, hold.cty());
                        self.emit_sink(&hold, &code, ind);
                    }
                    self.emit_drops(&owned, ind);
                    owned.clear();
                    self.emit_sink(sink, &held, ind);
                }
                Stmt::Expr(e) => {
                    let cur = if last { sink } else { &Sink::Discard };
                    if is_control(e) {
                        self.stmt_expr(env, e, cur, ind);
                    } else {
                        let (code, _) = self.expr(env, e, cur.cty());
                        self.emit_sink(cur, &code, ind);
                    }
                }
                Stmt::Fn(f) => {
                    if let Some((code, ty)) = self.nested_fn(env, f) {
                        self.indent(ind);
                        self.push(&format!("{} {} = {code};", c_type(&ty), cid(&f.name)));
                        env.push((f.name.clone(), ty));
                    }
                }
                _ => {}
            }
        }
        self.emit_drops(&owned, ind);
        for (name, _) in env.drain(base..) {
            self.owned_strs.remove(&name);
        }
    }

    /// `xs = xs.push(v)` and the other self-updates, lowered as a mutation, when that cannot be observed.
    fn accumulating_push(
        &mut self,
        env: &mut Env,
        name: &str,
        value: &Expr,
        stmts: &[Stmt],
        kept: &HashSet<String>,
    ) -> Option<String> {
        let (m, args) = ownership::self_update_method(name, value)?;
        let Some(CTy::Arr(elem)) = lookup(env, name) else {
            return None;
        };
        let _ = (kept, stmts);
        if !self.appendable.contains(name) {
            return None;
        }
        let an = arr_name(&elem);
        let xs = cid(name);
        let at = |cx: &mut Self, env: &mut Env| cx.expr(env, arg_expr(&args[0]), Some(&CTy::Int)).0;
        Some(match m {
            "push" => {
                let (v, _) = self.expr(env, arg_expr(&args[0]), Some(&elem));
                format!("{an}_push(&{xs}, {v});")
            }
            "remove" => {
                let i = at(self, env);
                format!("{an}_erase(&{xs}, {i});")
            }
            "set" => {
                let i = at(self, env);
                let (v, _) = self.expr(env, arg_expr(&args[1]), Some(&elem));
                format!("{an}_put(&{xs}, {i}, {v});")
            }
            _ => {
                let i = at(self, env);
                let (v, _) = self.expr(env, arg_expr(&args[1]), Some(&elem));
                format!("{an}_insert(&{xs}, {i}, {v});")
            }
        })
    }

    /// May this block release the string `name` holds?
    fn owns_str(&self, name: &str, stmts: &[Stmt], retained: &HashSet<String>) -> bool {
        if retained.contains(name) {
            return false;
        }
        let mut all_fresh = true;
        ownership::each_bind(stmts, &mut |n, value| {
            if n == name && !self.fresh.allocates(value) {
                all_fresh = false;
            }
        });
        all_fresh
    }

    /// Build one string out of `pieces`, giving back the ones this expression made.
    fn concat(&mut self, pieces: &[Piece]) -> String {
        let pieces: Vec<&Piece> = pieces.iter().filter(|p| p.code != "\"\"").collect();
        match pieces.as_slice() {
            [] => return "\"\"".into(),
            [one] if one.owned => return one.code.clone(),
            _ => {}
        }
        let n = pieces.len();
        if !pieces.iter().any(|p| p.releasable()) {
            let args: Vec<&str> = pieces.iter().map(|p| p.code.as_str()).collect();
            return format!("maca_concat_n({n}, {})", args.join(", "));
        }
        let (mut lets, mut args, mut drops) = (Vec::new(), Vec::new(), Vec::new());
        for p in pieces {
            if p.releasable() || !p.is_settled() {
                let t = self.temp();
                lets.push(format!("maca_str {t} = {};", p.code));
                if p.releasable() {
                    drops.push(format!("maca_drop_str({t});"));
                }
                args.push(t);
            } else {
                args.push(p.code.clone());
            }
        }
        let r = self.temp();
        format!(
            "({{ {} maca_str {r} = maca_concat_n({n}, {}); {} {r}; }})",
            lets.join(" "),
            args.join(", "),
            drops.join(" ")
        )
    }

    /// Release each owned local's buffer.
    fn emit_drops(&mut self, owned: &[(String, CTy)], ind: usize) {
        for (name, ty) in owned {
            let v = cid(name);
            self.indent(ind);
            match ty {
                CTy::Map(_) => self.push(&format!(
                    "maca_drop({v}.keys); maca_drop({v}.vals); maca_drop({v}.used);"
                )),
                CTy::Str => self.push(&format!("maca_drop_str({v});")),
                _ => self.push(&format!("maca_drop({v}.data);")),
            }
        }
    }

    /// Emit a simple (already-lowered) value against a sink.
    fn emit_sink(&mut self, sink: &Sink, code: &str, ind: usize) {
        self.indent(ind);
        match sink {
            Sink::Discard => self.push(&format!("{code};")),
            Sink::Return(_) => self.push(&format!("return {code};")),
            Sink::Assign(v, _) => self.push(&format!("{v} = {code};")),
        }
    }

    /// Infer the C type of a control-flow expression used in value position, by looking at a branch tail.
    fn result_cty(&mut self, env: &Env, e: &Expr) -> CTy {
        match e {
            Expr::If { then, els, .. } => {
                let t = tail_expr(then)
                    .map(|e| self.result_cty(env, e))
                    .unwrap_or(CTy::Unknown);
                if !matches!(t, CTy::Unknown) {
                    return t;
                }
                els.as_ref()
                    .and_then(|e| tail_expr(e))
                    .map(|e| self.result_cty(env, e))
                    .unwrap_or(CTy::Unknown)
            }
            Expr::Match { arms, .. } => arms
                .iter()
                .map(|a| self.result_cty(env, &a.body))
                .find(|t| !matches!(t, CTy::Unknown))
                .unwrap_or(CTy::Unknown),
            Expr::Block(stmts) => tail_expr(stmts)
                .map(|e| self.result_cty(env, e))
                .unwrap_or(CTy::Unit),
            Expr::For { .. }
            | Expr::While { .. }
            | Expr::Break
            | Expr::Continue
            | Expr::Return(_) => CTy::Unit,
            leaf => {
                let save = self.out.len();
                let tmp = self.tmp;
                let (_, t) = self.expr(&mut env.clone(), leaf, None);
                self.out.truncate(save);
                self.tmp = tmp;
                t
            }
        }
    }

    /// Emit a control-flow expression (for/match/if/block), routing its value to `sink` (Discard in statement position, Return/Assign in value position).
    fn stmt_expr(&mut self, env: &mut Env, e: &Expr, sink: &Sink, ind: usize) {
        match e {
            Expr::Block(stmts) => self.block(env, stmts, sink, ind),
            Expr::For { pat, iter, body } if matches!(&**iter, Expr::Range { .. }) => {
                let Expr::Range { lo, hi } = &**iter else {
                    unreachable!()
                };
                let (lc, _) = self.expr(env, lo, Some(&CTy::Int));
                let (hc, _) = self.expr(env, hi, Some(&CTy::Int));
                let var = if let Pattern::Bind(n) = pat {
                    n.clone()
                } else {
                    self.temp()
                };
                let hv = self.temp();
                self.indent(ind);
                self.push(&format!(
                    "{{ int64_t {hv} = {hc}; for (int64_t {var} = {lc}; {var} < {hv}; {var}++) {{"
                ));
                let mut env2 = env.clone();
                env2.push((var, CTy::Int));
                self.block(&mut env2, body, &Sink::Discard, ind + 1);
                self.indent(ind);
                self.push("} }");
            }
            Expr::For { pat, iter, body } => {
                let (ic, ity) = self.expr(env, iter, None);
                let elem = match ity {
                    CTy::Arr(e) => *e,
                    _ => CTy::Unknown,
                };
                let it = self.temp();
                let idx = self.temp();
                let var = if let Pattern::Bind(n) = pat {
                    n.clone()
                } else {
                    self.temp()
                };
                let an = arr_name(&elem);
                self.indent(ind);
                self.push(&format!(
                    "{{ {an} {it} = {ic}; for (int64_t {idx} = 0; {idx} < {it}.len; {idx}++) {{"
                ));
                self.indent(ind + 1);
                self.push(&format!("{} {var} = {it}.data[{idx}];", c_type(&elem)));
                let mut env2 = env.clone();
                env2.push((var, elem));
                self.block(&mut env2, body, &Sink::Discard, ind + 1);
                self.indent(ind);
                self.push("} }");
            }
            Expr::While { cond, body } => {
                let (cc, _) = self.expr(env, cond, None);
                self.indent(ind);
                self.push(&format!("while ({cc}) {{"));
                let mut e2 = env.clone();
                self.block(&mut e2, body, &Sink::Discard, ind + 1);
                self.indent(ind);
                self.push("}");
            }
            Expr::Break => {
                self.indent(ind);
                self.push("break;");
            }
            Expr::Continue => {
                self.indent(ind);
                self.push("continue;");
            }
            Expr::Return(v) => {
                let (rt, boxed) = self.returns.clone();
                let code = match v {
                    Some(x) => {
                        let (c, _) = self.expr(env, x, Some(&rt));
                        if boxed { box_i64(&c, &rt) } else { c }
                    }
                    None => "0".into(),
                };
                self.indent(ind);
                self.push(&format!("return {code};"));
            }
            Expr::Match { scrut, arms } => self.match_stmt(env, scrut, arms, sink, ind),
            Expr::If { cond, then, els } => {
                let (cc, _) = self.expr(env, cond, None);
                self.indent(ind);
                self.push(&format!("if ({cc}) {{"));
                let mut e1 = env.clone();
                self.block(&mut e1, then, sink, ind + 1);
                if let Some(e) = els {
                    self.indent(ind);
                    self.push("} else {");
                    let mut e2 = env.clone();
                    self.block(&mut e2, e, sink, ind + 1);
                }
                self.indent(ind);
                self.push("}");
            }
            _ => {
                let (code, _) = self.expr(env, e, sink.cty());
                self.emit_sink(sink, &code, ind);
            }
        }
    }

    fn match_stmt(&mut self, env: &mut Env, scrut: &Expr, arms: &[Arm], sink: &Sink, ind: usize) {
        let (sc, sty) = self.expr(env, scrut, None);
        let sv = self.temp();
        self.indent(ind);
        self.push(&format!("{{ {} {sv} = {sc};", c_type(&sty)));
        let elem = match &sty {
            CTy::Arr(e) => (**e).clone(),
            _ => CTy::Unknown,
        };
        if arms.iter().any(|a| a.guard.is_some()) {
            let end = format!("_m{}", self.temp());
            for arm in arms {
                let (cond, binds) = self.pattern_cond(&sv, &sty, &elem, &arm.pat);
                self.indent(ind);
                self.push(&format!("if ({cond}) {{"));
                let mut env2 = env.clone();
                for (bname, bcode, bty) in binds {
                    self.indent(ind + 1);
                    self.push(&bcode);
                    env2.push((bname, bty));
                }
                let body_ind = if let Some(g) = &arm.guard {
                    let (gc, _) = self.expr(&mut env2, g, None);
                    self.indent(ind + 1);
                    self.push(&format!("if ({gc}) {{"));
                    ind + 2
                } else {
                    ind + 1
                };
                self.stmt_expr(&mut env2, body_as_block(&arm.body), sink, body_ind);
                self.indent(body_ind);
                self.push(&format!("goto {end};"));
                if arm.guard.is_some() {
                    self.indent(ind + 1);
                    self.push("}");
                }
                self.indent(ind);
                self.push("}");
            }
            self.indent(ind);
            self.push(&format!("{end}: ;"));
            self.indent(ind);
            self.push("}");
            return;
        }
        for (i, arm) in arms.iter().enumerate() {
            let (cond, binds) = self.pattern_cond(&sv, &sty, &elem, &arm.pat);
            let kw = if i == 0 { "if" } else { "else if" };
            self.indent(ind);
            if cond == "1" && i > 0 {
                self.push("else {");
            } else {
                self.push(&format!("{kw} ({cond}) {{"));
            }
            let mut env2 = env.clone();
            for (bname, bcode, bty) in binds {
                self.indent(ind + 1);
                self.push(&bcode);
                env2.push((bname, bty));
            }
            self.stmt_expr(&mut env2, body_as_block(&arm.body), sink, ind + 1);
            self.indent(ind);
            self.push("}");
        }
        self.indent(ind);
        self.push("}");
    }

    /// (condition, [(name, decl_stmt, type)]) for matching `sv` against a pattern.
    fn pattern_cond(
        &mut self,
        sv: &str,
        sty: &CTy,
        elem: &CTy,
        p: &Pattern,
    ) -> (String, Vec<(String, String, CTy)>) {
        match p {
            Pattern::Wild => ("1".into(), vec![]),
            Pattern::Int(n) => (format!("{sv} == {n}"), vec![]),
            Pattern::Float(f) => (format!("{sv} == {f}"), vec![]),
            Pattern::Bool(b) => (
                format!("{sv} == {}", if *b { "true" } else { "false" }),
                vec![],
            ),
            Pattern::Str(lit) if matches!(sty, CTy::Str) => {
                (format!("maca_str_eq({sv}, {})", c_str(lit)), vec![])
            }
            Pattern::Bind(n) | Pattern::Ctor { name: n, args: _ } if matches!(sty, CTy::Sum(s) if self.sums.get(s).is_some_and(|vs| vs.iter().any(|v| v == n))) =>
            {
                let CTy::Sum(s) = sty else { unreachable!() };
                if self.is_tagged(s) {
                    let cond = format!("{sv}.tag == {s}_tag_{n}");
                    let mut binds = Vec::new();
                    if let Pattern::Ctor { args, .. } = p {
                        let ptys = self.variant_payloads.get(n).cloned().unwrap_or_default();
                        for (i, a) in args.iter().enumerate() {
                            if let Pattern::Bind(bn) = a {
                                let bty = ptys.get(i).cloned().unwrap_or(CTy::Unknown);
                                let deref = if self.is_boxed(s, &bty) { "*" } else { "" };
                                binds.push((
                                    bn.clone(),
                                    format!(
                                        "{} {} = {deref}{sv}.as.{n}._{i};",
                                        c_type(&bty),
                                        cid(bn)
                                    ),
                                    bty,
                                ));
                            }
                        }
                    }
                    (cond, binds)
                } else {
                    (format!("{sv} == {s}_{n}"), vec![])
                }
            }
            Pattern::Record(fields) if matches!(sty, CTy::Rec(_)) => {
                let CTy::Rec(rname) = sty else { unreachable!() };
                let decl = self.records.get(rname).cloned().unwrap_or_default();
                let mut binds = Vec::new();
                for (fname, sub) in fields {
                    let fty = decl
                        .iter()
                        .find(|(n, _)| n == fname)
                        .map(|(_, t)| t.clone())
                        .unwrap_or(CTy::Unknown);
                    let bn = match sub {
                        None => Some(fname.clone()),
                        Some(Pattern::Bind(b)) => Some(b.clone()),
                        _ => None,
                    };
                    if let Some(bn) = bn {
                        binds.push((
                            bn.clone(),
                            format!("{} {} = {sv}.{};", c_type(&fty), cid(&bn), cid(fname)),
                            fty,
                        ));
                    }
                }
                ("1".into(), binds)
            }
            Pattern::Bind(n) => (
                "1".into(),
                vec![(
                    n.clone(),
                    format!("{} {} = {sv};", c_type(sty), cid(n)),
                    sty.clone(),
                )],
            ),
            Pattern::Str(lit) if matches!(sty, CTy::Arr(_)) => (
                format!("{sv}.len == 1 && maca_str_eq({sv}.data[0], {})", c_str(lit)),
                vec![],
            ),
            Pattern::List { elems, rest } => {
                let n = elems.len();
                let mut conds = vec![if rest.is_some() {
                    format!("{sv}.len >= {n}")
                } else {
                    format!("{sv}.len == {n}")
                }];
                let mut binds = Vec::new();
                for (i, ep) in elems.iter().enumerate() {
                    match ep {
                        Pattern::Str(lit) => {
                            conds.push(format!("maca_str_eq({sv}.data[{i}], {})", c_str(lit)))
                        }
                        Pattern::Bind(bn) => binds.push((
                            bn.clone(),
                            format!("{} {bn} = {sv}.data[{i}];", c_type(elem)),
                            elem.clone(),
                        )),
                        _ => {}
                    }
                }
                if let Some(r) = rest
                    && let Pattern::Bind(rn) = &**r
                {
                    binds.push((
                        rn.clone(),
                        format!("{an} {rn} = {an}_slice({sv}, {n});", an = arr_name(elem)),
                        CTy::Arr(Box::new(elem.clone())),
                    ));
                }
                (conds.join(" && "), binds)
            }
            Pattern::Or(alts) => {
                let mut conds = Vec::new();
                let mut binds = Vec::new();
                for (i, a) in alts.iter().enumerate() {
                    let (c, b) = self.pattern_cond(sv, sty, elem, a);
                    conds.push(c);
                    if i == 0 {
                        binds = b;
                    }
                }
                if conds.is_empty() {
                    ("1".into(), binds)
                } else {
                    (format!("({})", conds.join(" || ")), binds)
                }
            }
            _ => ("1".into(), vec![]),
        }
    }

    /// Lower `e`, and leave behind the flattened pieces if it was a concatenation.
    fn expr(&mut self, env: &mut Env, e: &Expr, expected: Option<&CTy>) -> (String, CTy) {
        let out = self.expr_inner(env, e, expected);
        if !matches!(
            e,
            Expr::Binary {
                op: BinOp::Concat,
                ..
            }
        ) {
            self.concat_pieces = None;
        }
        out
    }

    fn expr_inner(&mut self, env: &mut Env, e: &Expr, expected: Option<&CTy>) -> (String, CTy) {
        match e {
            Expr::Int(n) => (n.to_string(), CTy::Int),
            Expr::Float(f) => (format!("{f:?}"), CTy::Float),
            Expr::Bool(b) => ((if *b { "true" } else { "false" }).into(), CTy::Bool),
            Expr::Unit => ("0".into(), CTy::Unit),
            Expr::Str(parts) => (self.interp(env, parts), CTy::Str),
            Expr::Path(p) => (c_str(p), CTy::Str),
            Expr::Ident(n) => self.ident(env, n),
            Expr::Ctor { name, fields } => self.ctor(env, name, fields),
            Expr::Record(fields) => {
                let name = self
                    .expected_record(expected, fields)
                    .unwrap_or_else(|| anon_record_name(&self.anon_shape(fields)));
                self.ctor(env, &name, fields)
            }
            Expr::List(es) => self.list(env, es, expected),
            Expr::Call { callee, args } => self.call(env, callee, args, expected),
            Expr::Field { base, name } => self.field(env, base, name),
            Expr::Index { base, index } => self.index(env, base, index),
            Expr::Range { lo, hi } => {
                let (lc, _) = self.expr(env, lo, Some(&CTy::Int));
                let (hc, _) = self.expr(env, hi, Some(&CTy::Int));
                let an = arr_name(&CTy::Int);
                let lv = self.temp();
                let hv = self.temp();
                let n = self.temp();
                let i = self.temp();
                (
                    format!(
                        "({{ int64_t {lv} = {lc}, {hv} = {hc}; {an} _a = {an}_new(); \
                         int64_t {n} = {hv} > {lv} ? {hv} - {lv} : 0; \
                         if ({n} > 0) {{ _a.data = (int64_t*)maca_realloc(0, (size_t){n} * sizeof(int64_t)); \
                         _a.len = {n}; _a.cap = {n}; \
                         for (int64_t {i} = 0; {i} < {n}; {i}++) _a.data[{i}] = {lv} + {i}; }} _a; }})"
                    ),
                    CTy::Arr(Box::new(CTy::Int)),
                )
            }
            Expr::Unary { op, expr } => {
                let (c, t) = self.expr(env, expr, None);
                let op = match op {
                    UnOp::Neg => "-",
                    UnOp::Not => "!",
                };
                (format!("({op}{c})"), t)
            }
            Expr::Binary { op, lhs, rhs } => self.binary(env, *op, lhs, rhs),
            Expr::Ternary { cond, then, els } => {
                let (c, _) = self.expr(env, cond, None);
                let (t, tt) = self.expr(env, then, expected);
                let (e2, _) = self.expr(env, els, expected);
                (
                    format!("({c} ? {t} : {e2})"),
                    expected.cloned().unwrap_or(tt),
                )
            }
            Expr::Try(x) => self.expr(env, x, expected),
            Expr::Fail(msg) => {
                let (mc, _) = self.expr(env, msg, Some(&CTy::Str));
                let ty = expected.cloned().unwrap_or(CTy::Unit);
                let zero = match &ty {
                    CTy::Rec(_) | CTy::Sum(_) | CTy::Arr(_) | CTy::Map(_) => {
                        format!("({}){{0}}", c_type(&ty))
                    }
                    _ => "0".into(),
                };
                (format!("(maca_fail({mc}), {zero})"), ty)
            }
            Expr::Lambda { params, body, .. } => self.emit_lambda(env, params, body),
            Expr::With { base, fields } => self.with_update(env, base, fields),
            Expr::Reify(x) => {
                let jb = self.temp();
                let r = self.temp();
                let (xc, _) = self.expr(env, x, None);
                (
                    format!(
                        "({{ jmp_buf* {jb} = maca_try_push(); maca_str {r}; \
                         if (setjmp(*{jb}) == 0) {{ (void)({xc}); maca_try_pop(); {r} = \"\"; }} \
                         else {{ {r} = maca_last_fail(); }} {r}; }})"
                    ),
                    CTy::Str,
                )
            }
            Expr::Spawn(inner) => match &**inner {
                Expr::Call { callee, args } if matches!(&**callee, Expr::Ident(_)) => {
                    let Expr::Ident(f) = &**callee else {
                        unreachable!()
                    };
                    if args.len() > 2 {
                        self.problem(format!(
                            "`spawn {f}(…)` takes at most two arguments; \
                             pass the rest in a record"
                        ));
                    }
                    if self.variadics.contains_key(f) {
                        self.problem(format!(
                            "`spawn {f}(…)` cannot take a variadic: a task takes \
                             whole numbers and strings. Wrap it in a function \
                             that does."
                        ));
                    }
                    let typed: Vec<(String, CTy)> =
                        args.iter().map(|x| self.arg_typed(env, x)).collect();
                    for (_, t) in &typed {
                        let what = match t {
                            CTy::Closure(_) | CTy::Closure2(_) => "a function value",
                            CTy::Float => "a float",
                            CTy::Rec(_) => "a record",
                            _ => continue,
                        };
                        self.problem(format!(
                            "`spawn {f}(…)` cannot take {what}: a task takes \
                             whole numbers and strings. Wrap it in a function \
                             that does."
                        ));
                    }
                    let a: Vec<String> = typed.into_iter().map(|(c, _)| c).collect();
                    let code = match a.len() {
                        0 => format!("maca_spawn((maca_task_fn){}, 0)", cid(f)),
                        1 => format!("maca_spawn((maca_task_fn){}, (int64_t)({}))", cid(f), a[0]),
                        _ => format!(
                            "maca_spawn2((maca_task_fn2){}, (int64_t)({}), (int64_t)({}))",
                            cid(f),
                            a[0],
                            a[1]
                        ),
                    };
                    (code, CTy::Future)
                }
                _ => {
                    self.problem("`spawn` expects a direct function call, e.g. `spawn f(x)`");
                    ("0 /* unsupported: spawn */".into(), CTy::Unknown)
                }
            },
            Expr::Await(inner) => {
                let (fc, _) = self.expr(env, inner, None);
                (format!("maca_await({fc})"), CTy::Int)
            }
            other
                if is_control(other)
                    && !matches!(
                        other,
                        Expr::For { .. } | Expr::While { .. } | Expr::Break | Expr::Continue
                    ) =>
            {
                let ty = expected
                    .cloned()
                    .unwrap_or_else(|| self.result_cty(env, other));
                let tmp = self.temp();
                let saved = std::mem::take(&mut self.out);
                self.push(&format!("{} {tmp};", c_type(&ty)));
                let mut inner = env.clone();
                self.stmt_expr(&mut inner, other, &Sink::Assign(tmp.clone(), ty.clone()), 0);
                let body = std::mem::replace(&mut self.out, saved);
                (format!("({{ {} {tmp}; }})", body.trim()), ty)
            }
            other => {
                self.problem(format!(
                    "expression not supported by the native backend: {other:?}"
                ));
                ("0 /* unsupported */".into(), CTy::Unknown)
            }
        }
    }

    fn ident(&mut self, env: &Env, n: &str) -> (String, CTy) {
        if let Some(t) = lookup(env, n) {
            return (self.local_ref(n), t);
        }
        if let Some(sum) = self.variant_of.get(n).cloned() {
            if self.is_tagged(&sum) {
                return (format!("{sum}_{n}()"), CTy::Sum(sum));
            }
            return (format!("{sum}_{n}"), CTy::Sum(sum));
        }
        if self.let_names.contains(n) {
            let found = self.lets.iter().find(|(name, _, _)| name == n).cloned();
            let ty = found
                .map(|(_, t, init)| self.let_ty(&t, &init))
                .unwrap_or(CTy::Unknown);
            let call = match self.written_lets.contains(n) {
                true => format!("mv_{n}"),
                false => format!("mv_{n}()"),
            };
            return (call, ty);
        }
        if self.fns.contains_key(n) {
            return self.fn_value_closure(n);
        }
        (cid(n), CTy::Unknown)
    }

    /// A `maca_closure` that calls top-level fn `name`, boxing its argument(s) and result across the uniform closure ABI.
    fn fn_value_closure(&mut self, name: &str) -> (String, CTy) {
        let (params, ret) = self.fns[name].clone();
        let arity = params.len();
        if self.variadics.contains_key(name) {
            self.problem(format!(
                "`{name}` is variadic, so it cannot be used as a function value"
            ));
        }
        let thunk = format!("{}__fnval", cid(name));
        if self.fn_thunks.insert(name.to_string()) {
            let sig = if arity >= 2 {
                format!("static int64_t {thunk}(void* _e, int64_t _a0, int64_t _a1)")
            } else {
                format!("static int64_t {thunk}(void* _e, int64_t _a0)")
            };
            let call_args = if arity >= 2 {
                format!(
                    "{}, {}",
                    unbox_i64("_a0", &params[0]),
                    unbox_i64("_a1", &params[1])
                )
            } else if arity == 1 {
                unbox_i64("_a0", &params[0])
            } else {
                String::new()
            };
            let call = format!("{}({call_args})", cid(name));
            self.hoisted_decls.push(format!("{sig};"));
            self.hoisted_defs.push(format!(
                "{sig} {{ (void)_e; return {}; }}",
                box_i64(&call, &ret)
            ));
        }
        let (ctype, cast) = if arity >= 2 {
            ("maca_closure2", "(int64_t(*)(void*,int64_t,int64_t))")
        } else {
            ("maca_closure", "(int64_t(*)(void*,int64_t))")
        };
        (
            format!("(({ctype}){{ {cast}{thunk}, NULL }})"),
            closure_ty(arity, ret),
        )
    }

    /// Lower a lambda in a function-value position.
    fn emit_lambda(&mut self, env: &Env, params: &[Param], body: &Expr) -> (String, CTy) {
        let hint = self.lambda_hint.clone();
        let ptys: Vec<CTy> = params
            .iter()
            .enumerate()
            .map(|(i, p)| {
                p.ty.as_ref()
                    .map(|t| self.cty(t))
                    .or_else(|| hint.as_ref().and_then(|h| h.get(i).cloned()))
                    .or_else(|| self.param_ty_from_use(&p.name, body))
                    .or_else(|| lambda_param_ty(&p.name, body))
                    .unwrap_or(CTy::Int)
            })
            .collect();
        let (val, ret) = self.emit_closure(env, params, body, &ptys);
        (val, closure_ty(params.len(), ret))
    }

    /// The type a lambda parameter must have, read off the place it is passed.
    fn param_ty_from_use(&self, name: &str, body: &Expr) -> Option<CTy> {
        let mut found = None;
        self.scan_passed_to(name, body, &mut found);
        found
    }

    fn scan_passed_to(&self, name: &str, e: &Expr, found: &mut Option<CTy>) {
        if let Expr::Call { callee, args } = e
            && let Expr::Ident(f) = &**callee
            && let Some((ptys, _)) = self.fns.get(f)
        {
            for (i, a) in args.iter().enumerate() {
                let Arg::Pos(Expr::Ident(n)) = a else {
                    continue;
                };
                if n != name {
                    continue;
                }
                let slot = match self.variadics.get(f) {
                    Some(&at) if i >= at => match ptys.get(at) {
                        Some(CTy::Arr(e)) => Some((**e).clone()),
                        _ => None,
                    },
                    _ => ptys.get(i).cloned(),
                };
                if let Some(t) = slot
                    && !matches!(t, CTy::Unknown | CTy::Unit)
                {
                    *found = Some(t);
                    return;
                }
            }
        }
        if found.is_some() {
            return;
        }
        if let Expr::Lambda { params, .. } = e
            && params.iter().any(|p| p.name == name)
        {
            return;
        }
        walk_children(e, &mut |c| self.scan_passed_to(name, c, found));
    }

    /// The type a lambda handed to `callee` at position `index` is called with.
    fn callee_param_ty(&self, callee: &str, index: usize, fuel: usize) -> Option<CTy> {
        if fuel == 0 {
            return None;
        }
        let f = self.m.items.iter().find_map(|it| match it {
            Stmt::Fn(f) if f.name == callee => Some(f),
            _ => None,
        })?;
        let param = f.params.get(index)?;
        if param.variadic {
            return None;
        }
        let pname = &param.name;
        let body = f.body.as_ref()?;

        let mut env: Vec<(String, CTy)> = f
            .params
            .iter()
            .filter_map(|p| Some((p.name.clone(), self.cty(p.ty.as_ref()?))))
            .collect();
        if let FnBody::Block(stmts) = body {
            for st in stmts {
                let Stmt::Bind(b) = st else { continue };
                let (Expr::Ident(n), Expr::Call { callee: c, .. }) = (&b.target, &b.value) else {
                    continue;
                };
                if let Expr::Ident(g) = &**c
                    && let Some((_, ret)) = self.fns.get(g)
                    && !matches!(ret, CTy::Unknown | CTy::Unit)
                {
                    env.push((n.clone(), ret.clone()));
                }
            }
        }

        let mut direct = None;
        let mut forwarded = None;
        body_exprs(body, &mut |e| {
            self.scan_called_or_forwarded(pname, e, &env, &mut direct, &mut forwarded)
        });
        direct.or_else(|| forwarded.and_then(|(g, j)| self.callee_param_ty(&g, j, fuel - 1)))
    }

    fn scan_called_or_forwarded(
        &self,
        pname: &str,
        e: &Expr,
        env: &[(String, CTy)],
        direct: &mut Option<CTy>,
        forwarded: &mut Option<(String, usize)>,
    ) {
        if let Expr::Call { callee, args } = e {
            match &**callee {
                Expr::Ident(n) if n == pname => {
                    if let Some(Arg::Pos(Expr::Ident(a))) = args.first()
                        && let Some((_, t)) = env.iter().find(|(k, _)| k == a)
                    {
                        *direct = Some(t.clone());
                    }
                }
                Expr::Ident(g) if forwarded.is_none() => {
                    for (i, a) in args.iter().enumerate() {
                        if matches!(a, Arg::Pos(Expr::Ident(n)) if n == pname) {
                            *forwarded = Some((g.clone(), i));
                        }
                    }
                }
                _ => {}
            }
        }
        if direct.is_none() {
            walk_children(e, &mut |c| {
                self.scan_called_or_forwarded(pname, c, env, direct, forwarded)
            });
        }
    }

    /// Lower a lambda to a `maca_closure`.
    fn emit_closure(
        &mut self,
        env: &Env,
        params: &[Param],
        body: &Expr,
        param_tys: &[CTy],
    ) -> (String, CTy) {
        let saved_appendable = std::mem::take(&mut self.appendable);
        let saved_returns = self.returns.clone();

        let mut bound: HashSet<String> = params.iter().map(|p| p.name.clone()).collect();
        let mut refs = HashSet::new();
        free_vars(body, &bound, &mut refs);
        let mut written = HashSet::new();
        let outer_cells: HashSet<String> = self
            .cells
            .iter()
            .filter(|n| !bound.contains(*n))
            .cloned()
            .collect();
        writes_under_nesting(body, &outer_cells, true, &mut written);
        refs.extend(written.into_iter().filter(|n| lookup(env, n).is_some()));
        bound.clear();
        let mut caps: Vec<(String, CTy)> = refs
            .into_iter()
            .filter(|n| lookup(env, n).is_some() || !self.is_known_global(n))
            .map(|n| {
                let t = self.cap_ty(env, &n);
                (n, t)
            })
            .collect();
        caps.sort_by(|a, b| a.0.cmp(&b.0));
        let shared: HashSet<String> = caps
            .iter()
            .map(|(n, _)| n.clone())
            .filter(|n| self.cells.contains(n))
            .collect();

        let id = self.lambda_count;
        self.lambda_count += 1;
        let fname = format!("_lam{id}");
        let ename = format!("_lam{id}_env");
        let two = params.len() >= 2;
        let field_ty = |n: &str, t: &CTy| {
            if shared.contains(n) {
                format!("{}*", c_type(t))
            } else {
                c_type(t)
            }
        };

        if !caps.is_empty() {
            let fields = caps
                .iter()
                .map(|(n, t)| format!("{} {};", field_ty(n, t), cell_or_name(&shared, n)))
                .collect::<Vec<_>>()
                .join(" ");
            self.hoisted_decls
                .push(format!("typedef struct {{ {fields} }} {ename};"));
        }
        let sig = if two {
            format!("static int64_t {fname}(void* _envp, int64_t _a0, int64_t _a1)")
        } else {
            format!("static int64_t {fname}(void* _envp, int64_t _a0)")
        };
        self.hoisted_decls.push(format!("{sig};"));

        let mut inner_cells = shared.clone();
        inner_cells.extend(writes_below(body));
        let saved_cells = std::mem::replace(&mut self.cells, inner_cells);

        let saved = std::mem::take(&mut self.out);
        self.push(&format!("{sig} {{"));
        if params.is_empty() {
            self.push("    (void)_a0;");
        }
        let mut lenv: Env = Vec::new();
        if caps.is_empty() {
            self.push("    (void)_envp;");
        } else {
            self.push(&format!("    {ename}* _e = ({ename}*)_envp;"));
            for (n, t) in &caps {
                let slot = cell_or_name(&shared, n);
                self.push(&format!("    {} {slot} = _e->{slot};", field_ty(n, t)));
                lenv.push((n.clone(), t.clone()));
            }
        }
        for (i, p) in params.iter().enumerate() {
            let pt = param_tys.get(i).cloned().unwrap_or(CTy::Int);
            let arg = format!("_a{i}");
            self.push(&format!(
                "    {} {} = {};",
                c_type(&pt),
                cid(&p.name),
                unbox_i64(&arg, &pt)
            ));
            lenv.push((p.name.clone(), pt));
        }
        let declared = self.closure_ret.take();
        let ret = if is_control(body) {
            let ret = declared.unwrap_or_else(|| self.result_cty(&lenv, body));
            self.returns = (ret.clone(), true);
            self.push(&format!("    {} _r;", c_type(&ret)));
            self.stmt_expr(&mut lenv, body, &Sink::Assign("_r".into(), ret.clone()), 1);
            self.push(&format!("    return {};", box_i64("_r", &ret)));
            ret
        } else {
            let (bc, bt) = self.expr(&mut lenv, body, None);
            let ret = declared.unwrap_or(if bt == CTy::Unknown { CTy::Int } else { bt });
            self.push(&format!("    return {};", box_i64(&bc, &ret)));
            ret
        };
        self.push("}");
        let def = std::mem::replace(&mut self.out, saved);
        self.hoisted_defs.push(def);
        self.cells = saved_cells;

        let (ctype, cast) = if two {
            ("maca_closure2", "(int64_t(*)(void*,int64_t,int64_t))")
        } else {
            ("maca_closure", "(int64_t(*)(void*,int64_t))")
        };
        let val = if caps.is_empty() {
            format!("(({ctype}){{ {cast}{fname}, NULL }})")
        } else {
            let mut fills = String::new();
            for (n, _) in &caps {
                let slot = cell_or_name(&shared, n);
                let outer = if shared.contains(n) {
                    cell_of(n)
                } else {
                    self.ident(env, n).0
                };
                fills.push_str(&format!("_e->{slot} = {outer}; "));
            }
            format!(
                "({{ {ename}* _e = ({ename}*)maca_alloc(sizeof({ename})); {fills}({ctype}){{ {cast}{fname}, _e }}; }})"
            )
        };
        self.appendable = saved_appendable;
        self.returns = saved_returns;
        (val, ret)
    }

    /// A named definition inside a block: a closure bound to its name, sharing the scope that encloses it.
    fn nested_fn(&mut self, env: &Env, f: &FnDef) -> Option<(String, CTy)> {
        let body = match f.body.as_ref()? {
            FnBody::Block(stmts) => Expr::Block(stmts.clone()),
            FnBody::Expr(e) => (**e).clone(),
        };
        if f.params.len() > 2 {
            self.problem(format!(
                "`{}` is defined inside another function and takes {} arguments; \
                 a nested definition takes at most two, so lift it to the top level",
                f.name,
                f.params.len()
            ));
            return None;
        }
        if f.params.iter().any(|p| p.variadic) {
            self.problem(format!(
                "`{}` is defined inside another function, so it cannot be variadic; \
                 lift it to the top level",
                f.name
            ));
            return None;
        }
        let ptys: Vec<CTy> = f
            .params
            .iter()
            .map(|p| {
                p.ty.as_ref()
                    .map(|t| self.cty(t))
                    .or_else(|| self.param_ty_from_use(&p.name, &body))
                    .or_else(|| lambda_param_ty(&p.name, &body))
                    .unwrap_or(CTy::Int)
            })
            .collect();
        self.closure_ret = f.ret.as_ref().map(|t| self.cty(t));
        let (val, ret) = self.emit_closure(env, &f.params, &body, &ptys);
        self.closure_ret = None;
        if f.params.is_empty() {
            self.nullary.insert(f.name.clone());
        } else {
            self.nullary.remove(&f.name);
        }
        Some((val, closure_ty(f.params.len(), ret)))
    }

    /// Emit one monomorphized copy of a generic function for a concrete tuple of argument types (mangled name, concrete param/ret types).
    fn emit_specialization(&mut self, name: &str, arg_ctys: &[CTy]) {
        let genf = self.generics[name].clone();
        let subst = self.build_subst(&genf, arg_ctys);
        let ret = genf
            .ret
            .as_ref()
            .map_or(CTy::Unit, |t| self.subst_cty(t, &subst));
        let mangled = mangle_name(name, arg_ctys);
        let param_ctys: Vec<CTy> = genf
            .params
            .iter()
            .enumerate()
            .map(|(i, _)| arg_ctys.get(i).cloned().unwrap_or(CTy::Unknown))
            .collect();
        let mut env: Env = genf
            .params
            .iter()
            .zip(&param_ctys)
            .map(|(p, t)| (p.name.clone(), t.clone()))
            .collect();
        let ps: Vec<String> = genf
            .params
            .iter()
            .zip(&param_ctys)
            .map(|(p, t)| format!("{} {}", c_type(t), cid(&p.name)))
            .collect();
        let sig = format!(
            "{} {mangled}({})",
            c_type(&ret),
            if ps.is_empty() {
                "void".into()
            } else {
                ps.join(", ")
            }
        );
        self.hoisted_decls.push(format!("static {sig};"));
        let saved = std::mem::take(&mut self.out);
        let saved_subst = std::mem::replace(&mut self.type_subst, subst);
        let saved_appendable = std::mem::replace(
            &mut self.appendable,
            ownership::appendable_names(&genf, self.fresh.defined()),
        );
        let saved_cells = std::mem::replace(&mut self.cells, captured_writes(genf.body.as_ref()));
        self.appendable.retain(|n| !self.cells.contains(n.as_str()));
        let saved_returns = std::mem::replace(&mut self.returns, (ret.clone(), false));
        if let Some(FnBody::Block(stmts)) = &genf.body {
            self.note_local_containers(&stmts.clone());
        }
        self.push(&format!("static {sig} {{"));
        self.move_params_into_cells(&env);
        match &genf.body {
            Some(FnBody::Block(stmts)) => {
                let sink = if ret == CTy::Unit {
                    Sink::Discard
                } else {
                    Sink::Return(ret.clone())
                };
                self.block(&mut env, stmts, &sink, 1);
            }
            Some(FnBody::Expr(e)) => {
                let (c, _) = self.expr(&mut env, e, Some(&ret));
                self.push(&format!("    return {c};"));
            }
            None => {}
        }
        if ret == CTy::Unit && matches!(&genf.body, Some(FnBody::Block(_))) {
            self.push("    return 0;");
        }
        self.push("}");
        self.type_subst = saved_subst;
        self.appendable = saved_appendable;
        self.cells = saved_cells;
        self.returns = saved_returns;
        let def = std::mem::replace(&mut self.out, saved);
        self.hoisted_defs.push(def);
    }

    /// type-variable name → concrete `CTy`, from a generic fn's params vs. the concrete argument types at a call.
    fn build_subst(&self, genf: &FnDef, arg_ctys: &[CTy]) -> HashMap<String, CTy> {
        let mut m = HashMap::new();
        for (p, cty) in genf.params.iter().zip(arg_ctys) {
            if let Some(t) = &p.ty {
                let against = match (p.variadic, cty) {
                    (true, CTy::Arr(e)) => e,
                    _ => cty,
                };
                bind_vars(t, against, &mut m);
            }
        }
        m
    }

    /// A declared type with type variables resolved via `subst`.
    fn subst_cty(&self, t: &Type, subst: &HashMap<String, CTy>) -> CTy {
        match t {
            Type::Name(segs) if segs.len() == 1 => {
                subst.get(&segs[0]).cloned().unwrap_or_else(|| self.cty(t))
            }
            Type::Array(inner) => CTy::Arr(Box::new(self.subst_cty(inner, subst))),
            Type::Opt(inner) => self.subst_cty(inner, subst),
            Type::Paren(inner) => self.subst_cty(inner, subst),
            Type::Apply(base, args) if matches!(&**base, Type::Name(n) if n.last().is_some_and(|h| h == "Map")) => {
                CTy::Map(Box::new(
                    args.last()
                        .map_or(CTy::Unknown, |a| self.subst_cty(a, subst)),
                ))
            }
            Type::Fn(ps, r) => closure_ty(ps.len(), self.subst_cty(r, subst)),
            _ => self.cty(t),
        }
    }

    fn spec_ret(&self, genf: &FnDef, arg_ctys: &[CTy]) -> CTy {
        let subst = self.build_subst(genf, arg_ctys);
        genf.ret
            .as_ref()
            .map_or(CTy::Unit, |t| self.subst_cty(t, &subst))
    }

    /// The C type of a captured free variable (a function-local of the enclosing scope).
    fn cap_ty(&self, env: &Env, n: &str) -> CTy {
        lookup(env, n).unwrap_or(CTy::Unknown)
    }

    /// Higher-order list methods whose lambda argument must be lowered with the element type as its parameter type.
    fn list_hof(
        &mut self,
        env: &mut Env,
        rc: &str,
        elem: &CTy,
        method: &str,
        args: &[Arg],
    ) -> Option<(String, CTy)> {
        let src = arr_name(elem);
        let lambda = |a: Option<&Arg>| match a {
            Some(Arg::Pos(Expr::Lambda { params, body, .. })) => {
                Some((params.clone(), (**body).clone()))
            }
            _ => None,
        };
        let named_fn = |a: Option<&Arg>, cx: &Self| match a {
            Some(Arg::Pos(Expr::Ident(n))) if cx.fns.contains_key(n) => Some(n.clone()),
            _ => None,
        };
        match method {
            "map" => {
                let (clos, ret) = match named_fn(args.first(), self) {
                    Some(n) => {
                        let ret = self.fns[&n].1.clone();
                        (self.fn_value_closure(&n).0, ret)
                    }
                    None => {
                        let (params, body) = lambda(args.first())?;
                        self.emit_closure(env, &params, &body, std::slice::from_ref(elem))
                    }
                };
                self.note_arr(&CTy::Arr(Box::new(ret.clone())));
                let dst = arr_name(&ret);
                let boxed = box_i64("_s.data[_i]", elem);
                let unboxed = unbox_i64("_v", &ret);
                let code = format!(
                    "({{ {src} _s = {rc}; {dst} _r = {dst}_new(); maca_closure _f = {clos}; \
                     for (int64_t _i = 0; _i < _s.len; _i++) {{ int64_t _v = maca_call1(_f, {boxed}); {dst}_push(&_r, {unboxed}); }} _r; }})"
                );
                Some((code, CTy::Arr(Box::new(ret))))
            }
            "filter" => {
                let held = match args.first() {
                    Some(Arg::Pos(Expr::Ident(n)))
                        if matches!(lookup(env, n), Some(CTy::Closure(_) | CTy::Closure2(_))) =>
                    {
                        Some(cid(n))
                    }
                    _ => None,
                };
                let clos = match (held, named_fn(args.first(), self)) {
                    (Some(v), _) => v,
                    (None, Some(n)) => self.fn_value_closure(&n).0,
                    (None, None) => {
                        let (params, body) = lambda(args.first())?;
                        self.emit_closure(env, &params, &body, std::slice::from_ref(elem))
                            .0
                    }
                };
                let boxed = box_i64("_s.data[_i]", elem);
                let code = format!(
                    "({{ {src} _s = {rc}; {src} _r = {src}_new(); maca_closure _f = {clos}; \
                     for (int64_t _i = 0; _i < _s.len; _i++) if (maca_call1(_f, {boxed})) {src}_push(&_r, _s.data[_i]); _r; }})"
                );
                Some((code, CTy::Arr(Box::new(elem.clone()))))
            }
            "index_of_by" => {
                let clos = match named_fn(args.first(), self) {
                    Some(n) => self.fn_value_closure(&n).0,
                    None => {
                        let (params, body) = lambda(args.first())?;
                        self.emit_closure(env, &params, &body, std::slice::from_ref(elem))
                            .0
                    }
                };
                let boxed = box_i64("_s.data[_i]", elem);
                let code = format!(
                    "({{ {src} _s = {rc}; maca_closure _f = {clos}; int64_t _r = -1; \
                     for (int64_t _i = 0; _i < _s.len; _i++) if (maca_call1(_f, {boxed})) {{ _r = _i; break; }} _r; }})"
                );
                Some((code, CTy::Int))
            }
            "sort_by" => {
                let (clos, key) = match named_fn(args.first(), self) {
                    Some(n) => {
                        let ret = self.fns[&n].1.clone();
                        (self.fn_value_closure(&n).0, ret)
                    }
                    None => {
                        let (params, body) = lambda(args.first())?;
                        self.emit_closure(env, &params, &body, std::slice::from_ref(elem))
                    }
                };
                self.note_arr(&CTy::Arr(Box::new(key.clone())));
                let kn = arr_name(&key);
                let kt = c_type(&key);
                if !matches!(key, CTy::Int | CTy::Float | CTy::F32 | CTy::Str) {
                    self.problem(
                        "`sort_by` needs a key that orders: an int, a float or a str".to_string(),
                    );
                }
                let boxed = box_i64("_r.data[_i]", elem);
                let read = unbox_i64(&format!("maca_call1(_f, {boxed})"), &key);
                let ahead = if matches!(key, CTy::Str) {
                    "maca_str_cmp(_k.data[_j], _kv) > 0"
                } else {
                    "_k.data[_j] > _kv"
                };
                let code = format!(
                    "({{ {src} _r = {src}_concat({rc}, {src}_new()); maca_closure _f = {clos}; \
                     {kn} _k = {kn}_new(); \
                     for (int64_t _i = 0; _i < _r.len; _i++) {kn}_push(&_k, {read}); \
                     for (int64_t _i = 1; _i < _r.len; _i++) {{ \
                     {elem_c} _v = _r.data[_i]; {kt} _kv = _k.data[_i]; int64_t _j = _i - 1; \
                     while (_j >= 0 && {ahead}) {{ _r.data[_j + 1] = _r.data[_j]; \
                     _k.data[_j + 1] = _k.data[_j]; _j--; }} \
                     _r.data[_j + 1] = _v; _k.data[_j + 1] = _kv; }} _r; }})",
                    elem_c = c_type(elem)
                );
                Some((code, CTy::Arr(Box::new(elem.clone()))))
            }
            "reduce" | "fold" => {
                let (initc, acc_ty) = self.arg_typed(env, args.first()?);
                let clos = match named_fn(args.get(1), self) {
                    Some(n) => self.fn_value_closure(&n).0,
                    None => {
                        let (params, body) = lambda(args.get(1))?;
                        self.emit_closure(env, &params, &body, &[acc_ty.clone(), elem.clone()])
                            .0
                    }
                };
                let init_boxed = box_i64(&initc, &acc_ty);
                let elem_boxed = box_i64("_s.data[_i]", elem);
                let result = unbox_i64("_acc", &acc_ty);
                let code = format!(
                    "({{ {src} _s = {rc}; maca_closure2 _f = {clos}; int64_t _acc = {init_boxed}; \
                     for (int64_t _i = 0; _i < _s.len; _i++) _acc = maca_call2(_f, _acc, {elem_boxed}); {result}; }})"
                );
                Some((code, acc_ty))
            }
            _ => None,
        }
    }

    /// Names that resolve globally (so referencing them isn't a capture).
    fn is_known_global(&self, n: &str) -> bool {
        maca_parser::is_backend_intrinsic(n)
            || self.fns.contains_key(n)
            || self.let_names.contains(n)
            || self.variant_of.contains_key(n)
            || self.modules.contains(n)
            || console_fn(n).is_some()
            || matches!(
                n,
                "str"
                    | "int"
                    | "float"
                    | "print"
                    | "info"
                    | "len"
                    | "chr"
                    | "ord"
                    | "true"
                    | "false"
                    | "read_file"
                    | "write_file"
                    | "file_exists"
                    | "real_path"
                    | "is_tty"
                    | "make_dir"
                    | "list_dir"
                    | "is_dir"
                    | "file_size"
                    | "modified_ms"
                    | "remove_file"
                    | "remove_dir"
                    | "copy_bytes"
                    | "exec"
                    | "capture"
                    | "env"
                    | "cwd"
                    | "chdir"
                    | "read_line"
                    | "at_eof"
                    | "read_stdin"
                    | "now_ms"
                    | "now_iso"
                    | "format_time"
                    | "assert"
                    | "assert_eq"
                    | "failures"
                    | "alloc_count"
                    | "reuse_count"
                    | "map"
            )
    }

    fn ctor(&mut self, env: &mut Env, name: &str, fields: &[Field]) -> (String, CTy) {
        let decl = self.records.get(name).cloned();
        let Some(decl) = decl else {
            self.problem(format!("construction of unknown record type `{name}`"));
            return ("0 /* unknown ctor */".into(), CTy::Unknown);
        };
        let mut parts = Vec::new();
        for (fname, fty) in &decl {
            let val = fields.iter().find_map(|f| match f {
                Field::Value { name: n, value } if n == fname => Some((value.clone(), false)),
                Field::Shorthand(n) if n == fname => Some((Expr::Ident(n.clone()), true)),
                _ => None,
            });
            let code = match val {
                Some((v, _)) => {
                    self.lambda_hint = self
                        .record_fn_params
                        .get(&format!("{name}.{fname}"))
                        .cloned();
                    let c = self.expr(env, &v, Some(fty)).0;
                    self.lambda_hint = None;
                    c
                }
                None => zero_value(fty),
            };
            parts.push(format!(".{} = {code}", cid(fname)));
        }
        (
            format!("(({name}){{ {} }})", parts.join(", ")),
            CTy::Rec(name.into()),
        )
    }

    /// `base with { f = v }` → `({ T _t = base; _t.f = v; _t; })`.
    fn with_update(&mut self, env: &mut Env, base: &Expr, fields: &[Field]) -> (String, CTy) {
        let (bc, bt) = self.expr(env, base, None);
        let CTy::Rec(rname) = bt else {
            self.problem("`with` update requires a record on the left");
            return (
                "0 /* unsupported: `with` on non-record */".into(),
                CTy::Unknown,
            );
        };
        let decl = self.records.get(&rname).cloned().unwrap_or_default();
        let t = self.temp();
        let mut assigns = Vec::new();
        for f in fields {
            let (fname, val) = match f {
                Field::Value { name, value } => (name.clone(), value.clone()),
                Field::Shorthand(name) => (name.clone(), Expr::Ident(name.clone())),
                _ => continue,
            };
            let fty = decl
                .iter()
                .find(|(n, _)| *n == fname)
                .map(|(_, t)| t.clone());
            self.lambda_hint = self
                .record_fn_params
                .get(&format!("{rname}.{fname}"))
                .cloned();
            let code = self.expr(env, &val, fty.as_ref()).0;
            self.lambda_hint = None;
            assigns.push(format!("{t}.{} = {code};", cid(&fname)));
        }
        (
            format!("({{ {rname} {t} = {bc}; {} {t}; }})", assigns.join(" ")),
            CTy::Rec(rname),
        )
    }

    fn list(&mut self, env: &mut Env, es: &[Expr], expected: Option<&CTy>) -> (String, CTy) {
        let elem = match expected {
            Some(CTy::Arr(e)) => (**e).clone(),
            _ => es
                .first()
                .map(|e0| self.expr(env, e0, None).1)
                .unwrap_or(CTy::Unknown),
        };
        let an = arr_name(&elem);
        if es.is_empty() {
            return (format!("{an}_new()"), CTy::Arr(Box::new(elem)));
        }
        let mut body = format!("{an} _a = {an}_new();");
        for e in es {
            let (c, _) = self.expr(env, e, Some(&elem));
            body.push_str(&format!(" {an}_push(&_a, {c});"));
        }
        (format!("({{ {body} _a; }})"), CTy::Arr(Box::new(elem)))
    }

    /// The trailing arguments of a variadic call, collected into one list.
    fn collect_rest(&mut self, env: &mut Env, tail: &[Arg], elem: &CTy) -> String {
        let pieces: Vec<String> = tail
            .iter()
            .map(|x| self.arg_expected(env, x, Some(elem)).0)
            .collect();
        self.list_of(&pieces, elem)
    }

    /// `receiver.f(…)` where `f` is variadic.
    fn ufcs_variadic(
        &mut self,
        env: &mut Env,
        rc: &str,
        name: &str,
        args: &[Arg],
        at: usize,
    ) -> (String, CTy) {
        let Some((params, ret)) = self.fns.get(name).cloned() else {
            self.problem(format!(
                "`{name}` is generic and variadic, so it cannot be called \
                 UFCS-style. Write `{name}(…)`"
            ));
            return (format!("{}({rc})", cid(name)), CTy::Unknown);
        };
        let elem = match params.get(at) {
            Some(CTy::Arr(e)) => (**e).clone(),
            _ => CTy::Unknown,
        };
        let mut fixed = vec![rc.to_string()];
        let mut rest = Vec::new();
        for (i, x) in args.iter().enumerate() {
            let slot = i + 1;
            if slot < at {
                fixed.push(self.arg_expected(env, x, params.get(slot)).0);
            } else {
                rest.push(self.arg_expected(env, x, Some(&elem)).0);
            }
        }
        if at == 0 {
            rest.insert(0, fixed.remove(0));
        }
        fixed.push(self.list_of(&rest, &elem));
        (format!("{}({})", cid(name), fixed.join(", ")), ret)
    }

    /// Already-lowered elements, gathered into one fresh list.
    fn list_of(&mut self, pieces: &[String], elem: &CTy) -> String {
        let an = arr_name(elem);
        if pieces.is_empty() {
            return format!("{an}_new()");
        }
        let v = self.temp();
        let mut body = format!("{an} {v} = {an}_new();");
        for c in pieces {
            body.push_str(&format!(" {an}_push(&{v}, {c});"));
        }
        format!("({{ {body} {v}; }})")
    }

    /// `tag(attr=value, …, child, …)` → the HTML text for that element.
    fn html_element(&mut self, env: &mut Env, tag: &str, args: &[Arg]) -> (String, CTy) {
        let (attrs, kids) = self.html_args(env, args);
        let mut pieces = vec![Piece::literal(format!("\"<{tag}\""))];
        pieces.extend(attrs);
        pieces.push(Piece::literal("\">\"".into()));
        if is_void_html(tag) {
            if !kids.is_empty() {
                self.problem(format!("`{tag}` is a void element and takes no children"));
            }
            return (self.concat(&pieces), CTy::Str);
        }
        pieces.extend(kids);
        pieces.push(Piece::literal(format!("\"</{tag}>\"")));
        (self.concat(&pieces), CTy::Str)
    }

    /// `element(tag, attr=value, …, child, …)` is the same element, with the tag itself an expression.
    fn dynamic_element(&mut self, env: &mut Env, args: &[Arg]) -> (String, CTy) {
        let Some((Arg::Pos(tag_expr), rest)) = args.split_first() else {
            self.problem("`element` needs a tag name as its first argument".to_string());
            return ("\"\"".to_string(), CTy::Str);
        };
        let (tag, tt) = self.expr(env, tag_expr, Some(&CTy::Str));
        let tag = to_str(&tag, &tt);
        let (attrs, kids) = self.html_args(env, rest);
        let attrs = self.concat(&attrs);
        let kids = self.concat(&kids);
        (format!("maca_element({tag}, {attrs}, {kids})"), CTy::Str)
    }

    /// Is this attribute value a function rather than a value, which only a live DOM has anything to do with?
    fn is_handler(&self, env: &Env, value: &Expr) -> bool {
        match value {
            Expr::Lambda { .. } => true,
            Expr::Ident(n) => {
                self.fns.contains_key(n) && lookup(env, n).is_none() && !self.let_names.contains(n)
            }
            _ => false,
        }
    }

    /// Lower an element call's arguments into (attribute pieces, child pieces).
    fn html_args(&mut self, env: &mut Env, args: &[Arg]) -> (Vec<Piece>, Vec<Piece>) {
        let mut attrs: Vec<Piece> = Vec::new();
        let mut kids: Vec<Piece> = Vec::new();
        for a in args {
            match a {
                Arg::Named { name, value } => {
                    if self.is_handler(env, value) {
                        self.problem(format!(
                            "`{name}=` is given a function, and there is nothing here to \
                             call it: markup is text. Build this with `--target js`"
                        ));
                        continue;
                    }
                    let (v, t) = self.expr(env, value, Some(&CTy::Str));
                    if name == "class" {
                        self.note_classes(value);
                    }
                    let key = name.replace('"', "");
                    let code = if t == CTy::Bool {
                        format!("maca_flag(\"{key}\", {v})")
                    } else {
                        format!("maca_attr(\"{key}\", {})", to_str(&v, &t))
                    };
                    attrs.push(Piece { code, owned: true });
                }
                Arg::Directive { prop, .. } => {
                    self.problem(format!(
                        "`on:{prop}` needs a live DOM; build this with `--target js`"
                    ));
                }
                Arg::Pos(e) => {
                    let (c, t) = self.expr(env, e, Some(&CTy::Str));
                    if let CTy::Arr(inner) = &t
                        && matches!(**inner, CTy::Str)
                    {
                        let v = self.temp();
                        kids.push(Piece {
                            code: format!(
                                "({{ StrArr {v} = {c}; maca_join({v}.data, {v}.len, \"\"); }})"
                            ),
                            owned: true,
                        });
                        continue;
                    }
                    kids.push(Piece::rendered(&c, &t, self.fresh.allocates(e)));
                }
            }
        }
        (attrs, kids)
    }

    fn call(
        &mut self,
        env: &mut Env,
        callee: &Expr,
        args: &[Arg],
        expected: Option<&CTy>,
    ) -> (String, CTy) {
        if let Expr::Ident(tag) = callee
            && (maca_parser::is_ui_element_tag(tag) || tag == "element")
            && !self.fns.contains_key(tag)
            && !self.generics.contains_key(tag)
            && lookup(env, tag).is_none()
        {
            return if tag == "element" {
                self.dynamic_element(env, args)
            } else {
                self.html_element(env, tag, args)
            };
        }
        if let Expr::Ident(f) = callee
            && matches!(f.as_str(), "encode" | "decode")
            && self.generics.contains_key(f)
            && args.len() == 1
            && lookup(env, f).is_none()
        {
            return self.module_call(env, "json", f, args, expected);
        }
        if let Expr::Field { base, name } = callee {
            if let Expr::Ident(m) = &**base {
                if name == "splat"
                    && let Some((_, lanes)) = parse_vec_c(m)
                {
                    let k = self.arg(env, &args[0]);
                    let elems = vec![k.as_str(); lanes].join(", ");
                    let (sc, ln) = parse_vec_c(m).unwrap();
                    return (
                        format!("(({m}){{ {elems} }})"),
                        CTy::Vec {
                            name: m.clone(),
                            scalar_c: sc,
                            lanes: ln,
                        },
                    );
                }
                if self.modules.contains(m) && lookup(env, m).is_none() {
                    return self.module_call(env, m, name, args, expected);
                }
            }
            let (rc, rty) = self.expr(env, base, None);
            if let CTy::Arr(elem) = &rty
                && let Some(res) = self.list_hof(env, &rc, elem, name, args)
            {
                return res;
            }
            if let CTy::Rec(r) = &rty
                && let Some(f) = self.field_ty(r, name)
                && matches!(f, CTy::Closure(_) | CTy::Closure2(_))
            {
                return self.call_closure(
                    env,
                    &format!("({rc}).{}", cid(name)),
                    &f,
                    args,
                    expected,
                    1,
                );
            }
            if let Some(&at) = self.variadics.get(name) {
                return self.ufcs_variadic(env, &rc, name, args, at);
            }
            let a: Vec<String> = args.iter().map(|x| self.arg(env, x)).collect();
            return self.ufcs(&rc, &rty, name, &a);
        }
        if let Expr::Ident(name) = callee {
            if let Some(t @ (CTy::Closure(_) | CTy::Closure2(_))) = lookup(env, name) {
                let target = self.local_ref(name);
                let want = usize::from(!self.nullary.contains(name));
                return self.call_closure(env, &target, &t, args, expected, want);
            }
            if name == "str" {
                let (c, t) = self.arg_typed(env, &args[0]);
                return match t {
                    CTy::Str => (format!("maca_str_copy({c})"), CTy::Str),
                    _ => (to_str(&c, &t), CTy::Str),
                };
            }
            if name == "int" {
                let (c, t) = self.arg_typed(env, &args[0]);
                return match t {
                    CTy::Str => (format!("atoll({c})"), CTy::Int),
                    _ => (format!("((int64_t)({c}))"), CTy::Int),
                };
            }
            if name == "float" {
                let (c, t) = self.arg_typed(env, &args[0]);
                return match t {
                    CTy::Str => (format!("atof({c})"), CTy::Float),
                    _ => (format!("((double)({c}))"), CTy::Float),
                };
            }
            if name == "sleep_ms" && args.len() == 1 {
                let a = self.arg(env, &args[0]);
                return (format!("(maca_sleep_ms({a}), 0)"), CTy::Unit);
            }
            if args.len() == 1
                && matches!(
                    name.as_str(),
                    "sqrt" | "floor" | "ceil" | "round" | "sin" | "cos" | "tan" | "log" | "exp"
                )
            {
                let (c, _) = self.arg_typed(env, &args[0]);
                return (format!("__builtin_{name}((double)({c}))"), CTy::Float);
            }
            if name == "pow" && args.len() == 2 {
                let a = self.arg(env, &args[0]);
                let b = self.arg(env, &args[1]);
                return (
                    format!("__builtin_pow((double)({a}), (double)({b}))"),
                    CTy::Float,
                );
            }
            if name == "abs" && args.len() == 1 {
                let (c, t) = self.arg_typed(env, &args[0]);
                return match t {
                    CTy::Float | CTy::F32 => (format!("__builtin_fabs({c})"), CTy::Float),
                    _ => (format!("__builtin_llabs({c})"), CTy::Int),
                };
            }
            if (name == "min" || name == "max") && args.len() == 2 {
                let (a, ta) = self.arg_typed(env, &args[0]);
                let (b, _) = self.arg_typed(env, &args[1]);
                let op = if name == "min" { "<" } else { ">" };
                let ct = c_type(&ta);
                return (
                    format!("({{ {ct} _a = {a}; {ct} _b = {b}; _a {op} _b ? _a : _b; }})"),
                    ta,
                );
            }
            if name == "clamp" && args.len() == 3 {
                let (x, tx) = self.arg_typed(env, &args[0]);
                let lo = self.arg(env, &args[1]);
                let hi = self.arg(env, &args[2]);
                let ct = c_type(&tx);
                return (
                    format!(
                        "({{ {ct} _x = {x}, _lo = {lo}, _hi = {hi}; _x < _lo ? _lo : (_x > _hi ? _hi : _x); }})"
                    ),
                    tx,
                );
            }
            if name == "sign" && args.len() == 1 {
                let (c, _) = self.arg_typed(env, &args[0]);
                return (
                    format!("({{ __typeof__({c}) _v = {c}; (int64_t)((_v > 0) - (_v < 0)); }})"),
                    CTy::Int,
                );
            }
            if name == "gcd" && args.len() == 2 {
                let a = self.arg(env, &args[0]);
                let b = self.arg(env, &args[1]);
                return (
                    format!(
                        "({{ int64_t _a = __builtin_llabs({a}), _b = __builtin_llabs({b}); while (_b) {{ int64_t _t = _a % _b; _a = _b; _b = _t; }} _a; }})"
                    ),
                    CTy::Int,
                );
            }
            if name == "chr" && args.len() == 1 {
                let a = self.arg(env, &args[0]);
                return (format!("maca_chr({a})"), CTy::Str);
            }
            if name == "ord" && args.len() == 1 {
                let a = self.arg(env, &args[0]);
                return (format!("maca_ord({a})"), CTy::Int);
            }
            if name == "len" && args.len() == 1 {
                let (c, t) = self.arg_typed(env, &args[0]);
                return match t {
                    CTy::Str => (format!("((int64_t)strlen({c}))"), CTy::Int),
                    _ => (format!("({c}).len"), CTy::Int),
                };
            }
            if let Some(genf) = self.generics.get(name).cloned() {
                let declared = self.declared_lambda_params(name, args, env);
                let fixed = self.variadics.get(name).copied();
                let spelled = fixed.unwrap_or(args.len()).min(args.len());
                let want: Vec<Option<CTy>> = genf
                    .params
                    .iter()
                    .map(|p| p.ty.as_ref().map(|t| self.cty(t)).filter(is_settled))
                    .collect();
                let mut lowered: Vec<(String, CTy)> = args[..spelled]
                    .iter()
                    .enumerate()
                    .map(|(i, x)| {
                        self.lambda_hint = declared.get(&i).cloned();
                        let got = self.arg_expected(env, x, want.get(i).and_then(|t| t.as_ref()));
                        self.lambda_hint = None;
                        got
                    })
                    .collect();
                if fixed.is_some() {
                    let tail: Vec<(String, CTy)> = args[spelled..]
                        .iter()
                        .map(|x| self.arg_typed(env, x))
                        .collect();
                    let elem = tail.first().map_or(CTy::Unknown, |(_, t)| t.clone());
                    let pieces: Vec<String> = tail.into_iter().map(|(c, _)| c).collect();
                    let code = self.list_of(&pieces, &elem);
                    lowered.push((code, CTy::Arr(Box::new(elem))));
                }
                let arg_ctys: Vec<CTy> = lowered.iter().map(|(_, t)| t.clone()).collect();
                let key = (name.to_string(), arg_ctys.clone());
                if !self.spec_done.contains(&key) && !self.spec_pending.contains(&key) {
                    self.spec_pending.push(key);
                }
                let ret = self.spec_ret(&genf, &arg_ctys);
                let code = format!(
                    "{}({})",
                    mangle_name(name, &arg_ctys),
                    lowered
                        .iter()
                        .map(|(c, _)| c.clone())
                        .collect::<Vec<_>>()
                        .join(", ")
                );
                return (code, ret);
            }
            if let Some(sum) = self.variant_of.get(name).cloned()
                && self.is_tagged(&sum)
            {
                let a: Vec<String> = args.iter().map(|x| self.arg(env, x)).collect();
                return (format!("{sum}_{name}({})", a.join(", ")), CTy::Sum(sum));
            }
            if let Some((params, ret)) = self.fns.get(name).cloned() {
                let declared = self.declared_lambda_params(name, args, env);
                let fixed = self.variadics.get(name).copied();
                let spelled = fixed.unwrap_or(args.len()).min(args.len());
                let mut a: Vec<String> = args[..spelled]
                    .iter()
                    .enumerate()
                    .map(|(i, x)| {
                        if matches!(x, Arg::Pos(Expr::Lambda { .. })) {
                            self.lambda_hint = declared
                                .get(&i)
                                .cloned()
                                .or_else(|| self.callee_param_ty(name, i, 8).map(|t| vec![t]));
                        }
                        let c = self.arg_expected(env, x, params.get(i)).0;
                        self.lambda_hint = None;
                        c
                    })
                    .collect();
                if let Some(at) = fixed {
                    let elem = match params.get(at) {
                        Some(CTy::Arr(e)) => (**e).clone(),
                        _ => CTy::Unknown,
                    };
                    a.push(self.collect_rest(env, &args[spelled..], &elem));
                }
                return (format!("{}({})", cid(name), a.join(", ")), ret);
            }
            let a: Vec<String> = args.iter().map(|x| self.arg(env, x)).collect();
            if let Some(cfn) = console_fn(name) {
                return (format!("{cfn}({})", a.join(", ")), CTy::Unit);
            }
            if name == "styles" && a.is_empty() {
                return ("MACA_STYLES".into(), CTy::Str);
            }
            if name == "map" && a.is_empty() {
                let v = match expected {
                    Some(CTy::Map(v)) => (**v).clone(),
                    _ => CTy::Int,
                };
                self.note_arr(&CTy::Map(Box::new(v.clone())));
                return (format!("{}_new()", map_name(&v)), CTy::Map(Box::new(v)));
            }
            match name.as_str() {
                "read_file" => return (format!("maca_read_file({})", a.join(", ")), CTy::Str),
                "write_file" => {
                    return (format!("maca_write_file({})", a.join(", ")), CTy::Bool);
                }
                "file_exists" => {
                    return (format!("maca_file_exists({})", a.join(", ")), CTy::Bool);
                }
                "real_path" => {
                    return (format!("maca_real_path({})", a.join(", ")), CTy::Str);
                }
                "is_tty" => return ("maca_is_tty()".into(), CTy::Bool),
                "make_dir" => return (format!("maca_make_dir({})", a.join(", ")), CTy::Bool),
                "is_dir" => return (format!("maca_is_dir({})", a.join(", ")), CTy::Bool),
                "file_size" => return (format!("maca_file_size({})", a.join(", ")), CTy::Int),
                "modified_ms" => return (format!("maca_modified_ms({})", a.join(", ")), CTy::Int),
                "remove_file" => return (format!("maca_remove_file({})", a.join(", ")), CTy::Bool),
                "remove_dir" => return (format!("maca_remove_dir({})", a.join(", ")), CTy::Bool),
                "copy_bytes" => {
                    return (format!("maca_copy_bytes({})", a.join(", ")), CTy::Bool);
                }
                "exec" | "capture" => {
                    let (cmd, args) = (&a[0], &a[1]);
                    let fn_name = format!("maca_{name}");
                    let ret = if name == "exec" { CTy::Int } else { CTy::Str };
                    return (format!("{fn_name}({cmd}, {args}.data, {args}.len)"), ret);
                }
                "env" => return (format!("maca_env({})", a.join(", ")), CTy::Str),
                "cwd" => return ("maca_cwd()".into(), CTy::Str),
                "chdir" => return (format!("maca_chdir({})", a.join(", ")), CTy::Bool),
                "read_line" => return ("maca_read_line()".into(), CTy::Str),
                "at_eof" => return ("maca_at_eof()".into(), CTy::Bool),
                "read_stdin" => return ("maca_read_stdin()".into(), CTy::Str),
                "now_ms" => return ("maca_now_ms()".into(), CTy::Int),
                "now_iso" => return ("maca_now_iso()".into(), CTy::Str),
                "format_time" => {
                    return (format!("maca_format_time({})", a.join(", ")), CTy::Str);
                }
                "assert" => return (format!("maca_assert({})", a.join(", ")), CTy::Bool),
                "assert_eq" => {
                    let shown: Vec<String> = args
                        .iter()
                        .enumerate()
                        .map(|(i, x)| {
                            let (c, t) = self.expr(env, arg_expr(x), None);
                            if i < 2 { to_str(&c, &t) } else { c }
                        })
                        .collect();
                    return (format!("maca_assert_eq({})", shown.join(", ")), CTy::Bool);
                }
                "failures" => return ("maca_failures()".into(), CTy::Int),
                "alloc_count" => return ("(int64_t)maca_alloc_count()".into(), CTy::Int),
                "reuse_count" => return ("(int64_t)maca_reuse_count()".into(), CTy::Int),
                "list_dir" => {
                    self.note_arr(&CTy::Arr(Box::new(CTy::Str)));
                    return (
                        format!(
                            "({{ int64_t _dn; maca_str* _dd = maca_list_dir({}, &_dn); \
                             StrArr _dr = StrArr_new(); for (int64_t _di = 0; _di < _dn; _di++) StrArr_push(&_dr, _dd[_di]); _dr; }})",
                            a.join(", ")
                        ),
                        CTy::Arr(Box::new(CTy::Str)),
                    );
                }
                _ => {}
            }
            let _ = expected;
            return (format!("{}({})", cid(name), a.join(", ")), CTy::Unknown);
        }
        self.problem("call target is not a function name (higher-order call value unsupported)");
        ("0 /* unsupported call */".into(), CTy::Unknown)
    }

    /// The JSON text for a value of this type, as one expression, or `None` where the type has no JSON form.
    fn json_text_of(&mut self, code: &str, t: &CTy) -> Option<String> {
        Some(match t {
            CTy::Int => format!("maca_from_int({code})"),
            CTy::Float | CTy::F32 => format!("maca_from_float({code})"),
            CTy::Bool => format!("maca_from_bool({code})"),
            CTy::Str => format!("maca_json_quote({code})"),
            CTy::Sum(s) => format!("maca_json_quote({s}_to_json_name({code}))"),
            CTy::Rec(r) => format!("{r}_to_json({code})"),
            CTy::Arr(e) => {
                let an = arr_name(e);
                let v = self.temp();
                let piece = self.json_text_of(&format!("{v}.data[_i]"), e)?;
                format!(
                    "({{ {an} {v} = {code}; maca_sb _sb; maca_sb_init(&_sb); \
                     maca_sb_putc(&_sb, '['); \
                     for (int64_t _i = 0; _i < {v}.len; _i++) {{ \
                     if (_i) maca_sb_putc(&_sb, ','); \
                     maca_str _p = {piece}; maca_sb_puts(&_sb, _p); maca_drop_str(_p); }} \
                     maca_sb_putc(&_sb, ']'); maca_sb_finish(&_sb); }})"
                )
            }
            _ => return None,
        })
    }

    fn module_call(
        &mut self,
        env: &mut Env,
        module: &str,
        member: &str,
        args: &[Arg],
        expected: Option<&CTy>,
    ) -> (String, CTy) {
        match (module, member) {
            ("json", "encode") => {
                let (c, t) = self.arg_typed(env, &args[0]);
                match self.json_text_of(&c, &t) {
                    Some(code) => (code, CTy::Str),
                    None => {
                        self.problem(format!(
                            "`encode`: `{}` has no JSON form; a record, a sum, a \
                             primitive or a list of those does",
                            c_type(&t)
                        ));
                        ("\"null\"".into(), CTy::Str)
                    }
                }
            }
            ("json", "decode") => {
                let (c, _) = self.arg_typed(env, &args[0]);
                match expected {
                    Some(CTy::Rec(r)) => (
                        format!("{r}_from_json(maca_json_parse({c}))"),
                        CTy::Rec(r.clone()),
                    ),
                    _ => {
                        self.problem(
                            "`decode`: say what it reads into, as in \
                             `c: Config = decode(text)`"
                                .to_string(),
                        );
                        ("\"\"".into(), CTy::Str)
                    }
                }
            }
            _ => {
                let a: Vec<String> = args.iter().map(|x| self.arg(env, x)).collect();
                (
                    format!("/* {module}.{member} */ ({})", a.join(", ")),
                    CTy::Unknown,
                )
            }
        }
    }

    /// For each argument position holding a lambda, the parameter types the callee declared for it (`(T, U) -> R`), with any type variables settled from the arguments the call site gives concretely.
    fn declared_lambda_params(
        &mut self,
        callee: &str,
        args: &[Arg],
        env: &mut Env,
    ) -> HashMap<usize, Vec<CTy>> {
        let mut out = HashMap::new();
        let Some(def) = self.fn_def(callee) else {
            return out;
        };
        let mut subst: HashMap<String, CTy> = HashMap::new();
        for (i, p) in def.params.iter().enumerate() {
            let Some(t) = &p.ty else { continue };
            let Some(a) = args.get(i) else { continue };
            if matches!(a, Arg::Pos(Expr::Lambda { .. })) {
                continue;
            }
            let (_, cty) = self.expr(env, arg_expr(a), None);
            bind_vars(t, &cty, &mut subst);
        }
        for (i, p) in def.params.iter().enumerate() {
            if let Some(Type::Fn(ps, _)) = &p.ty
                && matches!(args.get(i), Some(Arg::Pos(Expr::Lambda { .. })))
            {
                let ctys: Vec<CTy> = ps.iter().map(|t| self.subst_cty(t, &subst)).collect();
                out.insert(i, ctys);
            }
        }
        out
    }

    /// A function the module defines, generic or not.
    fn fn_def(&self, name: &str) -> Option<FnDef> {
        if let Some(g) = self.generics.get(name) {
            return Some(g.clone());
        }
        self.m.items.iter().find_map(|it| match it {
            Stmt::Fn(f) if f.name == name => Some(f.clone()),
            _ => None,
        })
    }

    /// The declared C type of `field` on record `rec`, if it has one.
    fn field_ty(&self, rec: &str, field: &str) -> Option<CTy> {
        self.records
            .get(rec)?
            .iter()
            .find(|(n, _)| n == field)
            .map(|(_, t)| t.clone())
    }

    /// Call through a closure value, however it was reached: a local, a parameter, or a record field.
    fn call_closure(
        &mut self,
        env: &mut Env,
        target: &str,
        ty: &CTy,
        args: &[Arg],
        expected: Option<&CTy>,
        one: usize,
    ) -> (String, CTy) {
        let (CTy::Closure(r) | CTy::Closure2(r)) = ty else {
            return ("0 /* not a function */".into(), CTy::Unknown);
        };
        let want = if matches!(ty, CTy::Closure2(_)) {
            2
        } else {
            one
        };
        if args.len() != want {
            self.problem(format!(
                "this function value takes {want} argument(s), not {}",
                args.len()
            ));
        }
        let boxed: Vec<(String, CTy)> = args.iter().map(|x| self.arg_typed(env, x)).collect();
        let bx: Vec<String> = boxed.iter().map(|(c, t)| box_i64(c, t)).collect();
        let ret = match expected {
            Some(t) if !matches!(t, CTy::Unknown | CTy::Unit) => t.clone(),
            _ => (**r).clone(),
        };
        let call = if bx.len() >= 2 {
            format!("maca_call2({target}, {}, {})", bx[0], bx[1])
        } else {
            let a0 = bx.first().cloned().unwrap_or_else(|| "0".into());
            format!("maca_call1({target}, {a0})")
        };
        (unbox_i64(&call, &ret), ret)
    }

    /// The record `enumerate` pairs an element with, named exactly as the literal `{ index = i, value = v }` would be.
    fn entry_record(&mut self, elem: &CTy) -> String {
        let shape = vec![
            ("index".to_string(), CTy::Int),
            ("value".to_string(), elem.clone()),
        ];
        let name = anon_record_name(&shape);
        if !self.records.contains_key(&name) {
            self.anon_records.insert(name.clone());
            self.records.insert(name.clone(), shape);
        }
        self.note_arr(&CTy::Arr(Box::new(CTy::Rec(name.clone()))));
        name
    }

    fn ufcs(&mut self, rc: &str, rty: &CTy, method: &str, a: &[String]) -> (String, CTy) {
        let arg0 = || a.first().cloned().unwrap_or_default();
        let arg1 = || a.get(1).cloned().unwrap_or_default();
        match (rty, method) {
            (_, "read") => (format!("maca_read({rc})"), CTy::Str),
            (_, "exists") => (format!("maca_path_exists({rc})"), CTy::Bool),
            (_, "write") => (format!("(maca_write({rc}, {}), 0)", arg0()), CTy::Unit),
            (CTy::Arr(e), "join") if matches!(**e, CTy::Str) => (
                format!("maca_join({rc}.data, {rc}.len, {})", arg0()),
                CTy::Str,
            ),
            (CTy::Map(v), "set") => {
                let mn = map_name(v);
                (
                    format!(
                        "({{ {mn} _m = {rc}; {mn}_set(&_m, {}, {}); _m; }})",
                        arg0(),
                        arg1()
                    ),
                    CTy::Map(v.clone()),
                )
            }
            (CTy::Map(v), "remove") => {
                let mn = map_name(v);
                (
                    format!("({{ {mn} _m = {rc}; {mn}_remove(&_m, {}); _m; }})", arg0()),
                    CTy::Map(v.clone()),
                )
            }
            (CTy::Map(v), "get") => {
                let mn = map_name(v);
                let dflt = a.get(1).cloned().unwrap_or_else(|| zero_value(v));
                (format!("{mn}_get({rc}, {}, {dflt})", arg0()), (**v).clone())
            }
            (CTy::Map(v), "has") => (format!("{}_has({rc}, {})", map_name(v), arg0()), CTy::Bool),
            (CTy::Map(v), "length") => (format!("{}_len({rc})", map_name(v)), CTy::Int),
            (CTy::Map(v), "keys") => {
                let mn = map_name(v);
                (
                    format!(
                        "({{ {mn} _m = {rc}; maca_str* _kb = (maca_str*)maca_alloc((size_t)(_m.len > 0 ? _m.len : 1) * sizeof(maca_str)); \
                         int64_t _kn = {mn}_keys(_m, _kb); StrArr _kr = StrArr_new(); \
                         for (int64_t _ki = 0; _ki < _kn; _ki++) StrArr_push(&_kr, _kb[_ki]); _kr; }})"
                    ),
                    CTy::Arr(Box::new(CTy::Str)),
                )
            }
            (CTy::Arr(e), "sort") if matches!(**e, CTy::Int | CTy::Float | CTy::Str) => {
                let an = arr_name(e);
                let sorter = match **e {
                    CTy::Float => "maca_sort_f64",
                    CTy::Str => "maca_sort_str",
                    _ => "maca_sort_i64",
                };
                (
                    format!(
                        "({{ {an} _s = {an}_concat({rc}, {an}_new()); {sorter}(_s.data, _s.len); _s; }})"
                    ),
                    CTy::Arr(e.clone()),
                )
            }
            (CTy::Arr(e), "reverse") => {
                let an = arr_name(e);
                (
                    format!(
                        "({{ {an} _s = {rc}; {an} _r = {an}_new(); for (int64_t _i = _s.len - 1; _i >= 0; _i--) {an}_push(&_r, _s.data[_i]); _r; }})"
                    ),
                    CTy::Arr(e.clone()),
                )
            }
            (CTy::Arr(e), "push") => {
                let an = arr_name(e);
                (
                    format!(
                        "({{ {an} _s = {an}_concat({rc}, {an}_new()); {an}_push(&_s, {}); _s; }})",
                        arg0()
                    ),
                    CTy::Arr(e.clone()),
                )
            }
            (CTy::Arr(e), "pop") => {
                let an = arr_name(e);
                (
                    format!(
                        "({{ {an} _s = {an}_concat({rc}, {an}_new()); if (_s.len > 0) _s.len--; _s; }})"
                    ),
                    CTy::Arr(e.clone()),
                )
            }
            (CTy::Arr(e), "set") => {
                let an = arr_name(e);
                (
                    format!(
                        "({{ {an} _s = {an}_concat({rc}, {an}_new()); {an}_put(&_s, {}, {}); _s; }})",
                        arg0(),
                        arg1()
                    ),
                    CTy::Arr(e.clone()),
                )
            }
            (CTy::Arr(e), "insert") => {
                let an = arr_name(e);
                (
                    format!(
                        "({{ {an} _s = {an}_concat({rc}, {an}_new()); {an}_insert(&_s, {}, {}); _s; }})",
                        arg0(),
                        arg1()
                    ),
                    CTy::Arr(e.clone()),
                )
            }
            (CTy::Arr(e), "remove") => {
                let an = arr_name(e);
                (
                    format!(
                        "({{ {an} _s = {an}_concat({rc}, {an}_new()); {an}_erase(&_s, {}); _s; }})",
                        arg0()
                    ),
                    CTy::Arr(e.clone()),
                )
            }
            (CTy::Arr(e), "enumerate") => {
                let src = arr_name(e);
                let entry = self.entry_record(e);
                let dst = arr_name(&CTy::Rec(entry.clone()));
                (
                    format!(
                        "({{ {src} _s = {rc}; {dst} _r = {dst}_new(); \
                         for (int64_t _i = 0; _i < _s.len; _i++) \
                         {dst}_push(&_r, ({entry}){{ .index = _i, .value = _s.data[_i] }}); _r; }})"
                    ),
                    CTy::Arr(Box::new(CTy::Rec(entry))),
                )
            }
            (CTy::Arr(e), "contains") => {
                let an = arr_name(e);
                let eq = if matches!(**e, CTy::Str) {
                    format!("maca_str_eq(_s.data[_i], {})", arg0())
                } else {
                    format!("_s.data[_i] == ({})", arg0())
                };
                (
                    format!(
                        "({{ {an} _s = {rc}; bool _f = false; for (int64_t _i = 0; _i < _s.len; _i++) if ({eq}) {{ _f = true; break; }} _f; }})"
                    ),
                    CTy::Bool,
                )
            }
            (CTy::Arr(e), "index_of") => {
                let an = arr_name(e);
                let eq = if matches!(**e, CTy::Str) {
                    format!("maca_str_eq(_s.data[_i], {})", arg0())
                } else {
                    format!("_s.data[_i] == ({})", arg0())
                };
                (
                    format!(
                        "({{ {an} _s = {rc}; int64_t _r = -1; for (int64_t _i = 0; _i < _s.len; _i++) if ({eq}) {{ _r = _i; break; }} _r; }})"
                    ),
                    CTy::Int,
                )
            }
            (CTy::Arr(e), "sum") if matches!(**e, CTy::Int | CTy::Float | CTy::F32) => {
                let an = arr_name(e);
                let z = if matches!(**e, CTy::Float) {
                    "0.0"
                } else {
                    "0"
                };
                (
                    format!(
                        "({{ {an} _s = {rc}; {} _acc = {z}; for (int64_t _i = 0; _i < _s.len; _i++) _acc += _s.data[_i]; _acc; }})",
                        c_type(e)
                    ),
                    (**e).clone(),
                )
            }
            (CTy::Arr(e), "min") if matches!(**e, CTy::Int | CTy::Float | CTy::F32) => {
                let an = arr_name(e);
                (
                    format!(
                        "({{ {an} _s = {rc}; {ct} _acc = _s.len > 0 ? _s.data[0] : 0; for (int64_t _i = 1; _i < _s.len; _i++) if (_s.data[_i] < _acc) _acc = _s.data[_i]; _acc; }})",
                        ct = c_type(e)
                    ),
                    (**e).clone(),
                )
            }
            (CTy::Arr(e), "max") if matches!(**e, CTy::Int | CTy::Float | CTy::F32) => {
                let an = arr_name(e);
                (
                    format!(
                        "({{ {an} _s = {rc}; {ct} _acc = _s.len > 0 ? _s.data[0] : 0; for (int64_t _i = 1; _i < _s.len; _i++) if (_s.data[_i] > _acc) _acc = _s.data[_i]; _acc; }})",
                        ct = c_type(e)
                    ),
                    (**e).clone(),
                )
            }
            (CTy::Arr(e), "first") => {
                let an = arr_name(e);
                (
                    format!(
                        "({{ {an} _s = {rc}; _s.len > 0 ? _s.data[0] : {}; }})",
                        zero_value(e)
                    ),
                    (**e).clone(),
                )
            }
            (CTy::Arr(e), "last") => {
                let an = arr_name(e);
                (
                    format!(
                        "({{ {an} _s = {rc}; _s.len > 0 ? _s.data[_s.len - 1] : {}; }})",
                        zero_value(e)
                    ),
                    (**e).clone(),
                )
            }
            (CTy::Arr(_), "length") => (format!("({rc}).len"), CTy::Int),
            (CTy::Arr(e), "get") => {
                let an = arr_name(e);
                let t = self.temp();
                (
                    format!(
                        "({{ {an} _s = {rc}; int64_t {t} = {}; \
                         ({t} >= 0 && {t} < _s.len) ? _s.data[{t}] : {}; }})",
                        arg0(),
                        zero_value(e)
                    ),
                    (**e).clone(),
                )
            }
            (CTy::Arr(e), "slice") => {
                let an = arr_name(e);
                (
                    format!(
                        "({{ {an} _s = {rc}; {an} _r = {an}_new(); \
                         for (int64_t _i = ({}); _i < ({}) && _i < _s.len; _i++) {an}_push(&_r, _s.data[_i]); _r; }})",
                        arg0(),
                        arg1()
                    ),
                    CTy::Arr(e.clone()),
                )
            }
            (CTy::Str | CTy::Unknown, "trim") => (format!("maca_trim({rc})"), CTy::Str),
            (CTy::Str | CTy::Unknown, "upper") => (format!("maca_upper({rc})"), CTy::Str),
            (CTy::Str | CTy::Unknown, "lower") => (format!("maca_lower({rc})"), CTy::Str),
            (CTy::Str | CTy::Unknown, "contains") => {
                (format!("maca_contains({rc}, {})", arg0()), CTy::Bool)
            }
            (CTy::Str | CTy::Unknown, "starts_with") => {
                (format!("maca_starts_with({rc}, {})", arg0()), CTy::Bool)
            }
            (CTy::Str | CTy::Unknown, "ends_with") => {
                (format!("maca_ends_with({rc}, {})", arg0()), CTy::Bool)
            }
            (CTy::Str | CTy::Unknown, "index_of") => {
                (format!("maca_index_of({rc}, {})", arg0()), CTy::Int)
            }
            (CTy::Str | CTy::Unknown, "replace") => (
                format!("maca_replace({rc}, {}, {})", arg0(), arg1()),
                CTy::Str,
            ),
            (CTy::Str | CTy::Unknown, "substr") => (
                format!("maca_substr({rc}, {}, {})", arg0(), arg1()),
                CTy::Str,
            ),
            (CTy::Str, "slice") => (
                format!("maca_str_slice({rc}, {}, {})", arg0(), arg1()),
                CTy::Str,
            ),
            (CTy::Str | CTy::Unknown, "repeat") => {
                (format!("maca_repeat({rc}, {})", arg0()), CTy::Str)
            }
            (CTy::Str | CTy::Unknown, "pad_start") => (
                format!(
                    "maca_pad_start({rc}, {}, {})",
                    arg0(),
                    if a.len() > 1 {
                        arg1()
                    } else {
                        "\" \"".to_string()
                    }
                ),
                CTy::Str,
            ),
            (CTy::Str | CTy::Unknown, "pad_end") => (
                format!(
                    "maca_pad_end({rc}, {}, {})",
                    arg0(),
                    if a.len() > 1 {
                        arg1()
                    } else {
                        "\" \"".to_string()
                    }
                ),
                CTy::Str,
            ),
            (CTy::Str | CTy::Unknown, "pad_center") => (
                format!(
                    "maca_pad_center({rc}, {}, {})",
                    arg0(),
                    if a.len() > 1 {
                        arg1()
                    } else {
                        "\" \"".to_string()
                    }
                ),
                CTy::Str,
            ),
            (CTy::Float | CTy::Int | CTy::Unknown, "fixed") => {
                (format!("maca_fixed((double)({rc}), {})", arg0()), CTy::Str)
            }
            (CTy::Str | CTy::Unknown, "split") => {
                self.note_arr(&CTy::Arr(Box::new(CTy::Str)));
                let sep = arg0();
                (
                    format!(
                        "({{ int64_t _sn; maca_str* _sd = maca_split({rc}, {sep}, &_sn); \
                         StrArr _sr = StrArr_new(); for (int64_t _si = 0; _si < _sn; _si++) StrArr_push(&_sr, _sd[_si]); _sr; }})"
                    ),
                    CTy::Arr(Box::new(CTy::Str)),
                )
            }
            (CTy::Str | CTy::Unknown, "length") => (format!("maca_strlen({rc})"), CTy::Int),
            (CTy::Str | CTy::Unknown, "at") => (format!("maca_str_at({rc}, {})", arg0()), CTy::Str),
            (CTy::Str | CTy::Unknown, "chars") => {
                self.note_arr(&CTy::Arr(Box::new(CTy::Str)));
                (
                    format!(
                        "({{ maca_str _cs = {rc}; int64_t _cn = maca_strlen(_cs); \
                         StrArr _cr = StrArr_new(); for (int64_t _ci = 0; _ci < _cn; _ci++) StrArr_push(&_cr, maca_str_at(_cs, _ci)); _cr; }})"
                    ),
                    CTy::Arr(Box::new(CTy::Str)),
                )
            }
            (CTy::Str | CTy::Unknown, "is_whitespace") => {
                (format!("maca_is_space({rc})"), CTy::Bool)
            }
            (CTy::Str | CTy::Unknown, "is_ascii_digit") => {
                (format!("maca_is_digit({rc})"), CTy::Bool)
            }
            (CTy::Str | CTy::Unknown, "is_alpha") => (format!("maca_is_alpha({rc})"), CTy::Bool),
            (CTy::Arr(e), "parallel") if matches!(**e, CTy::Int) => {
                let f = a.first().cloned().unwrap_or_default();
                (
                    format!(
                        "({{ int64_t _pn = {rc}.len; int64_t* _pp = maca_parallel_i64({rc}.data, _pn, {f}, 4); \
                         IntArr _pr = IntArr_new(); for (int64_t _pi = 0; _pi < _pn; _pi++) IntArr_push(&_pr, _pp[_pi]); _pr; }})"
                    ),
                    CTy::Arr(Box::new(CTy::Int)),
                )
            }
            _ => {
                if !self.fns.contains_key(method)
                    && !self.generics.contains_key(method)
                    && known_method(rty, method)
                {
                    self.problem(format!(
                        "`{}` has no `{method}`: the method exists for simpler \
                         element types, not this one",
                        c_type(rty)
                    ));
                }
                let ret = self
                    .fns
                    .get(method)
                    .map(|(_, r)| r.clone())
                    .unwrap_or(CTy::Unknown);
                (
                    format!(
                        "{}({rc}{}{})",
                        cid(method),
                        if a.is_empty() { "" } else { ", " },
                        a.join(", ")
                    ),
                    ret,
                )
            }
        }
    }

    /// A writable location for `target = value`.
    fn lvalue(&mut self, env: &mut Env, target: &Expr) -> Option<(String, Option<CTy>)> {
        match target {
            Expr::Index { base, index } => {
                let (bc, bty) = self.expr(env, base, None);
                let (ic, _) = self.expr(env, index, Some(&CTy::Int));
                let ety = match bty {
                    CTy::Arr(e) => Some(*e),
                    _ => None,
                };
                Some((format!("({bc}).data[{ic}]"), ety))
            }
            Expr::Field { base, name } => {
                let (bc, bty) = self.expr(env, base, None);
                let fty = match &bty {
                    CTy::Rec(r) => self
                        .records
                        .get(r)
                        .and_then(|fs| fs.iter().find(|(n, _)| n == name))
                        .map(|(_, t)| t.clone()),
                    _ => None,
                };
                Some((format!("({bc}).{}", cid(name)), fty))
            }
            Expr::Ident(n) if lookup(env, n).is_none() && self.written_lets.contains(n) => {
                let (slot, ty) = self.ident(env, n);
                Some((slot, Some(ty)))
            }
            _ => None,
        }
    }

    /// `base[index]` is element access.
    fn index(&mut self, env: &mut Env, base: &Expr, idx: &Expr) -> (String, CTy) {
        let (bc, bty) = self.expr(env, base, None);
        let (ic, _) = self.expr(env, idx, Some(&CTy::Int));
        match bty {
            CTy::Arr(e) => (format!("({bc}).data[{ic}]"), *e),
            CTy::Str => (format!("maca_str_at({bc}, {ic})"), CTy::Str),
            _ => (format!("({bc}).data[{ic}]"), CTy::Unknown),
        }
    }

    fn field(&mut self, env: &mut Env, base: &Expr, name: &str) -> (String, CTy) {
        if let Expr::Ident(m) = base
            && self.modules.contains(m)
        {
            if m == "dirs" && name == "data" {
                return ("maca_dirs_data()".into(), CTy::Str);
            }
            return (format!("/* {m}.{name} */ \"\""), CTy::Unknown);
        }
        let (bc, bty) = self.expr(env, base, None);
        let fty = match &bty {
            CTy::Rec(r) => self
                .records
                .get(r)
                .and_then(|fs| fs.iter().find(|(n, _)| n == name))
                .map(|(_, t)| t.clone())
                .unwrap_or(CTy::Unknown),
            _ => CTy::Unknown,
        };
        (format!("({bc}).{}", cid(name)), fty)
    }

    fn binary(&mut self, env: &mut Env, op: BinOp, lhs: &Expr, rhs: &Expr) -> (String, CTy) {
        let (lc, lt) = self.expr(env, lhs, None);
        let left_pieces = self.concat_pieces.take();
        let (rc, rt) = self.expr(env, rhs, None);
        self.concat_pieces = None;

        if matches!(lt, CTy::Rec(_) | CTy::Sum(_))
            && let Some(name) = overload_name(op)
            && let Some((_, ret)) = self.fns.get(name).cloned()
        {
            return (format!("{}({lc}, {rc})", cid(name)), ret);
        }

        use BinOp::*;
        match op {
            Div if matches!(lt, CTy::Str) => (format!("maca_path_join({lc}, {rc})"), CTy::Str),
            Concat if matches!(lt, CTy::Str) || matches!(rt, CTy::Str) => {
                for t in [&lt, &rt] {
                    if !can_concat(t) {
                        self.problem(format!(
                            "`{}` has no text form: `++` joins strings, so \
                             convert it first",
                            c_type(t)
                        ));
                    }
                }
                let mut pieces = left_pieces
                    .unwrap_or_else(|| vec![Piece::operand(&lc, &lt, self.fresh.allocates(lhs))]);
                pieces.push(Piece::operand(&rc, &rt, self.fresh.allocates(rhs)));
                let code = self.concat(&pieces);
                self.concat_pieces = Some(pieces);
                (code, CTy::Str)
            }
            Concat => {
                let an = arr_name(match &lt {
                    CTy::Arr(e) => e,
                    _ => &CTy::Unknown,
                });
                (format!("{an}_concat({lc}, {rc})"), lt)
            }
            Add | Sub | Mul | Div | Mod | Shl | Shr => {
                let o = bin_op(op);
                (format!("({lc} {o} {rc})"), lt)
            }
            Eq | Ne => {
                if matches!(lt, CTy::Str) {
                    let neg = if op == Ne { "!" } else { "" };
                    (format!("{neg}maca_str_eq({lc}, {rc})"), CTy::Bool)
                } else {
                    (format!("({lc} {} {rc})", bin_op(op)), CTy::Bool)
                }
            }
            Lt | Gt | Le | Ge => (format!("({lc} {} {rc})", bin_op(op)), CTy::Bool),
            And | Or => (format!("({lc} {} {rc})", bin_op(op)), CTy::Bool),
            Union | Pipe => (lc, lt),
        }
    }

    fn arg(&mut self, env: &mut Env, a: &Arg) -> String {
        self.arg_typed(env, a).0
    }
    fn arg_typed(&mut self, env: &mut Env, a: &Arg) -> (String, CTy) {
        self.arg_expected(env, a, None)
    }
    /// Lower a call argument with an optional expected type, which lets an empty list literal `[]` take its element type from the callee's parameter (e.g. `scan(cs, 0, [])` where the 3rd param is `Token[]`).
    fn arg_expected(&mut self, env: &mut Env, a: &Arg, expected: Option<&CTy>) -> (String, CTy) {
        match a {
            Arg::Pos(e) | Arg::Named { value: e, .. } | Arg::Directive { value: e, .. } => {
                self.expr(env, e, expected)
            }
        }
    }

    fn interp(&mut self, env: &mut Env, parts: &[StrPart]) -> String {
        if parts.is_empty() {
            return "\"\"".into();
        }
        if parts.len() == 1
            && let StrPart::Text(t) = &parts[0]
        {
            return c_str(t);
        }
        let mut pieces = Vec::new();
        for p in parts {
            pieces.push(match p {
                StrPart::Text(t) => Piece::literal(c_str(t)),
                StrPart::Interp(e) => {
                    let (c, t) = self.expr(env, e, None);
                    Piece::rendered(&c, &t, self.fresh.allocates(e))
                }
            });
        }
        self.concat(&pieces)
    }

    /// A sum's JSON spelling: the variant's own name in lower case, and back again.
    fn emit_sum_json_name(&mut self, name: &str, vars: &[String], tagged: bool) {
        self.push(&format!("static maca_str {name}_to_json_name({name} v) {{"));
        self.push(&format!(
            "    switch (v{}) {{",
            if tagged { ".tag" } else { "" }
        ));
        for v in vars {
            let tag = if tagged {
                format!("{name}_tag_{v}")
            } else {
                format!("{name}_{v}")
            };
            self.push(&format!(
                "        case {tag}: return \"{}\";",
                v.to_lowercase()
            ));
        }
        self.push("    }");
        self.push("    return \"\";");
        self.push("}");
        self.push(&format!(
            "static {name} {name}_from_json_name(maca_str s, maca_str field) {{"
        ));
        for v in vars {
            let make = if tagged {
                let zeros = self
                    .variant_payloads
                    .get(v)
                    .map(|p| {
                        p.iter()
                            .map(|t| format!("({}){{0}}", c_type(t)))
                            .collect::<Vec<_>>()
                            .join(", ")
                    })
                    .unwrap_or_default();
                format!("{name}_{v}({zeros})")
            } else {
                format!("{name}_{v}")
            };
            self.push(&format!(
                "    if (maca_str_eq(s, \"{}\")) return {make};",
                v.to_lowercase()
            ));
        }
        let choices: Vec<String> = vars.iter().map(|v| v.to_lowercase()).collect();
        self.push(&format!(
            "    maca_fail(maca_concat_n(5, \"field `\", field, \"`: \\\"\", s, \
             \"\\\" is not one of {}\"));",
            choices.join(", ")
        ));
        let first = &vars[0];
        let make_first = if tagged {
            let zeros = self
                .variant_payloads
                .get(first)
                .map(|p| {
                    p.iter()
                        .map(|t| format!("({}){{0}}", c_type(t)))
                        .collect::<Vec<_>>()
                        .join(", ")
                })
                .unwrap_or_default();
            format!("{name}_{first}({zeros})")
        } else {
            format!("{name}_{first}")
        };
        self.push(&format!("    return {make_first};"));
        self.push("}");
    }

    fn emit_to_json(&mut self, name: &str) {
        let fields = self.records[name].clone();
        self.push(&format!("static maca_str {name}_to_json({name} v) {{"));
        self.push("    maca_sb sb; maca_sb_init(&sb); maca_sb_putc(&sb, '{');");
        for (i, (fname, fty)) in fields.iter().enumerate() {
            if i > 0 {
                self.push("    maca_sb_putc(&sb, ',');");
            }
            self.push(&format!("    maca_sb_puts(&sb, \"\\\"{fname}\\\":\");"));
            self.emit_json_value(&format!("v.{}", cid(fname)), fty);
        }
        self.push("    maca_sb_putc(&sb, '}');");
        self.push("    return maca_sb_finish(&sb);");
        self.push("}");
    }

    fn emit_json_value(&mut self, access: &str, t: &CTy) {
        match t {
            CTy::Int => self.push(&format!("    maca_sb_puts(&sb, maca_from_int({access}));")),
            CTy::Float | CTy::F32 => self.push(&format!(
                "    maca_sb_puts(&sb, maca_from_float({access}));"
            )),
            CTy::Bool => self.push(&format!(
                "    maca_sb_puts(&sb, {access} ? \"true\" : \"false\");"
            )),
            CTy::Str => self.push(&format!("    maca_sb_put_json_str(&sb, {access});")),
            CTy::Sum(s) => self.push(&format!(
                "    maca_sb_put_json_str(&sb, {s}_to_json_name({access}));"
            )),
            CTy::Rec(r) => self.push(&format!("    maca_sb_puts(&sb, {r}_to_json({access}));")),
            CTy::Arr(e) => {
                let idx = self.temp();
                self.push("    maca_sb_putc(&sb, '[');");
                self.push(&format!(
                    "    for (int64_t {idx} = 0; {idx} < {access}.len; {idx}++) {{ if ({idx}) maca_sb_putc(&sb, ',');"
                ));
                let inner = format!("{access}.data[{idx}]");
                self.emit_json_value(&inner, e);
                self.push("    }");
                self.push("    maca_sb_putc(&sb, ']');");
            }
            CTy::Map(_)
            | CTy::Unit
            | CTy::Unknown
            | CTy::Vec { .. }
            | CTy::Future
            | CTy::Closure(_)
            | CTy::Closure2(_) => self.push("    maca_sb_puts(&sb, \"null\");"),
        }
    }

    fn emit_from_json(&mut self, name: &str) {
        let fields = self.records[name].clone();
        self.push(&format!("static {name} {name}_from_json(maca_json* j) {{"));
        self.push(&format!("    j = maca_json_object(j, \"{name}\");"));
        self.push(&format!("    {name} v;"));
        for (fname, fty) in &fields {
            self.emit_json_read(&format!("v.{}", cid(fname)), fname, fty);
        }
        self.push("    return v;");
        self.push("}");
    }

    fn emit_json_read(&mut self, dest: &str, key: &str, t: &CTy) {
        let want = |kind: &str| format!("maca_json_want(j, \"{key}\", {kind})");
        match t {
            CTy::Int => self.push(&format!("    {dest} = maca_json_int({});", want("MJ_NUM"))),
            CTy::Float | CTy::F32 => self.push(&format!(
                "    {dest} = maca_json_float({});",
                want("MJ_NUM")
            )),
            CTy::Bool => self.push(&format!(
                "    {dest} = maca_json_bool({});",
                want("MJ_BOOL")
            )),
            CTy::Str => self.push(&format!("    {dest} = maca_json_str({});", want("MJ_STR"))),
            CTy::Sum(s) => self.push(&format!(
                "    {dest} = {s}_from_json_name(maca_json_str({}), \"{key}\");",
                want("MJ_STR")
            )),
            CTy::Rec(r) => self.push(&format!("    {dest} = {r}_from_json({});", want("MJ_OBJ"))),
            CTy::Arr(e) => {
                let an = arr_name(e);
                let a = self.temp();
                let idx = self.temp();
                self.push(&format!(
                    "    {{ maca_json* {a} = {}; {an} _acc = {an}_new();",
                    want("MJ_ARR")
                ));
                self.push(&format!(
                    "      for (int64_t {idx} = 0; {idx} < {a}->arr.len; {idx}++) {{"
                ));
                let elem_read = json_read_inline(&format!("{a}->arr.items[{idx}]"), e, key);
                self.push(&format!("        {an}_push(&_acc, {elem_read});"));
                self.push("      }");
                self.push(&format!("      {dest} = _acc; }}"));
            }
            CTy::Map(v) => {
                let mn = map_name(v);
                let o = self.temp();
                let idx = self.temp();
                self.push(&format!(
                    "    {{ maca_json* {o} = {}; {mn} _m = {mn}_new();",
                    want("MJ_OBJ")
                ));
                self.push(&format!(
                    "      for (int64_t {idx} = 0; {idx} < {o}->obj.len; {idx}++) {{"
                ));
                let read = json_read_inline(&format!("{o}->obj.vals[{idx}]"), v, key);
                self.push(&format!(
                    "        {mn}_set(&_m, {o}->obj.keys[{idx}], {read});"
                ));
                self.push("      }");
                self.push(&format!("      {dest} = _m; }}"));
            }
            CTy::Unit
            | CTy::Unknown
            | CTy::Vec { .. }
            | CTy::Future
            | CTy::Closure(_)
            | CTy::Closure2(_) => {
                self.push(&format!("    {dest} = {};", zero_value(t)));
            }
        }
    }
}

/// Inline reader for one element of a field's list or map, which reports under that field's name.
fn json_read_inline(j: &str, t: &CTy, field: &str) -> String {
    match t {
        CTy::Int => format!("maca_json_int({j})"),
        CTy::Float => format!("maca_json_float({j})"),
        CTy::Bool => format!("maca_json_bool({j})"),
        CTy::Str => format!("maca_json_str({j})"),
        CTy::Sum(s) => format!("{s}_from_json_name(maca_json_str({j}), \"{field}\")"),
        CTy::Rec(r) => format!("{r}_from_json({j})"),
        _ => "0".into(),
    }
}

fn body_as_block(e: &Expr) -> &Expr {
    e
}

fn is_control(e: &Expr) -> bool {
    matches!(
        e,
        Expr::For { .. }
            | Expr::While { .. }
            | Expr::Break
            | Expr::Continue
            | Expr::Return(_)
            | Expr::Match { .. }
            | Expr::If { .. }
            | Expr::Block(_)
    )
}

/// The tail (last) expression of a statement block, if any: the value it yields.
fn tail_expr(stmts: &[Stmt]) -> Option<&Expr> {
    match stmts.last() {
        Some(Stmt::Expr(e)) => Some(e),
        _ => None,
    }
}

fn c_type(t: &CTy) -> String {
    match t {
        CTy::Int => "int64_t".into(),
        CTy::Float => "double".into(),
        CTy::F32 => "float".into(),
        CTy::Str => "maca_str".into(),
        CTy::Bool => "bool".into(),
        CTy::Unit => "int64_t".into(),
        CTy::Rec(n) | CTy::Sum(n) => n.clone(),
        CTy::Arr(e) => arr_name(e),
        CTy::Map(v) => map_name(v),
        CTy::Vec { name, .. } => name.clone(),
        CTy::Future => "maca_future*".into(),
        CTy::Closure(_) => "maca_closure".into(),
        CTy::Closure2(_) => "maca_closure2".into(),
        CTy::Unknown => "int64_t".into(),
    }
}

/// The closure struct for a given arity.
fn closure_ty(arity: usize, ret: CTy) -> CTy {
    if arity >= 2 {
        CTy::Closure2(Box::new(ret))
    } else {
        CTy::Closure(Box::new(ret))
    }
}

/// Is `method` one the checker accepts on a receiver of this type?
fn known_method(rty: &CTy, method: &str) -> bool {
    match rty {
        CTy::Arr(_) => maca_core::LIST_METHODS.contains(&method),
        CTy::Str => maca_core::STR_METHODS.contains(&method),
        CTy::Map(_) => maca_core::MAP_METHODS.contains(&method),
        _ => false,
    }
}

/// The user function a callee names, written either way round.
fn called_name(callee: &Expr) -> Option<&str> {
    match callee {
        Expr::Ident(f) => Some(f),
        Expr::Field { name, .. } => Some(name),
        _ => None,
    }
}

fn arr_name(elem: &CTy) -> String {
    match elem {
        CTy::Int => "IntArr".into(),
        CTy::Float => "FloatArr".into(),
        CTy::F32 => "F32Arr".into(),
        CTy::Str => "StrArr".into(),
        CTy::Bool => "BoolArr".into(),
        CTy::Rec(n) | CTy::Sum(n) => format!("{n}Arr"),
        CTy::Vec { name, .. } => format!("{name}Arr"),
        CTy::Arr(e) => format!("{}Arr", arr_name(e)),
        CTy::Map(v) => format!("{}Arr", map_name(v)),
        CTy::Unit | CTy::Unknown | CTy::Future | CTy::Closure(_) | CTy::Closure2(_) => {
            "IntArr".into()
        }
    }
}

/// The monomorphized map type name for a value type.
fn map_name(val: &CTy) -> String {
    match val {
        CTy::Int => "IntMap".into(),
        CTy::Float => "FloatMap".into(),
        CTy::F32 => "F32Map".into(),
        CTy::Str => "StrMap".into(),
        CTy::Bool => "BoolMap".into(),
        CTy::Rec(n) | CTy::Sum(n) => format!("{n}Map"),
        CTy::Arr(e) => format!("{}Map", arr_name(e)),
        CTy::Map(v) => format!("{}Map", map_name(v)),
        _ => "IntMap".into(),
    }
}

fn zero_value(t: &CTy) -> String {
    match t {
        CTy::Str => "\"\"".into(),
        CTy::Bool => "false".into(),
        CTy::F32 | CTy::Float => "0.0".into(),
        CTy::Arr(e) => format!("{}_new()", arr_name(e)),
        CTy::Map(v) => format!("{}_new()", map_name(v)),
        CTy::Int | CTy::Unit | CTy::Unknown | CTy::Future => "0".into(),
        _ => format!("({{ {0} _z; memset(&_z, 0, sizeof _z); _z; }})", c_type(t)),
    }
}

fn to_str(code: &str, t: &CTy) -> String {
    match t {
        CTy::Str => code.to_string(),
        CTy::Int => format!("maca_from_int({code})"),
        CTy::Float | CTy::F32 => format!("maca_from_float({code})"),
        CTy::Bool => format!("maca_from_bool({code})"),
        CTy::Unknown => format!("maca_from_int({code})"),
        CTy::Sum(s) => format!("{s}_to_str({code})"),
        _ => code.to_string(),
    }
}

fn console_fn(name: &str) -> Option<&'static str> {
    Some(match name {
        "emerg" | "panic" => "maca_emerg",
        "alert" => "maca_alert",
        "crit" => "maca_crit",
        "err" => "maca_err",
        "warn" => "maca_warn",
        "notice" => "maca_notice",
        "info" => "maca_info",
        "debug" => "maca_debug",
        "print" => "maca_print",
        _ => return None,
    })
}

/// The canonical function name an operator overloads to for user types (`a + b` → `add(a, b)`), or `None` for operators that never overload.
fn overload_name(op: BinOp) -> Option<&'static str> {
    Some(match op {
        BinOp::Add => "add",
        BinOp::Sub => "sub",
        BinOp::Mul => "mul",
        BinOp::Div => "div",
        BinOp::Concat => "concat",
        BinOp::Eq => "eq",
        BinOp::Ne => "ne",
        BinOp::Lt => "lt",
        BinOp::Gt => "gt",
        BinOp::Le => "le",
        BinOp::Ge => "ge",
        _ => return None,
    })
}

fn bin_op(op: BinOp) -> &'static str {
    match op {
        BinOp::Add => "+",
        BinOp::Sub => "-",
        BinOp::Mul => "*",
        BinOp::Div => "/",
        BinOp::Mod => "%",
        BinOp::Shl => "<<",
        BinOp::Shr => ">>",
        BinOp::Eq => "==",
        BinOp::Ne => "!=",
        BinOp::Lt => "<",
        BinOp::Gt => ">",
        BinOp::Le => "<=",
        BinOp::Ge => ">=",
        BinOp::And => "&&",
        BinOp::Or => "||",
        _ => "/*op*/",
    }
}

/// The heap cell a local lives in when a nested definition assigns it.
fn cell_of(name: &str) -> String {
    format!("_cell_{}", cid(name))
}

/// The C name a captured variable goes by: its cell when the capture is shared, its own name when it is a copy.
fn cell_or_name(shared: &HashSet<String>, name: &str) -> String {
    if shared.contains(name) {
        cell_of(name)
    } else {
        cid(name)
    }
}

/// Every local of this body that a definition nested inside it assigns.
fn writes_below(body: &Expr) -> HashSet<String> {
    let mut out = HashSet::new();
    writes_under_nesting(body, &HashSet::new(), false, &mut out);
    out
}

/// Every local a nested definition inside this body assigns, so the write has to reach the enclosing scope.
fn captured_writes(body: Option<&FnBody>) -> HashSet<String> {
    let mut out = HashSet::new();
    match body {
        Some(FnBody::Expr(e)) => writes_under_nesting(e, &HashSet::new(), false, &mut out),
        Some(FnBody::Block(ss)) => stmt_writes_under_nesting(ss, &HashSet::new(), false, &mut out),
        None => {}
    }
    out
}

fn writes_under_nesting(
    e: &Expr,
    bound: &HashSet<String>,
    inside: bool,
    out: &mut HashSet<String>,
) {
    match e {
        Expr::Lambda { params, body, .. } => {
            let mut b = bound.clone();
            b.extend(params.iter().map(|p| p.name.clone()));
            writes_under_nesting(body, &b, true, out);
        }
        Expr::Assign { target, value } => {
            if let Expr::Ident(n) = &**target
                && inside
                && bound.contains(n)
            {
                out.insert(n.clone());
            }
            writes_under_nesting(value, bound, inside, out);
        }
        Expr::Block(ss) => stmt_writes_under_nesting(ss, bound, inside, out),
        Expr::If { cond, then, els } => {
            writes_under_nesting(cond, bound, inside, out);
            stmt_writes_under_nesting(then, bound, inside, out);
            if let Some(e) = els {
                stmt_writes_under_nesting(e, bound, inside, out);
            }
        }
        Expr::For { pat, iter, body } => {
            writes_under_nesting(iter, bound, inside, out);
            let mut b = bound.clone();
            bind_pat(pat, &mut b);
            stmt_writes_under_nesting(body, &b, inside, out);
        }
        Expr::While { cond, body } => {
            writes_under_nesting(cond, bound, inside, out);
            stmt_writes_under_nesting(body, bound, inside, out);
        }
        Expr::Match { scrut, arms } => {
            writes_under_nesting(scrut, bound, inside, out);
            for a in arms {
                let mut b = bound.clone();
                bind_pat(&a.pat, &mut b);
                writes_under_nesting(&a.body, &b, inside, out);
            }
        }
        other => walk_children(other, &mut |c| writes_under_nesting(c, bound, inside, out)),
    }
}

fn stmt_writes_under_nesting(
    stmts: &[Stmt],
    bound: &HashSet<String>,
    inside: bool,
    out: &mut HashSet<String>,
) {
    let mut b = bound.clone();
    for s in stmts {
        match s {
            Stmt::Bind(bd) => {
                writes_under_nesting(&bd.value, &b, inside, out);
                if let Expr::Ident(n) = &bd.target {
                    if inside && b.contains(n) {
                        out.insert(n.clone());
                    }
                    b.insert(n.clone());
                } else {
                    writes_under_nesting(&bd.target, &b, inside, out);
                }
            }
            Stmt::Expr(e) | Stmt::Alias { value: e, .. } => {
                writes_under_nesting(e, &b, inside, out)
            }
            Stmt::Fn(f) => {
                let mut inner = b.clone();
                inner.extend(f.params.iter().map(|p| p.name.clone()));
                match &f.body {
                    Some(FnBody::Expr(e)) => writes_under_nesting(e, &inner, true, out),
                    Some(FnBody::Block(ss)) => stmt_writes_under_nesting(ss, &inner, true, out),
                    None => {}
                }
                b.insert(f.name.clone());
            }
            Stmt::Import(_) => {}
        }
    }
}

/// Visit every statement inside a definition, including the ones a control-flow expression holds.
fn for_each_stmt(f: &FnDef, visit: &mut impl FnMut(&Stmt)) {
    match &f.body {
        Some(FnBody::Expr(e)) => stmts_in_expr(e, visit),
        Some(FnBody::Block(ss)) => stmts_in_block(ss, visit),
        None => {}
    }
}

fn stmts_in_block(stmts: &[Stmt], visit: &mut impl FnMut(&Stmt)) {
    for s in stmts {
        visit(s);
        match s {
            Stmt::Bind(b) => stmts_in_expr(&b.value, visit),
            Stmt::Expr(e) | Stmt::Alias { value: e, .. } => stmts_in_expr(e, visit),
            Stmt::Fn(f) => for_each_stmt(f, visit),
            Stmt::Import(_) => {}
        }
    }
}

fn stmts_in_expr(e: &Expr, visit: &mut impl FnMut(&Stmt)) {
    match e {
        Expr::Block(ss) | Expr::While { body: ss, .. } | Expr::For { body: ss, .. } => {
            stmts_in_block(ss, visit)
        }
        Expr::If { then, els, .. } => {
            stmts_in_block(then, visit);
            if let Some(els) = els {
                stmts_in_block(els, visit);
            }
        }
        other => walk_children(other, &mut |c| stmts_in_expr(c, visit)),
    }
}

/// Escape a Maca identifier that collides with a C (or common C++) reserved word so the emitted C stays valid.
fn cid(name: &str) -> String {
    const KW: &[&str] = &[
        "auto",
        "break",
        "case",
        "char",
        "const",
        "continue",
        "default",
        "do",
        "double",
        "else",
        "enum",
        "extern",
        "float",
        "for",
        "goto",
        "if",
        "inline",
        "int",
        "long",
        "register",
        "restrict",
        "return",
        "short",
        "signed",
        "sizeof",
        "static",
        "struct",
        "switch",
        "typedef",
        "union",
        "unsigned",
        "void",
        "volatile",
        "while",
        "bool",
        "complex",
        "imaginary",
        "new",
        "delete",
        "class",
        "this",
        "template",
        "namespace",
        "operator",
        "try",
        "catch",
        "throw",
        "public",
        "private",
        "protected",
        "virtual",
        "friend",
        "using",
    ];
    if KW.contains(&name) {
        format!("{name}_mc")
    } else {
        name.to_string()
    }
}

fn c_str(s: &str) -> String {
    let mut out = String::from("\"");
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            '\r' => out.push_str("\\r"),
            _ => out.push(c),
        }
    }
    out.push('"');
    out
}

/// Shallow best-effort type for a top-level `let` value (no env).
fn infer_cty_shallow(e: &Expr) -> CTy {
    match e {
        Expr::Int(_) => CTy::Int,
        Expr::Float(_) => CTy::Float,
        Expr::Bool(_) => CTy::Bool,
        Expr::Str(_) | Expr::Path(_) => CTy::Str,
        Expr::Binary {
            op: BinOp::Div,
            lhs,
            rhs,
        } => {
            if matches!(**lhs, Expr::Str(_) | Expr::Path(_))
                || matches!(**rhs, Expr::Str(_) | Expr::Path(_))
            {
                CTy::Str
            } else {
                infer_cty_shallow(lhs)
            }
        }
        Expr::List(items) => {
            let elem = match items.first() {
                Some(first) => infer_cty_shallow(first),
                None => CTy::Str,
            };
            CTy::Arr(Box::new(elem))
        }
        Expr::Lambda { params, .. } => closure_ty(params.len(), CTy::Int),
        Expr::Call { callee, .. } => match callee.as_ref() {
            Expr::Ident(f) => match f.as_str() {
                "ord" | "int" | "len" => CTy::Int,
                "chr" | "str" => CTy::Str,
                "float" | "sqrt" | "floor" | "ceil" | "round" | "pow" => CTy::Float,
                _ => CTy::Str,
            },
            _ => CTy::Str,
        },
        Expr::Binary {
            op: BinOp::Add | BinOp::Sub | BinOp::Mul | BinOp::Mod | BinOp::Shl | BinOp::Shr,
            lhs,
            ..
        } => infer_cty_shallow(lhs),
        Expr::Binary {
            op:
                BinOp::Eq
                | BinOp::Ne
                | BinOp::Lt
                | BinOp::Gt
                | BinOp::Le
                | BinOp::Ge
                | BinOp::And
                | BinOp::Or,
            ..
        } => CTy::Bool,
        Expr::Unary { op: UnOp::Not, .. } => CTy::Bool,
        Expr::Unary {
            op: UnOp::Neg,
            expr,
        } => infer_cty_shallow(expr),
        _ => CTy::Str,
    }
}

fn import_name(im: &Import) -> Option<String> {
    match im {
        Import::Module(segs) => segs.last().cloned(),
        Import::Bare(n) | Import::Foreign { lang: n, .. } => Some(n.clone()),
        Import::Names { .. } | Import::Path(_) => None,
    }
}

/// `A | Circle(int) | Rect(int, int)` → variants with payload types (bare ident = nullary, `Name(T, …)` = a payload-carrying variant).
fn sum_variants(e: &Expr) -> Option<Vec<(String, Vec<Type>)>> {
    fn arg_ty(e: &Expr) -> Type {
        fn path(e: &Expr, out: &mut Vec<String>) {
            match e {
                Expr::Ident(n) => out.push(n.clone()),
                Expr::Field { base, name } => {
                    path(base, out);
                    out.push(name.clone());
                }
                _ => {}
            }
        }
        let mut segs = Vec::new();
        path(e, &mut segs);
        if segs.is_empty() {
            Type::Name(vec!["any".into()])
        } else {
            Type::Name(segs)
        }
    }
    fn go(e: &Expr, out: &mut Vec<(String, Vec<Type>)>) -> bool {
        match e {
            Expr::Ident(n) => {
                out.push((n.clone(), vec![]));
                true
            }
            Expr::Call { callee, args } => {
                if let Expr::Ident(n) = &**callee {
                    let tys = args.iter().map(|a| arg_ty(arg_expr(a))).collect();
                    out.push((n.clone(), tys));
                    true
                } else {
                    false
                }
            }
            Expr::Binary {
                op: BinOp::Union,
                lhs,
                rhs,
            } => go(lhs, out) && go(rhs, out),
            _ => false,
        }
    }
    let mut out = Vec::new();
    match e {
        Expr::Binary {
            op: BinOp::Union, ..
        } if go(e, &mut out) => Some(out),
        _ => None,
    }
}

/// A function is generic if any parameter or the return type mentions a type variable.
fn is_fn_type(t: Option<&Type>) -> bool {
    match t {
        Some(Type::Fn(..)) => true,
        Some(Type::Paren(inner)) => is_fn_type(Some(inner)),
        _ => false,
    }
}

fn fn_is_generic(f: &FnDef) -> bool {
    f.params
        .iter()
        .any(|p| p.ty.as_ref().is_some_and(type_has_tyvar))
        || f.ret.as_ref().is_some_and(type_has_tyvar)
}

fn type_has_tyvar(t: &Type) -> bool {
    match t {
        Type::Name(segs) => segs.len() == 1 && is_type_var_name(&segs[0]),
        Type::Array(i) | Type::Opt(i) | Type::Paren(i) => type_has_tyvar(i),
        Type::Apply(h, args) => type_has_tyvar(h) || args.iter().any(type_has_tyvar),
        Type::Fn(ps, r) => ps.iter().any(type_has_tyvar) || type_has_tyvar(r),
    }
}

/// Bind every type variable in `declared` to the matching part of `concrete`.
fn bind_vars(declared: &Type, concrete: &CTy, m: &mut HashMap<String, CTy>) {
    match (declared, concrete) {
        (Type::Name(segs), _) if segs.len() == 1 && is_type_var_name(&segs[0]) => {
            match m.get(&segs[0]) {
                Some(prev) if is_settled(prev) => {}
                _ => {
                    m.insert(segs[0].clone(), concrete.clone());
                }
            }
        }
        (Type::Array(inner), CTy::Arr(e)) => bind_vars(inner, e, m),
        (Type::Opt(inner) | Type::Paren(inner), _) => bind_vars(inner, concrete, m),
        (Type::Apply(_, args), CTy::Map(v)) => {
            if let Some(last) = args.last() {
                bind_vars(last, v, m);
            }
        }
        (Type::Fn(_, r), CTy::Closure(ret) | CTy::Closure2(ret)) => bind_vars(r, ret, m),
        _ => {}
    }
}

/// A generic function's specialized C name for a concrete argument tuple, e.g. `id__int`, `id__str`, `id__Box`.
fn is_settled(t: &CTy) -> bool {
    match t {
        CTy::Unknown => false,
        CTy::Arr(e) | CTy::Map(e) => is_settled(e),
        _ => true,
    }
}

fn mangle_name(name: &str, ctys: &[CTy]) -> String {
    let tags: Vec<String> = ctys.iter().map(cty_tag).collect();
    format!("{name}__{}", tags.join("_"))
}

/// Where late container definitions go; see `Cx::late_containers`.
const LATE_CONTAINERS: &str = "/* containers a specialization asked for */";

fn cty_tag(t: &CTy) -> String {
    match t {
        CTy::Map(v) => format!("{}map", cty_tag(v)),
        CTy::Int => "int".into(),
        CTy::Float => "f64".into(),
        CTy::F32 => "f32".into(),
        CTy::Str => "str".into(),
        CTy::Bool => "bool".into(),
        CTy::Unit => "unit".into(),
        CTy::Rec(n) | CTy::Sum(n) => n.clone(),
        CTy::Arr(e) => format!("arr{}", cty_tag(e)),
        CTy::Vec { name, .. } => name.clone(),
        CTy::Future => "future".into(),
        CTy::Closure(r) => format!("fn{}", cty_tag(r)),
        CTy::Closure2(r) => format!("fn2{}", cty_tag(r)),
        CTy::Unknown => "any".into(),
    }
}

/// Box a C value of type `t` into an `int64_t` for the closure-call boundary.
fn box_i64(code: &str, t: &CTy) -> String {
    match t {
        CTy::Str => format!("(int64_t)(intptr_t)({code})"),
        CTy::Float => format!("maca_box_f64({code})"),
        CTy::F32 => format!("maca_box_f64((double)({code}))"),
        CTy::Rec(_) | CTy::Sum(_) | CTy::Arr(_) | CTy::Map(_) | CTy::Vec { .. } => {
            let ct = c_type(t);
            format!(
                "({{ {ct}* _bx = ({ct}*)maca_alloc(sizeof({ct})); *_bx = ({code}); (int64_t)(intptr_t)_bx; }})"
            )
        }
        _ => format!("(int64_t)({code})"),
    }
}
/// Unbox an `int64_t` boundary value back to a C value of type `t`.
fn unbox_i64(code: &str, t: &CTy) -> String {
    match t {
        CTy::Str => format!("(maca_str)(intptr_t)({code})"),
        CTy::Bool => format!("(bool)({code})"),
        CTy::Float => format!("maca_unbox_f64({code})"),
        CTy::F32 => format!("(float)maca_unbox_f64({code})"),
        CTy::Rec(_) | CTy::Sum(_) | CTy::Arr(_) | CTy::Map(_) | CTy::Vec { .. } => {
            format!("(*({}*)(intptr_t)({code}))", c_type(t))
        }
        _ => format!("(int64_t)({code})"),
    }
}

/// Collect the names invoked as a call callee anywhere in `e` (`f(x)` adds `f`).
fn callee_idents(e: &Expr, out: &mut HashSet<String>) {
    match e {
        Expr::Call { callee, args } => {
            if let Expr::Ident(n) = &**callee {
                out.insert(n.clone());
            }
            callee_idents(callee, out);
            for a in args {
                callee_idents(arg_expr(a), out);
            }
        }
        Expr::Str(parts) => {
            for p in parts {
                if let StrPart::Interp(x) = p {
                    callee_idents(x, out);
                }
            }
        }
        Expr::Unary { expr, .. }
        | Expr::Try(expr)
        | Expr::Fail(expr)
        | Expr::Reify(expr)
        | Expr::Await(expr)
        | Expr::Spawn(expr)
        | Expr::Field { base: expr, .. } => callee_idents(expr, out),
        Expr::Index { base: a, index: b }
        | Expr::Binary { lhs: a, rhs: b, .. }
        | Expr::Assign {
            target: a,
            value: b,
        }
        | Expr::Range { lo: a, hi: b } => {
            callee_idents(a, out);
            callee_idents(b, out);
        }
        Expr::Ternary { cond, then, els } => {
            callee_idents(cond, out);
            callee_idents(then, out);
            callee_idents(els, out);
        }
        Expr::List(es) => es.iter().for_each(|x| callee_idents(x, out)),
        Expr::Record(fs) | Expr::Ctor { fields: fs, .. } => callee_field_idents(fs, out),
        Expr::With { base, fields } => {
            callee_idents(base, out);
            callee_field_idents(fields, out);
        }
        Expr::Lambda { body, .. } => callee_idents(body, out),
        Expr::If { cond, then, els } => {
            callee_idents(cond, out);
            callee_idents_stmts(then, out);
            if let Some(e) = els {
                callee_idents_stmts(e, out);
            }
        }
        Expr::For { iter, body, .. } => {
            callee_idents(iter, out);
            callee_idents_stmts(body, out);
        }
        Expr::While { cond, body } => {
            callee_idents(cond, out);
            callee_idents_stmts(body, out);
        }
        Expr::Match { scrut, arms } => {
            callee_idents(scrut, out);
            for arm in arms {
                if let Some(g) = &arm.guard {
                    callee_idents(g, out);
                }
                callee_idents(&arm.body, out);
            }
        }
        Expr::Block(stmts) => callee_idents_stmts(stmts, out),
        _ => {}
    }
}

fn callee_field_idents(fields: &[Field], out: &mut HashSet<String>) {
    for f in fields {
        match f {
            Field::Value { value, .. } | Field::Bare(value) => callee_idents(value, out),
            _ => {}
        }
    }
}

fn callee_idents_body(body: &FnBody, out: &mut HashSet<String>) {
    match body {
        FnBody::Expr(e) => callee_idents(e, out),
        FnBody::Block(stmts) => callee_idents_stmts(stmts, out),
    }
}

fn callee_idents_stmts(stmts: &[Stmt], out: &mut HashSet<String>) {
    for s in stmts {
        match s {
            Stmt::Bind(b) => callee_idents(&b.value, out),
            Stmt::Expr(e) => callee_idents(e, out),
            Stmt::Alias { value, .. } => callee_idents(value, out),
            _ => {}
        }
    }
}

/// Collect the free variables of `e`.
fn free_vars(e: &Expr, bound: &HashSet<String>, out: &mut HashSet<String>) {
    match e {
        Expr::Ident(n) => {
            if !bound.contains(n) {
                out.insert(n.clone());
            }
        }
        Expr::Int(_)
        | Expr::Float(_)
        | Expr::Bool(_)
        | Expr::Unit
        | Expr::Path(_)
        | Expr::Break
        | Expr::Continue => {}
        Expr::Str(parts) => {
            for p in parts {
                if let StrPart::Interp(x) = p {
                    free_vars(x, bound, out);
                }
            }
        }
        Expr::Unary { expr, .. }
        | Expr::Try(expr)
        | Expr::Fail(expr)
        | Expr::Reify(expr)
        | Expr::Await(expr)
        | Expr::Spawn(expr) => free_vars(expr, bound, out),
        Expr::Field { base, .. } => free_vars(base, bound, out),
        Expr::Index { base, index } => {
            free_vars(base, bound, out);
            free_vars(index, bound, out);
        }
        Expr::Binary { lhs, rhs, .. }
        | Expr::Assign {
            target: lhs,
            value: rhs,
        } => {
            free_vars(lhs, bound, out);
            free_vars(rhs, bound, out);
        }
        Expr::Range { lo, hi } => {
            free_vars(lo, bound, out);
            free_vars(hi, bound, out);
        }
        Expr::Ternary { cond, then, els } => {
            free_vars(cond, bound, out);
            free_vars(then, bound, out);
            free_vars(els, bound, out);
        }
        Expr::Call { callee, args } => {
            free_vars(callee, bound, out);
            for a in args {
                free_vars(arg_expr(a), bound, out);
            }
        }
        Expr::List(es) => es.iter().for_each(|x| free_vars(x, bound, out)),
        Expr::Record(fs) | Expr::Ctor { fields: fs, .. } => free_field_vars(fs, bound, out),
        Expr::With { base, fields } => {
            free_vars(base, bound, out);
            free_field_vars(fields, bound, out);
        }
        Expr::Lambda { params, body, .. } => {
            let mut b = bound.clone();
            for p in params {
                b.insert(p.name.clone());
            }
            free_vars(body, &b, out);
        }
        Expr::Return(v) => {
            if let Some(x) = v {
                free_vars(x, bound, out);
            }
        }
        Expr::If { cond, then, els } => {
            free_vars(cond, bound, out);
            free_vars_stmts(then, bound, out);
            if let Some(e) = els {
                free_vars_stmts(e, bound, out);
            }
        }
        Expr::For { pat, iter, body } => {
            free_vars(iter, bound, out);
            let mut b = bound.clone();
            bind_pat(pat, &mut b);
            free_vars_stmts(body, &b, out);
        }
        Expr::While { cond, body } => {
            free_vars(cond, bound, out);
            free_vars_stmts(body, bound, out);
        }
        Expr::Match { scrut, arms } => {
            free_vars(scrut, bound, out);
            for a in arms {
                let mut b = bound.clone();
                bind_pat(&a.pat, &mut b);
                if let Some(g) = &a.guard {
                    free_vars(g, &b, out);
                }
                free_vars(&a.body, &b, out);
            }
        }
        Expr::Block(stmts) => free_vars_stmts(stmts, bound, out),
    }
}

fn free_field_vars(fs: &[Field], bound: &HashSet<String>, out: &mut HashSet<String>) {
    for f in fs {
        match f {
            Field::Value { value, .. } | Field::Bare(value) => free_vars(value, bound, out),
            Field::Shorthand(n) => {
                if !bound.contains(n) {
                    out.insert(n.clone());
                }
            }
            Field::Type { .. } => {}
        }
    }
}

fn free_vars_stmts(stmts: &[Stmt], bound: &HashSet<String>, out: &mut HashSet<String>) {
    let mut b = bound.clone();
    for s in stmts {
        match s {
            Stmt::Bind(bd) => {
                free_vars(&bd.value, &b, out);
                if let Expr::Ident(n) = &bd.target {
                    b.insert(n.clone());
                }
            }
            Stmt::Expr(e) | Stmt::Alias { value: e, .. } => free_vars(e, &b, out),
            Stmt::Fn(f) => {
                let mut inner = b.clone();
                inner.extend(f.params.iter().map(|p| p.name.clone()));
                match &f.body {
                    Some(FnBody::Expr(e)) => free_vars(e, &inner, out),
                    Some(FnBody::Block(ss)) => free_vars_stmts(ss, &inner, out),
                    None => {}
                }
                b.insert(f.name.clone());
            }
            Stmt::Import(_) => {}
        }
    }
}

fn bind_pat(p: &Pattern, set: &mut HashSet<String>) {
    match p {
        Pattern::Bind(n) => {
            set.insert(n.clone());
        }
        Pattern::Ctor { args, .. } => args.iter().for_each(|a| bind_pat(a, set)),
        Pattern::List { elems, rest } => {
            elems.iter().for_each(|e| bind_pat(e, set));
            if let Some(r) = rest {
                bind_pat(r, set);
            }
        }
        Pattern::Record(fields) => {
            for (n, sub) in fields {
                match sub {
                    Some(p) => bind_pat(p, set),
                    None => {
                        set.insert(n.clone());
                    }
                }
            }
        }
        Pattern::Or(alts) => alts.iter().for_each(|a| bind_pat(a, set)),
        _ => {}
    }
}

fn rec_dep(t: &CTy) -> Option<String> {
    match t {
        CTy::Rec(n) => Some(n.clone()),
        CTy::Arr(e) => rec_dep(e),
        _ => None,
    }
}

/// Struct-shaped dependency: a record or a sum referenced by value (the caller filters plain sums, which need no ordering).
fn struct_dep(t: &CTy) -> Option<String> {
    match t {
        CTy::Rec(n) | CTy::Sum(n) => Some(n.clone()),
        CTy::Arr(e) => struct_dep(e),
        _ => None,
    }
}

/// A strictly by-value type reference (arrays are heap pointers, so they do NOT propagate a value cycle).
fn value_dep(t: &CTy) -> Option<String> {
    match t {
        CTy::Rec(n) | CTy::Sum(n) => Some(n.clone()),
        _ => None,
    }
}

/// The struct name for an anonymous record's shape.
fn anon_record_name(fields: &[(String, CTy)]) -> String {
    let mut s = String::from("MacaAnon");
    for (n, t) in fields {
        s.push('_');
        s.push_str(&cid(n));
        s.push('_');
        s.push_str(&ty_tag(t));
    }
    s
}

/// An identifier-safe tag for a type, for use inside a generated struct name.
fn ty_tag(t: &CTy) -> String {
    match t {
        CTy::Int => "int".into(),
        CTy::Float => "float".into(),
        CTy::F32 => "f32".into(),
        CTy::Bool => "bool".into(),
        CTy::Str => "str".into(),
        CTy::Rec(n) | CTy::Sum(n) => cid(n),
        CTy::Arr(e) => format!("{}arr", ty_tag(e)),
        _ => "any".into(),
    }
}

/// What a lambda's body says its parameter is.
fn lambda_param_ty(name: &str, body: &Expr) -> Option<CTy> {
    let mut found = None;
    scan_param_use(name, body, &mut found);
    found
}

fn scan_param_use(name: &str, e: &Expr, found: &mut Option<CTy>) {
    let is_p = |x: &Expr| matches!(x, Expr::Ident(n) if n == name);
    match e {
        Expr::Binary { op, lhs, rhs, .. }
            if matches!(op, BinOp::Eq | BinOp::Ne | BinOp::Concat) && (is_p(lhs) || is_p(rhs)) =>
        {
            let other = if is_p(lhs) { rhs } else { lhs };
            match &**other {
                Expr::Str(_) => *found = Some(CTy::Str),
                Expr::Float(_) => *found = Some(CTy::Float),
                _ => {}
            }
        }
        Expr::Call { callee, .. } => {
            const STR_ONLY: &[&str] = &[
                "split",
                "trim",
                "upper",
                "lower",
                "starts_with",
                "ends_with",
                "replace",
                "substr",
                "repeat",
                "pad_start",
                "pad_end",
                "pad_center",
                "chars",
                "at",
                "is_whitespace",
                "is_ascii_digit",
                "is_alpha",
            ];
            if let Expr::Field { base, name: m } = &**callee
                && is_p(base)
                && STR_ONLY.contains(&m.as_str())
            {
                *found = Some(CTy::Str);
            }
        }
        _ => {}
    }
    if found.is_none() {
        walk_children(e, &mut |c| scan_param_use(name, c, found));
    }
}

/// Which unannotated parameters hold a function value.
fn closure_params(items: &[Stmt]) -> HashSet<(String, usize)> {
    let fns: Vec<&FnDef> = items
        .iter()
        .filter_map(|it| match it {
            Stmt::Fn(f) => Some(f),
            _ => None,
        })
        .collect();
    let mut out: HashSet<(String, usize)> = HashSet::new();
    for f in &fns {
        let mut callees = HashSet::new();
        if let Some(body) = &f.body {
            callee_idents_body(body, &mut callees);
            hof_method_args_body(body, &mut callees);
        }
        for (i, p) in f.params.iter().enumerate() {
            if p.ty.is_none() && callees.contains(&p.name) {
                out.insert((f.name.clone(), i));
            }
        }
    }
    let declared: HashSet<&str> = fns.iter().map(|f| f.name.as_str()).collect();
    let mut given_lambda: HashSet<(String, usize)> = HashSet::new();
    for f in &fns {
        if let Some(body) = &f.body {
            lambda_args_body(body, &declared, &mut given_lambda);
        }
    }
    for (name, i) in given_lambda {
        if let Some(f) = fns.iter().find(|f| f.name == name)
            && f.params.get(i).is_some_and(|p| p.ty.is_none())
        {
            out.insert((name, i));
        }
    }
    loop {
        let mut grew = false;
        for f in &fns {
            let mut fwd: Vec<(String, usize, String)> = Vec::new();
            if let Some(body) = &f.body {
                forwarded_args_body(body, &mut fwd);
            }
            for (callee, idx, arg) in fwd {
                if !out.contains(&(callee, idx)) {
                    continue;
                }
                for (i, p) in f.params.iter().enumerate() {
                    if p.ty.is_none() && p.name == arg && out.insert((f.name.clone(), i)) {
                        grew = true;
                    }
                }
            }
        }
        if !grew {
            return out;
        }
    }
}

/// Visit every expression in a function body.
fn body_exprs(b: &FnBody, f: &mut impl FnMut(&Expr)) {
    match b {
        FnBody::Expr(e) => f(e),
        FnBody::Block(ss) => ss.iter().for_each(|s| match s {
            Stmt::Expr(e) | Stmt::Bind(Bind { value: e, .. }) => f(e),
            _ => {}
        }),
    }
}

/// `(callee, index)` for every argument position a lambda is written into.
fn lambda_args_body(b: &FnBody, declared: &HashSet<&str>, out: &mut HashSet<(String, usize)>) {
    match b {
        FnBody::Expr(e) => lambda_args(e, declared, out),
        FnBody::Block(ss) => ss.iter().for_each(|s| match s {
            Stmt::Expr(e) | Stmt::Bind(Bind { value: e, .. }) => lambda_args(e, declared, out),
            _ => {}
        }),
    }
}

fn lambda_args(e: &Expr, declared: &HashSet<&str>, out: &mut HashSet<(String, usize)>) {
    if let Expr::Call { callee, args } = e
        && let Expr::Ident(f) = &**callee
        && declared.contains(f.as_str())
    {
        for (i, a) in args.iter().enumerate() {
            if matches!(a, Arg::Pos(Expr::Lambda { .. })) {
                out.insert((f.clone(), i));
            }
        }
    }
    walk_children(e, &mut |c| lambda_args(c, declared, out));
}

/// Bare identifiers handed to a list method that takes a function, as in `xs.filter(pred)`.
fn hof_method_args_body(b: &FnBody, out: &mut HashSet<String>) {
    match b {
        FnBody::Expr(e) => hof_method_args(e, out),
        FnBody::Block(ss) => ss.iter().for_each(|s| match s {
            Stmt::Expr(e) | Stmt::Bind(Bind { value: e, .. }) => hof_method_args(e, out),
            _ => {}
        }),
    }
}

fn hof_method_args(e: &Expr, out: &mut HashSet<String>) {
    if let Expr::Call { callee, args } = e
        && let Expr::Field { name, .. } = &**callee
    {
        let at = match name.as_str() {
            "map" | "filter" | "parallel" => 0,
            "reduce" | "fold" => 1,
            _ => usize::MAX,
        };
        if let Some(Arg::Pos(Expr::Ident(x))) = args.get(at) {
            out.insert(x.clone());
        }
    }
    walk_children(e, &mut |c| hof_method_args(c, out));
}

/// Every `g(…, x, …)` in a body, as `(g, index, x)` for a bare-identifier argument.
fn forwarded_args_body(b: &FnBody, out: &mut Vec<(String, usize, String)>) {
    match b {
        FnBody::Expr(e) => forwarded_args(e, out),
        FnBody::Block(ss) => ss.iter().for_each(|s| match s {
            Stmt::Expr(e) | Stmt::Bind(Bind { value: e, .. }) => forwarded_args(e, out),
            _ => {}
        }),
    }
}

fn forwarded_args(e: &Expr, out: &mut Vec<(String, usize, String)>) {
    if let Expr::Call { callee, args } = e
        && let Expr::Ident(g) = &**callee
    {
        for (i, a) in args.iter().enumerate() {
            if let Arg::Pos(Expr::Ident(x)) = a {
                out.push((g.clone(), i, x.clone()));
            }
        }
    }
    walk_children(e, &mut |c| forwarded_args(c, out));
}

/// How deeply an array type nests: `int` is 0, `int[]` is 1, `int[][]` is 2.
fn arr_depth(t: &CTy) -> usize {
    match t {
        CTy::Arr(e) => 1 + arr_depth(e),
        CTy::Map(v) => 1 + arr_depth(v),
        _ => 0,
    }
}

/// Types whose value carries a heap buffer this back end allocated and can therefore name.
fn owns_heap(t: &CTy) -> bool {
    matches!(t, CTy::Arr(_) | CTy::Map(_))
}

/// Names that may outlive the statement list they are bound in.
fn escaping_names(stmts: &[Stmt]) -> HashSet<String> {
    let mut out = HashSet::new();
    let n = stmts.len();
    for (i, s) in stmts.iter().enumerate() {
        match s {
            Stmt::Expr(e) if i + 1 == n => note_escapes_all(e, &mut out),
            Stmt::Expr(e) => note_escapes(e, &mut out),
            Stmt::Bind(b) => {
                note_escapes_all(&b.value, &mut out);
                note_escapes(&b.target, &mut out);
            }
            Stmt::Fn(f) => match &f.body {
                Some(FnBody::Expr(e)) => note_escapes_all(e, &mut out),
                Some(FnBody::Block(ss)) => ss.iter().for_each(|s| match s {
                    Stmt::Expr(e) | Stmt::Bind(Bind { value: e, .. }) => {
                        note_escapes_all(e, &mut out)
                    }
                    _ => {}
                }),
                None => {}
            },
            _ => {}
        }
    }
    out
}

/// Every name in `e` escapes.
fn note_escapes_all(e: &Expr, out: &mut HashSet<String>) {
    if let Expr::Ident(n) = e {
        out.insert(n.clone());
    }
    walk_children(e, &mut |c| note_escapes_all(c, out));
}

/// Names in `e` that escape, sparing a method call's receiver.
fn note_escapes(e: &Expr, out: &mut HashSet<String>) {
    if let Expr::Call { callee, args } = e
        && let Expr::Field { base, .. } = &**callee
    {
        if !matches!(&**base, Expr::Ident(_)) {
            note_escapes(base, out);
        }
        for a in args {
            note_escapes_all(arg_expr(a), out);
        }
        return;
    }
    if let Expr::Ident(n) = e {
        out.insert(n.clone());
    }
    walk_children(e, &mut |c| note_escapes(c, out));
}

/// The expressions a block's statements carry, one level down.
fn stmt_children(ss: &[Stmt], f: &mut dyn FnMut(&Expr)) {
    for s in ss {
        match s {
            Stmt::Expr(e) | Stmt::Bind(Bind { value: e, .. }) => f(e),
            Stmt::Alias { value, .. } => f(value),
            Stmt::Fn(fd) => match &fd.body {
                Some(FnBody::Expr(e)) => f(e),
                Some(FnBody::Block(inner)) => stmt_children(inner, f),
                None => {}
            },
            Stmt::Import(_) => {}
        }
    }
}

/// Apply `f` to each direct sub-expression of `e`.
fn walk_children(e: &Expr, f: &mut dyn FnMut(&Expr)) {
    let stmts = stmt_children;
    match e {
        Expr::Str(parts) => parts.iter().for_each(|p| {
            if let StrPart::Interp(x) = p {
                f(x)
            }
        }),
        Expr::List(es) => es.iter().for_each(f),
        Expr::Record(fs) | Expr::Ctor { fields: fs, .. } | Expr::With { fields: fs, .. } => {
            fs.iter().for_each(|fl| {
                if let Field::Value { value, .. } | Field::Bare(value) = fl {
                    f(value)
                }
            })
        }
        Expr::Call { callee, args } => {
            f(callee);
            args.iter().for_each(|a| f(arg_expr(a)));
        }
        Expr::Field { base, .. }
        | Expr::Unary { expr: base, .. }
        | Expr::Try(base)
        | Expr::Fail(base)
        | Expr::Reify(base)
        | Expr::Await(base)
        | Expr::Spawn(base)
        | Expr::Lambda { body: base, .. } => f(base),
        Expr::Index { base: a, index: b }
        | Expr::Range { lo: a, hi: b }
        | Expr::Binary { lhs: a, rhs: b, .. }
        | Expr::Assign {
            target: a,
            value: b,
        } => {
            f(a);
            f(b);
        }
        Expr::Ternary { cond, then, els } => {
            f(cond);
            f(then);
            f(els);
        }
        Expr::If { cond, then, els } => {
            f(cond);
            stmts(then, f);
            if let Some(e) = els {
                stmts(e, f);
            }
        }
        Expr::Match { scrut, arms } => {
            f(scrut);
            for a in arms {
                f(&a.body);
                if let Some(g) = &a.guard {
                    f(g);
                }
            }
        }
        Expr::For { iter, body, .. } => {
            f(iter);
            stmts(body, f);
        }
        Expr::While { cond, body } => {
            f(cond);
            stmts(body, f);
        }
        Expr::Block(ss) => stmts(ss, f),
        Expr::Return(Some(v)) => f(v),
        _ => {}
    }
}

/// One piece of a string being built, and who is holding it.
#[derive(Clone)]
struct Piece {
    code: String,
    /// Nothing outside this expression is holding these bytes: a literal, or a block built right here.
    owned: bool,
}

/// Can a value of this type be an operand of `++` beside a string?
fn can_concat(t: &CTy) -> bool {
    matches!(
        t,
        CTy::Str | CTy::Int | CTy::Float | CTy::F32 | CTy::Bool | CTy::Unknown
    )
}

impl Piece {
    /// An operand of `++` where the other side is a string.
    fn operand(code: &str, t: &CTy, owned: bool) -> Piece {
        match t {
            CTy::Int | CTy::Float | CTy::F32 | CTy::Bool => Piece::rendered(code, t, owned),
            _ => Piece {
                code: code.to_string(),
                owned,
            },
        }
    }

    /// A piece whose value is written out as text: an interpolation, or an element's child.
    fn rendered(code: &str, t: &CTy, owned: bool) -> Piece {
        match t {
            CTy::Str => Piece {
                code: code.to_string(),
                owned,
            },
            _ => Piece {
                code: to_str(code, t),
                owned: true,
            },
        }
    }

    /// A literal, which lives in `.rodata` and is nobody's to release.
    fn literal(code: String) -> Piece {
        Piece { code, owned: true }
    }

    /// Is this a piece a release would actually reach?
    fn releasable(&self) -> bool {
        self.owned && !self.code.starts_with('"')
    }

    /// Is reading this piece an event?
    fn is_settled(&self) -> bool {
        self.code.starts_with('"')
            || self
                .code
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '_')
    }
}

/// HTML elements that have no closing tag and take no children.
fn is_void_html(tag: &str) -> bool {
    matches!(
        tag,
        "area"
            | "base"
            | "br"
            | "col"
            | "embed"
            | "hr"
            | "img"
            | "input"
            | "link"
            | "meta"
            | "source"
            | "track"
            | "wbr"
    )
}
