//! maca-backend-c: lower the AST to C.
//!
//! Type-directed lowering. Records become C structs, nullary sums become
//! enums, `T[]` becomes a monomorphized dynamic array (via the runtime's
//! `MACA_DEFINE_ARRAY` macro), and `std/json` encode/decode are generated
//! per record/sum type. Strings are heap `maca_str`; interpolation folds to
//! `maca_concat`. `match` lowers to an if-else chain, `?` is transparent, and
//! `fail msg` calls `maca_fail` — a clean stderr message + `exit(1)` (the
//! runtime file/JSON helpers still abort). `reify`/`try e` catches a
//! failure via setjmp/longjmp, yielding the caught message (or "").
//!
//! GNU statement expressions (`({ ...; v; })`, supported by `zig cc`/clang) are
//! used for inline list literals.

use maca_parser::ast::*;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

pub fn emit(m: &Module) -> String {
    let mut cx = Cx::new(m);
    cx.collect();
    cx.emit_all();
    cx.finish()
}

/// Emit C, or the list of codegen limitations the module hit. The driver uses
/// this so an unsupported construct surfaces as a clean error instead of
/// silently-wrong C reaching the compiler.
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

/// Header names from `import c "header.h"` (FFI). The driver provides bindings +
/// links the corresponding library.
pub fn c_imports(m: &Module) -> Vec<String> {
    m.items
        .iter()
        .filter_map(|it| match it {
            Stmt::Import(Import::Foreign { lang, spec }) if lang == "c" => Some(spec.clone()),
            _ => None,
        })
        .collect()
}

/// Backend-facing type. Independent of the checker's `Ty`; carries exactly what
/// codegen needs to pick C types and (de)serializers.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
enum CTy {
    Int,
    Float, // f64 / double
    F32,   // f32 / float (distinct from Float for the SIMD C ABI)
    Str,
    Bool,
    Unit,
    Rec(String),
    Sum(String),
    Arr(Box<CTy>),
    /// `Map str V` — a string-keyed hash map, monomorphized on its value type
    /// the same way an array is on its element type. Keys are always `str`, so
    /// only the value type varies.
    Map(Box<CTy>),
    /// SIMD vector, e.g. `f32x8` → { name: "f32x8", scalar_c: "float", lanes: 8 }
    Vec {
        name: String,
        scalar_c: String,
        lanes: usize,
    },
    /// A closure value, carrying its arity — the runtime has one struct per
    /// arity (`maca_closure`, `maca_closure2`), so a local holding a two-
    /// parameter lambda has to be declared as the matching one.
    Closure2(Box<CTy>),
    /// A concurrent computation handle (`spawn e`), awaited with `await`.
    /// Lowers to `maca_future*`; its result is boxed as `int64_t` for the slice.
    Future,
    /// A first-class function value (a lambda). Lowers to `maca_closure`.
    /// Its result type: the boundary word is an `int64_t`, so a call site needs
    /// this to unbox a record result back into a record.
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
    // declarations
    sums: BTreeMap<String, Vec<String>>, // sum name -> variants
    variant_of: HashMap<String, String>, // variant -> sum name
    variant_payloads: HashMap<String, Vec<CTy>>, // variant -> payload field types
    records: BTreeMap<String, Vec<(String, CTy)>>, // record name -> ordered fields
    rec_order: Vec<String>,              // topo order
    modules: HashSet<String>,            // imported module names (json, dirs, …)
    lets: Vec<(String, CTy, Expr)>,      // top-level `let`/value bindings
    let_names: HashSet<String>,
    fns: HashMap<String, (Vec<CTy>, CTy)>, // fn name -> (param types, ret)
    arr_elems: HashSet<CTy>,               // array element types to instantiate
    map_vals: HashSet<CTy>,                // map value types to instantiate
    /// `(fn name, param index)` pairs that hold a function value.
    closure_params: HashSet<(String, usize)>,
    vecs: BTreeSet<(String, String, usize)>, // SIMD vector types (name, scalar_c, lanes)
    tmp: usize,
    // lambdas hoisted to top-level `static` functions (closures carry a heap env)
    hoisted_decls: Vec<String>,
    hoisted_defs: Vec<String>,
    lambda_count: usize,
    /// top-level fns already wrapped as a closure value (`f` passed by name);
    /// the boxing thunk is emitted once per fn.
    fn_thunks: HashSet<String>,
    // generic functions, monomorphized per concrete instantiation
    generics: HashMap<String, FnDef>,
    spec_pending: Vec<(String, Vec<CTy>)>, // instantiations to emit
    spec_done: HashSet<(String, Vec<CTy>)>, // already emitted
    // codegen limitations hit while lowering — surfaced as clean errors instead
    // of silently emitting wrong C.
    problems: Vec<String>,
    // Tailwind utility names seen in a `class=` attribute, so `styles()` can
    // emit only the rules the program actually uses.
    classes: BTreeSet<String>,
}

impl<'a> Cx<'a> {
    fn new(m: &'a Module) -> Self {
        Cx {
            m,
            out: String::new(),
            sums: BTreeMap::new(),
            variant_of: HashMap::new(),
            variant_payloads: HashMap::new(),
            records: BTreeMap::new(),
            rec_order: Vec::new(),
            modules: HashSet::new(),
            lets: Vec::new(),
            let_names: HashSet::new(),
            fns: HashMap::new(),
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
            spec_done: HashSet::new(),
            problems: Vec::new(),
            classes: BTreeSet::new(),
        }
    }

    /// Record a codegen limitation. The placeholder C keeps the output
    /// well-formed for tests, but `emit_checked` turns any recorded problem into
    /// a hard error so a real build never ships silently-wrong code.
    /// The finished translation unit.
    ///
    /// `styles()` lowers to `MACA_STYLES`, which can only be defined once every
    /// `class=` in the module has been seen — so it is prepended here, after
    /// lowering rather than during it.
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
                // the sheet becomes a C string literal, so its backslashes —
                // the selector escapes — have to survive that trip
                css.push_str(&r.replace('\\', "\\\\"));
                css.push_str("\\n");
            }
        }
        let def = format!("#define MACA_STYLES \"{css}\"\n");
        // after the includes, before anything that uses it
        match self.out.find("\n\n") {
            Some(i) => {
                self.out.insert_str(i + 2, &def);
                self.out
            }
            None => def + &self.out,
        }
    }

    fn problem(&mut self, msg: impl Into<String>) {
        self.problems.push(msg.into());
    }

    /// Record the utility names in a `class=` value.
    ///
    /// The value is often a call rather than a literal — `md_class("pre")` —
    /// so the whole module is scanned separately (see `collect`); this catches
    /// the direct case. A class assembled from run-time data can't be in the
    /// sheet, which is the same limitation Tailwind itself has.
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

    fn fresh(&mut self) -> String {
        self.tmp += 1;
        format!("_t{}", self.tmp)
    }

    // ---- collection -------------------------------------------------------

    /// Emit a payload-bearing sum as `{ tag; union { … } as; }` plus a
    /// constructor `Sum_Variant(payload…)` per variant.
    fn emit_tagged_sum(&mut self, name: &str, vars: &[String]) {
        // recursive sums are named structs (forward-declared elsewhere) so a
        // self-referential payload can be stored as a `Name*` pointer.
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
            // plain string (not format!): one brace closes the union, one the struct
            self.push("} as; };");
        } else {
            self.push(&format!("}} as; }} {name};"));
        }
        for v in vars {
            let p = self.variant_payloads.get(v).cloned().unwrap_or_default();
            let params = if p.is_empty() {
                "void".to_string()
            } else {
                // constructors take payloads by value; boxing is internal
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

    /// A sum with at least one payload-carrying variant → a tagged struct/union
    /// (as opposed to a plain C enum).
    fn is_tagged(&self, sum: &str) -> bool {
        self.sums.get(sum).is_some_and(|vs| {
            vs.iter()
                .any(|v| self.variant_payloads.get(v).is_some_and(|p| !p.is_empty()))
        })
    }

    fn collect(&mut self) {
        // Tailwind candidates from every string literal in the module, not just
        // `class=` sites — a program that names its classes in a helper has the
        // literals in that helper's body.
        for item in &self.m.items {
            maca_backend_js::collect_class_strings(item, &mut self.classes);
        }
        // register record names up front so both sum payload types and (later)
        // record field types can reference a record declared anywhere in the file
        for item in &self.m.items {
            if let Stmt::Bind(b) = item
                && let Expr::Ident(name) = &b.target
                && is_record_value(&b.value)
            {
                self.records.entry(name.clone()).or_default();
            }
        }
        // Types can reference each other (and themselves) in any order, so
        // register every sum and record *name* before resolving any payload or
        // field type. Otherwise a self-referential payload (`Tree = Node(Tree,
        // Tree)`) or a forward reference resolves to `Unknown` → `int64_t`.
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
                        } else if is_record_value(&b.value) {
                            self.records.insert(name.clone(), Vec::new());
                        }
                    }
                }
                _ => {}
            }
        }
        // now every type name is known: resolve sum payloads and record fields.
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
                    self.records.insert(name.clone(), fields);
                }
            }
        }
        self.closure_params = closure_params(&self.m.items);
        // The primitive arrays are always defined. They are a handful of
        // `static inline` functions each, which the C compiler drops if unused,
        // and having them unconditionally removes a whole class of bug: a type
        // that only becomes reachable while a body is being lowered — the
        // element type of a `.map`'s result, `chars()` used on its own — was
        // registered after the typedefs had already been written out.
        for t in [CTy::Int, CTy::Str, CTy::Float, CTy::Bool] {
            self.arr_elems.insert(t);
        }
        // fn signatures, lets
        for item in &self.m.items {
            match item {
                Stmt::Fn(f) => {
                    // a generic function (type-variable params/ret) is not emitted
                    // as one C function — it is monomorphized per call site.
                    if fn_is_generic(f) {
                        self.generics.insert(f.name.clone(), f.clone());
                    } else {
                        // an unannotated parameter that holds a function value
                        // → `maca_closure`. Surface Maca has no function-type
                        // syntax, so this is how a higher-order parameter is
                        // recognized (see `closure_params`).
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
                                    self.cty_opt(&p.ty)
                                }
                            })
                            .collect::<Vec<_>>();
                        // No `-> T`? Infer it from an arrow body, which *is* the
                        // function's value — otherwise the return type stays
                        // `Unit` and callers don't convert the result (a
                        // `"{inc(41)}"` would pass an int where a string is
                        // wanted). A block body keeps `Unit`.
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
        // array element types from record fields
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
    }

    /// Instantiate array types for list literals that don't come from a record
    /// field (e.g. `let xs = 1, 2, 3`).
    fn collect_list_arrays(&mut self) {
        let items = self.m.items.clone();
        for item in &items {
            match item {
                Stmt::Fn(f) => match &f.body {
                    Some(FnBody::Block(s)) => self.walk_stmts(s),
                    Some(FnBody::Expr(e)) => self.walk_expr(e),
                    None => {}
                },
                Stmt::Bind(b) => self.walk_expr(&b.value),
                _ => {}
            }
        }
    }
    fn walk_stmts(&mut self, stmts: &[Stmt]) {
        for s in stmts {
            match s {
                Stmt::Bind(b) => {
                    // a local's declared type instantiates its container:
                    // `counts: Map str int = map()` needs `IntMap` defined
                    for t in &b.tys {
                        let cty = self.cty(t);
                        self.note_arr(&cty);
                    }
                    self.walk_expr(&b.value);
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
                // `f32x8.splat(k)` — register the vector type for its typedef
                if let Expr::Field { base, name } = callee.as_ref() {
                    if name == "splat"
                        && let Expr::Ident(v) = base.as_ref()
                        && let Some((sc, ln)) = parse_vec_c(v)
                    {
                        self.vecs.insert((v.clone(), sc, ln));
                    }
                    // `s.split(sep)` and `s.chars()` both yield a `str[]` —
                    // register `StrArr` so its typedef/ops are emitted before
                    // any function body uses it. Without this, `chars()` only
                    // compiled when it was passed straight to a parameter
                    // declared `str[]` (which registered the type as a side
                    // effect); used on its own it emitted a reference to an
                    // undeclared `StrArr`.
                    if name == "split" || name == "chars" {
                        self.note_arr(&CTy::Arr(Box::new(CTy::Str)));
                    }
                }
                // `list_dir(path)` likewise yields a `str[]`.
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
                // An anonymous record literal has no declared type, so its
                // struct is synthesized from the shape and registered here —
                // before any typedef is emitted, which is why it happens in the
                // prepass rather than when the expression is lowered.
                if let Expr::Record(fs) = e {
                    let shape = self.anon_shape(fs);
                    for (_, t) in &shape {
                        self.note_arr(t);
                    }
                    self.records.insert(anon_record_name(&shape), shape);
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
                // a range materialized in value position is an `int[]`; register
                // the array type so its typedef/ops are emitted.
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
    /// Covers the shapes that actually appear (`x + 1`, a comparison, a call, a
    /// ternary, a literal); anything else stays `Unknown`, the gradual escape
    /// hatch, rather than the wrong concrete type.
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
                    // arithmetic: take whichever side we can name
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
            // both branches should agree; the `then` side names it
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
            // a list literal's type is its first element's, one level up — so a
            // record field holding `[1, 2, 3]` is an `int[]` and not an opaque
            // scalar
            Expr::List(es) => es
                .first()
                .map(|x| CTy::Arr(Box::new(self.shallow_cty(x))))
                .unwrap_or(CTy::Unknown),
            Expr::Record(fs) => CTy::Rec(anon_record_name(&self.anon_shape(fs))),
            Expr::Ident(n) => self
                .variant_of
                .get(n)
                .map(|s| CTy::Sum(s.clone()))
                .unwrap_or(CTy::Unknown),
            Expr::Call { callee, .. } => match &**callee {
                // a constructor call like `Circle(2.0)` has the sum's type — so a
                // list of them (`Circle(x), Rect(w, h)`) registers `ShapeArr`.
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
    ///
    /// Sorted, so `{ x = 1, y = 2 }` and `{ y = 2, x = 1 }` are the same type —
    /// which is what "structural" has to mean for the two to be assignable to
    /// one another. Field types come from `shallow_cty`, the same function the
    /// lowering uses, so the struct and its compound literal cannot disagree.
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

    fn note_arr(&mut self, t: &CTy) {
        match t {
            CTy::Arr(e) => {
                self.arr_elems.insert((**e).clone());
                self.note_arr(e);
            }
            // a map's typedef needs its value type instantiated too, and
            // `.keys()` yields a `str[]`, so that array comes along with it
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

    fn topo_records(&mut self) {
        let mut seen = HashSet::new();
        let names: Vec<String> = self.records.keys().cloned().collect();
        for n in names {
            self.topo_visit(&n, &mut seen);
        }
    }

    /// Combined dependency order over both records and tagged sums: a node is
    /// emitted only after every record / tagged-sum it references by value.
    /// (Plain sums are enums with no struct dependency and are excluded.)
    fn struct_order(&self) -> Vec<String> {
        let mut seen = HashSet::new();
        let mut order = Vec::new();
        // deterministic node set (BTreeMap key order): records then tagged sums
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
    /// Names referenced strictly *by value* (arrays are heap pointers and so
    /// break value cycles). Used for recursion detection.
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
    /// A record is recursive when it can reach itself through struct-shaped
    /// references (which recurse through arrays). Since a record can never hold
    /// another by value in a cycle (infinite size), any such cycle passes
    /// through an array — e.g. `Expr { children: Expr[] }`. Such a record needs
    /// a forward declaration so its element array can be declared first.
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
    /// A payload slot is boxed (stored behind a pointer) when it is a sum that
    /// can reach the enclosing sum — the value cycle that would make the struct
    /// infinitely sized. Records-in-cycles are not boxed (a documented limit).
    fn is_boxed(&self, sum: &str, t: &CTy) -> bool {
        matches!(t, CTy::Sum(n) if self.reaches(n, sum))
    }

    /// Names of records / tagged sums referenced by value in a node's fields
    /// (record) or variant payloads (tagged sum).
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
                        // a boxed slot is a pointer — it needs only a forward
                        // declaration, not the full definition, so it is not an
                        // ordering dependency (this breaks the value cycle).
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

    // ---- type conversion --------------------------------------------------

    fn cty(&self, t: &Type) -> CTy {
        match t {
            Type::Name(segs) if segs.len() == 1 => {
                let n = segs[0].as_str();
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
                    _ => CTy::Unknown,
                }
            }
            Type::Array(t) => CTy::Arr(Box::new(self.cty(t))),
            Type::Opt(t) => self.cty(t),
            Type::Paren(t) => self.cty(t),
            // `Map str V` — the key type is written for readability and checked
            // by the type checker; codegen only needs the value type.
            Type::Apply(base, args) if matches!(&**base, Type::Name(s) if s.last().is_some_and(|n| n == "Map")) => {
                CTy::Map(Box::new(args.last().map_or(CTy::Unknown, |t| self.cty(t))))
            }
            _ => CTy::Unknown,
        }
    }
    fn cty_opt(&self, t: &Option<Type>) -> CTy {
        t.as_ref().map_or(CTy::Unknown, |t| self.cty(t))
    }
}

// ---- emission ------------------------------------------------------------

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

        // SIMD vector typedefs (clang ext_vector_type; ABI-compatible with the
        // LLVM <N x T> used by the LLVM backend for the same functions)
        let vecs: Vec<_> = self.vecs.iter().cloned().collect();
        for (name, scalar, lanes) in &vecs {
            self.push(&format!(
                "typedef {scalar} {name} __attribute__((ext_vector_type({lanes})));"
            ));
        }
        if !vecs.is_empty() {
            self.push("");
        }

        // Plain (nullary-only) sums are C enums with no struct dependencies, so
        // emit them up front. Tagged sums (payloads) and records are structs that
        // may reference each other by value, so they are emitted together in
        // dependency order below; their to_str/from_str follow.
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
        // recursive tagged sums are named structs, forward-declared so a
        // self-referential payload can hold a `Name*`.
        for (name, _) in &sums {
            if self.is_tagged(name) && self.sum_is_recursive(name) {
                self.push(&format!("typedef struct {name} {name};"));
            }
        }
        self.push("");

        // primitive-element arrays. Record- and sum-element arrays are structs
        // themselves, so they wait until their element type is defined (emitted
        // with / after the struct block below).
        let mut emitted_arr: HashSet<CTy> = HashSet::new();
        let mut elems: Vec<CTy> = self.arr_elems.iter().cloned().collect();
        // Shallowest first: `IntArrArr`'s element is `IntArr`, so `IntArr` has
        // to be a complete type before the outer array's struct names it. A
        // HashSet's order is arbitrary, which made `int[][]` compile or not
        // depending on the hash seed.
        elems.sort_by_key(|e| (arr_depth(e), arr_name(e)));
        for e in &elems {
            if !matches!(e, CTy::Rec(_) | CTy::Sum(_)) {
                self.push(&format!(
                    "MACA_DEFINE_ARRAY({}, {})",
                    arr_name(e),
                    c_type(e)
                ));
                emitted_arr.insert(e.clone());
            }
        }
        // string-keyed maps, monomorphized on the value type. Sorted so the
        // generated C is byte-identical run to run.
        let mut vals: Vec<CTy> = self.map_vals.iter().cloned().collect();
        vals.sort_by_key(map_name);
        for v in &vals {
            if !matches!(v, CTy::Rec(_) | CTy::Sum(_)) {
                self.push(&format!("MACA_DEFINE_MAP({}, {})", map_name(v), c_type(v)));
            }
        }
        self.push("");

        // Recursive records (`Expr { children: Expr[] }`) form a definition
        // cycle: the record body needs its element-array type complete, but the
        // array's ops need the record's `sizeof` — complete. We break it with a
        // C forward declaration: name the record struct, declare the element
        // array *struct* (a bare `Elem* data` needs only the forward decl), then
        // close the record body, then emit the array *ops* once the record is
        // sized. `cyclic_arr_elems` are the element types deferred this way.
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
            // element arrays over any cyclic record, gathered from every field
            // and every stray array use, declared (struct only) up front.
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
                emitted_arr.insert(e.clone());
            }
            cyclic_arr_elems = want;
            self.push("");
        }

        // Records and tagged sums are structs that may reference each other by
        // value (a sum payload holds a record, a record field holds a sum), so
        // they are emitted together in one dependency order. Record-element
        // arrays are interleaved just before the struct that first needs them
        // (the element type must be defined before MACA_DEFINE_ARRAY, which uses
        // `sizeof(Elem)`). Plain (nullary) sums were emitted as enums above and
        // carry no struct dependency.
        let struct_order = self.struct_order();
        for name in &struct_order {
            // emit any record/sum-element arrays this struct references first
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
                    && !emitted_arr.contains(e)
                {
                    self.push(&format!(
                        "MACA_DEFINE_ARRAY({}, {})",
                        arr_name(e),
                        c_type(e)
                    ));
                    emitted_arr.insert((**e).clone());
                }
            }
            if self.records.contains_key(name) {
                let fields = self.records[name].clone();
                // a recursive record was forward-declared, so close its named
                // struct; a plain record gets the usual anonymous typedef.
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
        // now every record is sized: emit the deferred element-array ops.
        for e in &cyclic_arr_elems {
            self.push(&format!("MACA_ARRAY_OPS({}, {})", arr_name(e), c_type(e)));
        }
        // record/sum-element arrays that aren't a struct field but arise
        // elsewhere (e.g. a top-level `let xs = R{}, R{}` or an index target):
        // now that every struct is defined, emit their MACA_DEFINE_ARRAY.
        for e in &elems {
            if matches!(e, CTy::Rec(_) | CTy::Sum(_)) && !emitted_arr.contains(e) {
                self.push(&format!(
                    "MACA_DEFINE_ARRAY({}, {})",
                    arr_name(e),
                    c_type(e)
                ));
                emitted_arr.insert(e.clone());
            }
        }
        self.push("");

        // sum to_str/from_str (json codegen needs them). Plain and tagged sums
        // both get them; for tagged sums the payload is dropped in from_str — a
        // documented limitation. Emitted after the struct block so tagged-sum
        // constructors are already defined.
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
        }
        self.push("");

        // json forward decls + defs (records only; sums use to_str/from_str)
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

        // top-level let accessors: declared here, defined below with the
        // function bodies. A constant holding a lambda hoists that lambda to a
        // `static` function, and the hoist has to land *before* the accessor
        // that takes its address — which only the deferred region guarantees.
        let lets = self.lets.clone();
        for (name, cty, init) in &lets {
            let ret = self.let_ty(cty, init);
            self.push(&format!("static {} mv_{name}(void);", c_type(&ret)));
        }
        self.push("");

        // user fn forward decls (SIMD kernels are defined on the LLVM path, so
        // they are declared `extern` here and skipped in the def loop)
        for item in &self.m.items {
            if let Stmt::Fn(f) = item {
                // generic fns are emitted per-instantiation, not once
                if f.name == "main" || self.generics.contains_key(&f.name) {
                    continue;
                }
                // bodyless fns are foreign (FFI) declarations; SIMD kernels live
                // on the LLVM path — both are `extern`.
                if f.body.is_none() || self.is_simd_fn(&f.name) {
                    self.push(&format!("extern {};", self.fn_sig(f)));
                } else {
                    self.push(&format!("{};", self.fn_sig(f)));
                }
            }
        }
        self.push("");

        // user fn defs (skip SIMD kernels — the LLVM backend defines them).
        // Emit into a scratch buffer first: lowering a body may hoist lambdas to
        // top-level `static` functions, which must be declared/defined *before*
        // the functions that take their address.
        let saved = std::mem::take(&mut self.out);
        for (name, cty, init) in &lets {
            let ret = self.let_ty(cty, init);
            let mut env: Env = Vec::new();
            let (code, _) = self.expr(&mut env, init, None);
            self.push(&format!(
                "static {} mv_{name}(void) {{ return {code}; }}",
                c_type(&ret)
            ));
        }
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
        // drain the monomorphization worklist: emitting a specialization may
        // request further ones (generic calling generic), so loop until empty.
        while let Some((name, ctys)) = self.spec_pending.pop() {
            if !self.spec_done.insert((name.clone(), ctys.clone())) {
                continue;
            }
            self.emit_specialization(&name, &ctys);
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

    /// The C type of a top-level constant: its declared type when it has one,
    /// otherwise read off the initializer's shape.
    fn let_ty(&self, cty: &CTy, init: &Expr) -> CTy {
        if *cty == CTy::Unknown {
            infer_cty_shallow(init)
        } else {
            cty.clone()
        }
    }

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

    fn emit_fn(&mut self, f: &FnDef) {
        let (params, ret) = self.fns[&f.name].clone();
        let mut env: Env = f
            .params
            .iter()
            .zip(&params)
            .map(|(p, t)| (p.name.clone(), t.clone()))
            .collect();

        if f.name == "main" {
            // `_maca_argc`/`_maca_argv` rather than the conventional names: a
            // Maca `main(argv: str[])` would otherwise be shadowed by C's own
            // parameter and fail to compile.
            self.push("int main(int _maca_argc, char** _maca_argv) {");
            self.push("    maca_init();");
            // build args: str[]
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

        self.push(&format!("{} {{", self.fn_sig(f)));
        match &f.body {
            Some(FnBody::Block(stmts)) => {
                let sink = if ret == CTy::Unit {
                    Sink::Discard
                } else {
                    Sink::Return(ret.clone())
                };
                self.block(&mut env, stmts, &sink, 1);
            }
            // an arrow body that is itself a control-flow expression (a `match`,
            // `if`, or block returning a value) routes through the Sink like a
            // block body would, instead of the value-only `expr()` path.
            Some(FnBody::Expr(e)) if is_control(e) => {
                let sink = if ret == CTy::Unit {
                    Sink::Discard
                } else {
                    Sink::Return(ret.clone())
                };
                self.stmt_expr(&mut env, e, &sink, 1);
            }
            Some(FnBody::Expr(e)) => {
                // An arrow body *is* the function's value, so it is returned
                // even with no `-> T` declared: an undeclared return type is
                // `CTy::Unit`, which still lowers to a C `int64_t`, so
                // discarding here would fall off the end of a non-void function
                // and hand back garbage (`twice(f, x) => f(f(x))` segfaulted).
                let (c, _) = self.expr(&mut env, e, Some(&ret));
                self.push(&format!("    return {c};"));
            }
            None => {}
        }
        // A block body with no declared return type discards its statements,
        // so terminate it rather than running off the end.
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

/// Where the value of a block / control-flow expression goes. This is what lets
/// `if`/`match`/block be used in value position (`let x = if c { … } else { … }`):
/// each branch tail is lowered against the same sink.
#[derive(Clone)]
enum Sink {
    Discard,             // statement position — value ignored
    Return(CTy),         // tail of a value-returning function
    Assign(String, CTy), // assign the result into an existing variable
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

    fn block(&mut self, env: &mut Env, stmts: &[Stmt], sink: &Sink, ind: usize) {
        let base = env.len();
        // Perceus, the codegen half: a local that owns a heap buffer and cannot
        // outlive this block is dropped when the block ends, which returns the
        // buffer to the free-list for the next allocation of that size to reuse.
        // `escaping` is the conservative half — see `note_escapes`.
        let escaping = escaping_names(stmts);
        let mut owned: Vec<(String, CTy)> = Vec::new();
        for (i, s) in stmts.iter().enumerate() {
            let last = i + 1 == stmts.len();
            // Drops go before the block's final statement when that statement
            // carries the value out (a `return`, or an assignment into an
            // enclosing binding); an owned local is never named there, so
            // nothing it holds is read after being dropped.
            if last && !matches!(sink, Sink::Discard) {
                self.emit_drops(&owned, ind);
                owned.clear();
            }
            match s {
                // a declaration: a bare `x = e` that first introduces `x` (its
                // const-ness is a checker concern). A bare assignment to a name
                // already in scope is a reassignment (handled below).
                Stmt::Bind(b) if matches!(&b.target, Expr::Ident(n) if lookup(env, n).is_none()) => {
                    if let Expr::Ident(name) = &b.target {
                        let ann = b.tys.first().map(|t| self.cty(t));
                        if is_control(&b.value) {
                            // `let x = if/match/block …` — declare, then assign
                            // the result in each branch tail via an Assign sink.
                            let ty = ann
                                .clone()
                                .unwrap_or_else(|| self.result_cty(env, &b.value));
                            self.indent(ind);
                            self.push(&format!("{} {};", c_type(&ty), cid(name)));
                            env.push((name.clone(), ty.clone()));
                            self.stmt_expr(env, &b.value, &Sink::Assign(cid(name), ty), ind);
                        } else {
                            let (code, cty) = self.expr(env, &b.value, ann.as_ref());
                            let ty = ann.unwrap_or(cty);
                            self.indent(ind);
                            self.push(&format!("{} {} = {code};", c_type(&ty), cid(name)));
                            // Owned iff the buffer is fresh (not another name's)
                            // and the name never leaves this block.
                            if owns_heap(&ty)
                                && !escaping.contains(name)
                                && !matches!(&b.value, Expr::Ident(_))
                            {
                                owned.push((name.clone(), ty.clone()));
                            }
                            env.push((name.clone(), ty));
                        }
                    }
                }
                // reassignment: a non-`let` bind to an in-scope lvalue — a bare
                // name (`i = i + 1`, drives counters/`while`), an element
                // (`xs[i] = v`), or a field (`p.x = v`).
                Stmt::Bind(b) => {
                    if let Expr::Ident(name) = &b.target {
                        if let Some(ty) = lookup(env, name) {
                            let (code, _) = self.expr(env, &b.value, Some(&ty));
                            self.indent(ind);
                            self.push(&format!("{} = {code};", cid(name)));
                        }
                    } else if let Some((lv, ty)) = self.lvalue(env, &b.target) {
                        let (code, _) = self.expr(env, &b.value, ty.as_ref());
                        self.indent(ind);
                        self.push(&format!("{lv} = {code};"));
                    }
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
                _ => {}
            }
        }
        self.emit_drops(&owned, ind);
        env.truncate(base);
    }

    /// Release each owned local's buffer. `maca_drop` takes the last owner's
    /// count to zero and hands the block back to the free-list; a NULL buffer
    /// (an array that was never pushed to) is a no-op.
    fn emit_drops(&mut self, owned: &[(String, CTy)], ind: usize) {
        for (name, ty) in owned {
            let v = cid(name);
            self.indent(ind);
            match ty {
                CTy::Map(_) => self.push(&format!(
                    "maca_drop({v}.keys); maca_drop({v}.vals); maca_drop({v}.used);"
                )),
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

    /// Infer the C type of a control-flow expression used in value position, by
    /// looking at a branch tail. Leaves are type-inferred by a dry lowering
    /// (emitted code is rolled back). Falls back to `int64_t` via `Unknown`.
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

    /// Emit a control-flow expression (for/match/if/block), routing its value to
    /// `sink` (Discard in statement position, Return/Assign in value position).
    fn stmt_expr(&mut self, env: &mut Env, e: &Expr, sink: &Sink, ind: usize) {
        match e {
            Expr::Block(stmts) => self.block(env, stmts, sink, ind),
            Expr::For { pat, iter, body } if matches!(&**iter, Expr::Range { .. }) => {
                // `for i in lo..hi` — a counting loop, no array materialized.
                // Ranges are inclusive of `hi`, so the guard is `<=`.
                let Expr::Range { lo, hi } = &**iter else {
                    unreachable!()
                };
                let (lc, _) = self.expr(env, lo, Some(&CTy::Int));
                let (hc, _) = self.expr(env, hi, Some(&CTy::Int));
                let var = if let Pattern::Bind(n) = pat {
                    n.clone()
                } else {
                    self.fresh()
                };
                let hv = self.fresh();
                self.indent(ind);
                self.push(&format!(
                    "{{ int64_t {hv} = {hc}; for (int64_t {var} = {lc}; {var} <= {hv}; {var}++) {{"
                ));
                let mut env2 = env.clone();
                env2.push((var, CTy::Int));
                self.block(&mut env2, body, &Sink::Discard, ind + 1);
                self.indent(ind);
                self.push("} }");
            }
            Expr::For { pat, iter, body } => {
                // a `for` loop has no value; its body is always in statement position
                let (ic, ity) = self.expr(env, iter, None);
                let elem = match ity {
                    CTy::Arr(e) => *e,
                    _ => CTy::Unknown,
                };
                let it = self.fresh();
                let idx = self.fresh();
                let var = if let Pattern::Bind(n) = pat {
                    n.clone()
                } else {
                    self.fresh()
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
        let sv = self.fresh();
        self.indent(ind);
        self.push(&format!("{{ {} {sv} = {sc};", c_type(&sty)));
        let elem = match &sty {
            CTy::Arr(e) => (**e).clone(),
            _ => CTy::Unknown,
        };
        // A guard can fail after its pattern matches, so the arm must be able to
        // fall through to the next one — an `if/else if` chain can't express
        // that. Only when some arm has a guard do we switch to independent `if`
        // blocks with a `goto` past the rest once an arm fully matches.
        if arms.iter().any(|a| a.guard.is_some()) {
            let end = format!("_m{}", self.fresh());
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
            // a catch-all (`cond == "1"`) is `else {` only after a prior arm —
            // as the first arm it must stay a real `if (1) {`
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
            // literal patterns → an equality test against the scrutinee
            Pattern::Int(n) => (format!("{sv} == {n}"), vec![]),
            Pattern::Float(f) => (format!("{sv} == {f}"), vec![]),
            Pattern::Bool(b) => (
                format!("{sv} == {}", if *b { "true" } else { "false" }),
                vec![],
            ),
            // a string literal against a string scrutinee (vs. the list-of-chars
            // case handled below for array scrutinees)
            Pattern::Str(lit) if matches!(sty, CTy::Str) => {
                (format!("maca_str_eq({sv}, {})", c_str(lit)), vec![])
            }
            // A variant may reach here as a bare `Bind` (bare `Red`) or a `Ctor`
            // (`Red()` / `Circle(x)`); against a sum scrutinee it is a tag test,
            // not a binding — mirror the checker's `is_variant` disambiguation.
            Pattern::Bind(n) | Pattern::Ctor { name: n, args: _ } if matches!(sty, CTy::Sum(s) if self.sums.get(s).is_some_and(|vs| vs.iter().any(|v| v == n))) =>
            {
                let CTy::Sum(s) = sty else { unreachable!() };
                if self.is_tagged(s) {
                    let cond = format!("{sv}.tag == {s}_tag_{n}");
                    // extract payload bindings from a `Circle(x, y)` pattern
                    let mut binds = Vec::new();
                    if let Pattern::Ctor { args, .. } = p {
                        let ptys = self.variant_payloads.get(n).cloned().unwrap_or_default();
                        for (i, a) in args.iter().enumerate() {
                            if let Pattern::Bind(bn) = a {
                                let bty = ptys.get(i).cloned().unwrap_or(CTy::Unknown);
                                // a boxed (recursive) payload is a pointer — deref
                                // to bind the value.
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
            // a record pattern `{ x, y }` is irrefutable (always matches) but
            // binds each field: `x` = `sv.x`. Shorthand (`x`) and `x: name` bind.
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
                        _ => None, // nested destructuring inside a field — deferred
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
            Pattern::Bind(n) => {
                // bind whole scrutinee
                (
                    "1".into(),
                    vec![(
                        n.clone(),
                        format!("{} {} = {sv};", c_type(sty), cid(n)),
                        sty.clone(),
                    )],
                )
            }
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
            // `A | B => …` — any alternative matches; bindings come from the
            // first alternative (mirrors the checker's bind_pattern).
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

    // ---- expressions ------------------------------------------------------

    fn expr(&mut self, env: &mut Env, e: &Expr, expected: Option<&CTy>) -> (String, CTy) {
        match e {
            Expr::Int(n) => (n.to_string(), CTy::Int),
            Expr::Float(f) => (format!("{f:?}"), CTy::Float),
            Expr::Bool(b) => ((if *b { "true" } else { "false" }).into(), CTy::Bool),
            Expr::Unit => ("0".into(), CTy::Unit),
            Expr::Str(parts) => (self.interp(env, parts), CTy::Str),
            Expr::Path(p) => (c_str(p), CTy::Str),
            Expr::Ident(n) => self.ident(env, n),
            Expr::Ctor { name, fields } => self.ctor(env, name, fields),
            // An anonymous record literal: the struct was synthesized from the
            // shape in the prepass, so this is an ordinary construction of it.
            Expr::Record(fields) => {
                let name = anon_record_name(&self.anon_shape(fields));
                self.ctor(env, &name, fields)
            }
            Expr::List(es) => self.list(env, es, expected),
            Expr::Call { callee, args } => self.call(env, callee, args, expected),
            Expr::Field { base, name } => self.field(env, base, name),
            Expr::Index { base, index } => self.index(env, base, index),
            Expr::Range { lo, hi } => {
                // materialize `lo..hi` (inclusive) as an `int[]` in value position
                // (e.g. `xs = 0..n`); `for i in lo..hi` uses a counting loop with
                // no array at all. The whole span is sized in one allocation and
                // filled in a tight loop — no per-element push/realloc.
                let (lc, _) = self.expr(env, lo, Some(&CTy::Int));
                let (hc, _) = self.expr(env, hi, Some(&CTy::Int));
                let an = arr_name(&CTy::Int);
                let lv = self.fresh();
                let hv = self.fresh();
                let n = self.fresh();
                let i = self.fresh();
                (
                    format!(
                        "({{ int64_t {lv} = {lc}, {hv} = {hc}; {an} _a = {an}_new(); \
                         int64_t {n} = {hv} >= {lv} ? {hv} - {lv} + 1 : 0; \
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
            // `fail msg` — a clean error exit with the message (was a bare abort()
            // that discarded the message and raised SIGABRT). `maca_fail` is
            // noreturn; the trailing `0` only keeps the expression well-typed.
            Expr::Fail(msg) => {
                let (mc, _) = self.expr(env, msg, Some(&CTy::Str));
                let ty = expected.cloned().unwrap_or(CTy::Unit);
                // `maca_fail` is noreturn, so the value after the comma is
                // never produced — but C still type-checks the expression, and
                // a bare `0` is the wrong type wherever the branch is a struct.
                let zero = match &ty {
                    CTy::Rec(_) | CTy::Sum(_) | CTy::Arr(_) | CTy::Map(_) => {
                        format!("({}){{0}}", c_type(&ty))
                    }
                    _ => "0".into(),
                };
                (format!("(maca_fail({mc}), {zero})"), ty)
            }
            // `try e` / `reify e` — run `e` under a failure handler; the value is
            // the caught message (a `str`), or "" on success. setjmp must be
            // inline (not wrapped in a helper), so a GNU statement-expression.
            Expr::Lambda { params, body, .. } => self.emit_lambda(env, params, body),
            // `base with { f = v, … }` — functional record update: copy the base
            // struct (a value type) and overwrite the named fields.
            Expr::With { base, fields } => self.with_update(env, base, fields),
            Expr::Reify(x) => {
                let jb = self.fresh();
                let r = self.fresh();
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
            // colorblind async. `spawn f(x)` schedules `f(x)` on the runtime's
            // worker pool and yields a `maca_future*`; `await fut` blocks the
            // caller until it resolves, returning the (int64-boxed) result. No
            // `async` keyword and no ABI change — an async fn is an ordinary fn.
            Expr::Spawn(inner) => match &**inner {
                Expr::Call { callee, args } if matches!(&**callee, Expr::Ident(_)) => {
                    let Expr::Ident(f) = &**callee else {
                        unreachable!()
                    };
                    let arg = args
                        .first()
                        .map(|a| self.arg(env, a))
                        .unwrap_or_else(|| "0".into());
                    (
                        format!("maca_spawn((maca_task_fn){}, (int64_t)({arg}))", cid(f)),
                        CTy::Future,
                    )
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
            return (cid(n), t);
        }
        if let Some(sum) = self.variant_of.get(n).cloned() {
            // a tagged sum's nullary variant is a constructor call `Sum_n()`;
            // a plain enum's variant is the enum constant `Sum_n`.
            if self.is_tagged(&sum) {
                return (format!("{sum}_{n}()"), CTy::Sum(sum));
            }
            return (format!("{sum}_{n}"), CTy::Sum(sum));
        }
        if self.let_names.contains(n) {
            let ty = self
                .lets
                .iter()
                .find(|(name, _, _)| name == n)
                .map(|(_, t, init)| {
                    if *t == CTy::Unknown {
                        infer_cty_shallow(init)
                    } else {
                        t.clone()
                    }
                })
                .unwrap_or(CTy::Unknown);
            return (format!("mv_{n}()"), ty);
        }
        // a top-level function referenced by name (not called) is a function
        // *value* → wrap it in a `maca_closure` so it can be passed to a
        // higher-order parameter (`run(cs, i, is_alpha)`).
        if self.fns.contains_key(n) {
            return self.fn_value_closure(n);
        }
        (cid(n), CTy::Unknown)
    }

    /// A `maca_closure` that calls top-level fn `name`, boxing its argument(s)
    /// and result across the uniform closure ABI. The boxing thunk is hoisted
    /// once; capturing nothing, the closure's env is `NULL`.
    fn fn_value_closure(&mut self, name: &str) -> (String, CTy) {
        let (params, ret) = self.fns[name].clone();
        let arity = params.len();
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

    /// Lower a lambda in a function-value position: a `maca_closure` with
    /// default `int` parameters (the shape `.parallel`/first-class calls use).
    fn emit_lambda(&mut self, env: &Env, params: &[Param], body: &Expr) -> (String, CTy) {
        // No element type from the call site (this is a bare lambda value, e.g.
        // one handed to a higher-order *parameter*), so read the body for what
        // it does to each parameter. Without this, `s => s == "c"` typed `s` as
        // an integer and compared a string pointer to it.
        let ptys: Vec<CTy> = params
            .iter()
            .map(|p| lambda_param_ty(&p.name, body).unwrap_or(CTy::Int))
            .collect();
        let (val, ret) = self.emit_closure(env, params, body, &ptys);
        (val, closure_ty(params.len(), ret))
    }

    /// Lower a lambda to a `maca_closure`: a hoisted function plus a heap
    /// environment holding the captured free variables. Supports capturing and
    /// non-capturing lambdas with any body shape. `param_tys` gives the (known)
    /// parameter types — higher-order methods pass the element type; other sites
    /// default to `int`. Returns `(closure-value, body-result-type)`; boundary
    /// values (params/result) are boxed as `int64_t` (a str/pointer fits).
    fn emit_closure(
        &mut self,
        env: &Env,
        params: &[Param],
        body: &Expr,
        param_tys: &[CTy],
    ) -> (String, CTy) {
        // free variables captured from the enclosing scope
        let mut bound: HashSet<String> = params.iter().map(|p| p.name.clone()).collect();
        let mut refs = HashSet::new();
        free_vars(body, &bound, &mut refs);
        bound.clear();
        let mut caps: Vec<(String, CTy)> = refs
            .into_iter()
            .filter(|n| !self.is_known_global(n) && !self.variant_of.contains_key(n))
            .map(|n| {
                let t = self.cap_ty(env, &n);
                (n, t)
            })
            .collect();
        caps.sort_by(|a, b| a.0.cmp(&b.0)); // deterministic layout

        let id = self.lambda_count;
        self.lambda_count += 1;
        let fname = format!("_lam{id}");
        let ename = format!("_lam{id}_env");
        let two = params.len() >= 2;

        if !caps.is_empty() {
            let fields = caps
                .iter()
                .map(|(n, t)| format!("{} {};", c_type(t), cid(n)))
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

        let saved = std::mem::take(&mut self.out);
        self.push(&format!("{sig} {{"));
        let mut lenv: Env = Vec::new();
        if caps.is_empty() {
            self.push("    (void)_envp;");
        } else {
            self.push(&format!("    {ename}* _e = ({ename}*)_envp;"));
            for (n, t) in &caps {
                self.push(&format!("    {} {} = _e->{};", c_type(t), cid(n), cid(n)));
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
        // A block / `if` / `match` body is a statement shape, so it goes
        // through the same `Sink` the rest of the backend uses: assign the
        // value in each tail, then box that once. Evaluating it as an
        // expression is what made `(n) => { a = n * 2  a > 10 }` unsupported.
        let ret = if is_control(body) {
            let ret = self.result_cty(&lenv, body);
            self.push(&format!("    {} _r;", c_type(&ret)));
            self.stmt_expr(&mut lenv, body, &Sink::Assign("_r".into(), ret.clone()), 1);
            self.push(&format!("    return {};", box_i64("_r", &ret)));
            ret
        } else {
            let (bc, bt) = self.expr(&mut lenv, body, None);
            let ret = if bt == CTy::Unknown { CTy::Int } else { bt };
            self.push(&format!("    return {};", box_i64(&bc, &ret)));
            ret
        };
        self.push("}");
        let def = std::mem::replace(&mut self.out, saved);
        self.hoisted_defs.push(def);

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
                let (outer, _) = self.ident(env, n);
                fills.push_str(&format!("_e->{} = {}; ", cid(n), outer));
            }
            format!(
                "({{ {ename}* _e = ({ename}*)maca_alloc(sizeof({ename})); {fills}({ctype}){{ {cast}{fname}, _e }}; }})"
            )
        };
        (val, ret)
    }

    /// Emit one monomorphized copy of a generic function for a concrete tuple of
    /// argument types (mangled name, concrete param/ret types). The body flows
    /// concrete types via the env, so type-variable references resolve correctly.
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
        self.push(&format!("static {sig} {{"));
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
                // An arrow body *is* the function's value, so it is returned
                // even with no `-> T` declared: an undeclared return type is
                // `CTy::Unit`, which still lowers to a C `int64_t`, so
                // discarding here would fall off the end of a non-void function
                // and hand back garbage (`twice(f, x) => f(f(x))` segfaulted).
                let (c, _) = self.expr(&mut env, e, Some(&ret));
                self.push(&format!("    return {c};"));
            }
            None => {}
        }
        // A block body with no declared return type discards its statements,
        // so terminate it rather than running off the end.
        if ret == CTy::Unit && matches!(&genf.body, Some(FnBody::Block(_))) {
            self.push("    return 0;");
        }
        self.push("}");
        let def = std::mem::replace(&mut self.out, saved);
        self.hoisted_defs.push(def);
    }

    /// type-variable name → concrete `CTy`, from a generic fn's params vs. the
    /// concrete argument types at a call.
    fn build_subst(&self, genf: &FnDef, arg_ctys: &[CTy]) -> HashMap<String, CTy> {
        let mut m = HashMap::new();
        for (p, cty) in genf.params.iter().zip(arg_ctys) {
            if let Some(Type::Name(segs)) = &p.ty
                && segs.len() == 1
                && is_tyvar(&segs[0])
            {
                m.insert(segs[0].clone(), cty.clone());
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
            _ => self.cty(t),
        }
    }

    fn spec_ret(&self, genf: &FnDef, arg_ctys: &[CTy]) -> CTy {
        let subst = self.build_subst(genf, arg_ctys);
        genf.ret
            .as_ref()
            .map_or(CTy::Unit, |t| self.subst_cty(t, &subst))
    }

    /// The C type of a captured free variable (a function-local of the enclosing
    /// scope). Top-level lets are globals (excluded upstream), so this only sees
    /// locals, which live in `env`.
    fn cap_ty(&self, env: &Env, n: &str) -> CTy {
        lookup(env, n).unwrap_or(CTy::Unknown)
    }

    /// Higher-order list methods whose lambda argument must be lowered with the
    /// element type as its parameter type: `.map`, `.filter`, `.reduce`. Returns
    /// `None` (fall through to the generic UFCS path) if the shape doesn't match.
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
        // A named top-level function passed where a lambda is expected
        // (`xs.filter(is_even)`) — reuse the function-value closure so
        // `.map`/`.filter`/`.reduce` accept it as readily as a lambda.
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
                // A closure the caller already holds — a higher-order parameter
                // forwarded to `.filter` rather than a lambda written here.
                // `filter` can take one because its result type is the receiver's
                // element type; `map`'s would depend on the closure's return,
                // which an opaque value doesn't carry.
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
        self.fns.contains_key(n)
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
                    | "true"
                    | "false"
                    | "read_file"
                    | "write_file"
                    | "file_exists"
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
                Some((v, _)) => self.expr(env, &v, Some(fty)).0,
                None => zero_value(fty),
            };
            parts.push(format!(".{} = {code}", cid(fname)));
        }
        (
            format!("(({name}){{ {} }})", parts.join(", ")),
            CTy::Rec(name.into()),
        )
    }

    /// `base with { f = v }` → `({ T _t = base; _t.f = v; _t; })`. The base's
    /// record type supplies each overwritten field's C type (for coercion).
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
        let t = self.fresh();
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
            let code = self.expr(env, &val, fty.as_ref()).0;
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

    /// `tag(attr=value, …, child, …)` → the HTML text for that element.
    ///
    /// Named arguments become attributes, positional ones children. A void
    /// element takes no children and closes itself. Children are already
    /// strings — a nested element lowered by this same function, or any
    /// expression converted with `to_str` — so the whole tree is one
    /// concatenation with no intermediate representation.
    ///
    /// Attribute values are escaped; children are *not*, because a child is
    /// either another element (already markup) or text the program chose to
    /// put there. A generator that has to emit `<pre>` around code it escaped
    /// itself cannot have the renderer escape it again.
    fn html_element(&mut self, env: &mut Env, tag: &str, args: &[Arg]) -> (String, CTy) {
        let (attrs, kids) = self.html_args(env, args);
        // attributes are emitted by one runtime call per attribute, folded into
        // the open tag
        let open = attrs.into_iter().fold(format!("\"<{tag}\""), |acc, a| {
            format!("maca_concat({acc}, {a})")
        });
        if is_void_html(tag) {
            if !kids.is_empty() {
                self.problem(format!("`{tag}` is a void element and takes no children"));
            }
            return (format!("maca_concat({open}, \">\")"), CTy::Str);
        }
        let inner = fold_concat(kids);
        (
            format!("maca_concat(maca_concat({open}, \">\"), maca_concat({inner}, \"</{tag}>\"))"),
            CTy::Str,
        )
    }

    /// `element(tag, attr=value, …, child, …)` — the same element, with the tag
    /// itself an expression.
    ///
    /// A document generator picks tags from its input: a heading's depth chooses
    /// `h1`…`h6`, a table row chooses `th` or `td`. The static form cannot say
    /// that, and without this every such site falls back to string
    /// concatenation. Voidness is settled in the runtime, since the tag isn't
    /// known until then.
    fn dynamic_element(&mut self, env: &mut Env, args: &[Arg]) -> (String, CTy) {
        let Some((Arg::Pos(tag_expr), rest)) = args.split_first() else {
            self.problem("`element` needs a tag name as its first argument".to_string());
            return ("\"\"".to_string(), CTy::Str);
        };
        let (tag, tt) = self.expr(env, tag_expr, Some(&CTy::Str));
        let tag = to_str(&tag, &tt);
        let (attrs, kids) = self.html_args(env, rest);
        let attrs = fold_concat(attrs);
        let kids = fold_concat(kids);
        (format!("maca_element({tag}, {attrs}, {kids})"), CTy::Str)
    }

    /// Lower an element call's arguments into (attribute exprs, child exprs).
    fn html_args(&mut self, env: &mut Env, args: &[Arg]) -> (Vec<String>, Vec<String>) {
        let mut attrs: Vec<String> = Vec::new();
        let mut kids: Vec<String> = Vec::new();
        for a in args {
            match a {
                Arg::Named { name, value } => {
                    let (v, t) = self.expr(env, value, Some(&CTy::Str));
                    // `class` is where Tailwind utilities are written, so its
                    // literals are collected for the generated stylesheet.
                    if name == "class" {
                        self.note_classes(value);
                    }
                    // `data-tomo` is one identifier, not a subtraction: an
                    // attached `-` is part of a name, a spaced one is the
                    // operator. So a hyphenated attribute needs no rewriting.
                    let key = name.replace('"', "");
                    // A bool decides whether the attribute *exists*. HTML reads
                    // every value as true — `hidden="false"` still hides — so
                    // `open=false` has to emit nothing at all.
                    if t == CTy::Bool {
                        attrs.push(format!("maca_flag(\"{key}\", {v})"));
                    } else {
                        attrs.push(format!("maca_attr(\"{key}\", {})", to_str(&v, &t)));
                    }
                }
                // `on:click=…` is a DOM handler; there is no DOM in a string.
                Arg::Directive { prop, .. } => {
                    self.problem(format!(
                        "`on:{prop}` needs a live DOM — build this with `--target js`"
                    ));
                }
                Arg::Pos(e) => {
                    let (c, t) = self.expr(env, e, Some(&CTy::Str));
                    kids.push(to_str(&c, &t));
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
        // A UI element tag on the native target renders to an HTML *string*.
        //
        // The JS backend turns these into a reactive DOM; there is no DOM here,
        // so `div(class="x", "hi")` becomes the text `<div class="x">hi</div>`.
        // That is what a static site generator needs, and it means a program
        // that emits HTML can be written in Maca's own UI syntax instead of
        // concatenating angle brackets by hand.
        // A user's own definition always wins. `label`, `main`, `section`,
        // `code`, `p`, `a`, `form`, `option` are all HTML tags *and* names
        // people give their functions; hijacking a defined name would break
        // working programs (it broke `examples/record_pattern.maca`, which has
        // a `label(pos: bool) -> str`).
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
        // module.member(...) and UFCS receiver.method(...)
        if let Expr::Field { base, name } = callee {
            if let Expr::Ident(m) = &**base {
                // `f32x8.splat(k)` → a broadcast compound literal
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
                if self.modules.contains(m) {
                    return self.module_call(env, m, name, args, expected);
                }
            }
            // UFCS: receiver.method(args)
            let (rc, rty) = self.expr(env, base, None);
            // higher-order list methods lower their lambda with the element type
            if let CTy::Arr(elem) = &rty
                && let Some(res) = self.list_hof(env, &rc, elem, name, args)
            {
                return res;
            }
            let a: Vec<String> = args.iter().map(|x| self.arg(env, x)).collect();
            return self.ufcs(&rc, &rty, name, &a);
        }
        if let Expr::Ident(name) = callee {
            // calling a local that holds a closure value: `f = v => …; f(x)`
            if let Some(CTy::Closure(r) | CTy::Closure2(r)) = lookup(env, name) {
                let boxed: Vec<(String, CTy)> =
                    args.iter().map(|x| self.arg_typed(env, x)).collect();
                let bx: Vec<String> = boxed.iter().map(|(c, t)| box_i64(c, t)).collect();
                let ret = (*r).clone();
                let call = if bx.len() >= 2 {
                    format!("maca_call2({}, {}, {})", cid(name), bx[0], bx[1])
                } else {
                    let a0 = bx.first().cloned().unwrap_or_else(|| "0".into());
                    format!("maca_call1({}, {a0})", cid(name))
                };
                return (unbox_i64(&call, &ret), ret);
            }
            // coercions need the argument type
            if name == "str" {
                let (c, t) = self.arg_typed(env, &args[0]);
                return (to_str(&c, &t), CTy::Str);
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
            // `sleep_ms(ms)` — an async suspension point (the runtime yields the
            // task for `ms`). Comma-expr with 0 keeps it a well-typed unit value.
            if name == "sleep_ms" && args.len() == 1 {
                let a = self.arg(env, &args[0]);
                return (format!("(maca_sleep_ms({a}), 0)"), CTy::Unit);
            }
            // ---- math prelude (always available; __builtin_* needs no -lm) ----
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
            // `len(x)` — array length (the backing `.len`) or string byte length
            if name == "len" && args.len() == 1 {
                let (c, t) = self.arg_typed(env, &args[0]);
                return match t {
                    CTy::Str => (format!("((int64_t)strlen({c}))"), CTy::Int),
                    _ => (format!("({c}).len"), CTy::Int),
                };
            }
            // a generic function → monomorphize per concrete argument types
            if let Some(genf) = self.generics.get(name).cloned() {
                let lowered: Vec<(String, CTy)> =
                    args.iter().map(|x| self.arg_typed(env, x)).collect();
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
            // a payload sum constructor: `Circle(5)` → `Shape_Circle(5)`
            if let Some(sum) = self.variant_of.get(name).cloned()
                && self.is_tagged(&sum)
            {
                let a: Vec<String> = args.iter().map(|x| self.arg(env, x)).collect();
                return (format!("{sum}_{name}({})", a.join(", ")), CTy::Sum(sum));
            }
            // a known user function: lower each argument with the matching
            // parameter type as its expected type (so `[]` and other
            // context-typed literals resolve correctly).
            if let Some((params, ret)) = self.fns.get(name).cloned() {
                let a: Vec<String> = args
                    .iter()
                    .enumerate()
                    .map(|(i, x)| self.arg_expected(env, x, params.get(i)).0)
                    .collect();
                return (format!("{}({})", cid(name), a.join(", ")), ret);
            }
            let a: Vec<String> = args.iter().map(|x| self.arg(env, x)).collect();
            if let Some(cfn) = console_fn(name) {
                return (format!("{cfn}({})", a.join(", ")), CTy::Unit);
            }
            // `styles()` — the stylesheet for the Tailwind utilities this
            // program uses. Generated at compile time from every `class=`
            // literal in the module, so nothing unused ships and no hand-written
            // CSS is needed. The string is emitted at the end of codegen, once
            // every class has been seen.
            if name == "styles" && a.is_empty() {
                return ("MACA_STYLES".into(), CTy::Str);
            }
            // `map()` — an empty map. Its value type comes from the context it
            // is being assigned into (`counts: Map str int = map()`), the same
            // way an empty list literal gets its element type.
            if name == "map" && a.is_empty() {
                let v = match expected {
                    Some(CTy::Map(v)) => (**v).clone(),
                    _ => CTy::Int,
                };
                self.note_arr(&CTy::Map(Box::new(v.clone())));
                return (format!("{}_new()", map_name(&v)), CTy::Map(Box::new(v)));
            }
            // file I/O builtins.
            match name.as_str() {
                "read_file" => return (format!("maca_read_file({})", a.join(", ")), CTy::Str),
                "write_file" => {
                    return (format!("maca_write_file({})", a.join(", ")), CTy::Bool);
                }
                "file_exists" => {
                    return (format!("maca_file_exists({})", a.join(", ")), CTy::Bool);
                }
                "make_dir" => return (format!("maca_make_dir({})", a.join(", ")), CTy::Bool),
                "is_dir" => return (format!("maca_is_dir({})", a.join(", ")), CTy::Bool),
                "file_size" => return (format!("maca_file_size({})", a.join(", ")), CTy::Int),
                "modified_ms" => return (format!("maca_modified_ms({})", a.join(", ")), CTy::Int),
                "remove_file" => return (format!("maca_remove_file({})", a.join(", ")), CTy::Bool),
                "remove_dir" => return (format!("maca_remove_dir({})", a.join(", ")), CTy::Bool),
                "copy_bytes" => {
                    return (format!("maca_copy_bytes({})", a.join(", ")), CTy::Bool);
                }
                // processes. `args` is a `str[]`, so the buffer and its length
                // are unpacked at the call site the way `.split` results are.
                "exec" | "capture" => {
                    let (cmd, args) = (&a[0], &a[1]);
                    let fn_name = format!("maca_{name}");
                    let ret = if name == "exec" { CTy::Int } else { CTy::Str };
                    return (
                        format!("{fn_name}({cmd}, {args}.data, {args}.len)"),
                        ret,
                    );
                }
                "env" => return (format!("maca_env({})", a.join(", ")), CTy::Str),
                "cwd" => return ("maca_cwd()".into(), CTy::Str),
                "chdir" => return (format!("maca_chdir({})", a.join(", ")), CTy::Bool),
                // stdin
                "read_line" => return ("maca_read_line()".into(), CTy::Str),
                "at_eof" => return ("maca_at_eof()".into(), CTy::Bool),
                "read_stdin" => return ("maca_read_stdin()".into(), CTy::Str),
                // time, UTC throughout
                "now_ms" => return ("maca_now_ms()".into(), CTy::Int),
                "now_iso" => return ("maca_now_iso()".into(), CTy::Str),
                "format_time" => {
                    return (format!("maca_format_time({})", a.join(", ")), CTy::Str);
                }
                // assertions: report and carry on, so one run finds every
                // failure rather than only the first
                "assert" => return (format!("maca_assert({})", a.join(", ")), CTy::Bool),
                "assert_eq" => return (format!("maca_assert_eq({})", a.join(", ")), CTy::Bool),
                "failures" => return ("maca_failures()".into(), CTy::Int),
                // allocator counters — how a program can see reuse happening
                "alloc_count" => return ("(int64_t)maca_alloc_count()".into(), CTy::Int),
                "reuse_count" => return ("(int64_t)maca_reuse_count()".into(), CTy::Int),
                // `list_dir(p)` → a `str[]` of entry names, built from the
                // runtime's malloc'd array (mirrors how `.split` lowers).
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
            // an FFI/foreign function (declared, provided by C glue) — a direct call
            return (format!("{}({})", cid(name), a.join(", ")), CTy::Unknown);
        }
        self.problem("call target is not a function name (higher-order call value unsupported)");
        ("0 /* unsupported call */".into(), CTy::Unknown)
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
                match t {
                    CTy::Rec(r) => (format!("{r}_to_json({c})"), CTy::Str),
                    CTy::Sum(s) => (format!("{s}_to_str({c})"), CTy::Str),
                    _ => (c, CTy::Str),
                }
            }
            ("json", "decode") => {
                let (c, _) = self.arg_typed(env, &args[0]);
                match expected {
                    Some(CTy::Rec(r)) => (
                        format!("{r}_from_json(maca_json_parse({c}))"),
                        CTy::Rec(r.clone()),
                    ),
                    _ => (format!("maca_json_parse({c})"), CTy::Unknown),
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
            // ---- map methods -------------------------------------------------
            //
            // `set` and `remove` return the map rather than mutating in place,
            // so a map behaves like every other value in the language: `m =
            // m.set(k, v)`. The copy is shallow — the same buffers with an
            // updated count — which is why the receiver is a statement
            // expression rather than a call on a temporary.
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
            // `get(k, default)` — a miss returns the default rather than a
            // sentinel, because the language has no null to return.
            (CTy::Map(v), "get") => {
                let mn = map_name(v);
                let dflt = a.get(1).cloned().unwrap_or_else(|| zero_value(v));
                (format!("{mn}_get({rc}, {}, {dflt})", arg0()), (**v).clone())
            }
            (CTy::Map(v), "has") => (format!("{}_has({rc}, {})", map_name(v), arg0()), CTy::Bool),
            (CTy::Map(v), "length") => (format!("{}_len({rc})", map_name(v)), CTy::Int),
            // sorted, so walking a map twice writes the same file twice
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
            // ---- list methods (closure-free; map/filter/reduce are in list_hof) ----
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
            (CTy::Arr(e), "first") => (format!("({rc}).data[0]"), (**e).clone()),
            (CTy::Arr(e), "last") => {
                let an = arr_name(e);
                (
                    format!("({{ {an} _s = {rc}; _s.data[_s.len - 1]; }})"),
                    (**e).clone(),
                )
            }
            // `.length()` and `.get(i)` — a method spelling of `len(xs)` / `xs[i]`,
            // used by the self-hosted lexer's index-walk over a char array.
            (CTy::Arr(_), "length") => (format!("({rc}).len"), CTy::Int),
            (CTy::Arr(e), "get") => {
                let an = arr_name(e);
                (
                    format!("({{ {an} _s = {rc}; _s.data[{}]; }})", arg0()),
                    (**e).clone(),
                )
            }
            // `.slice(from, to)` — a fresh sub-array over the half-open range.
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
            // string stdlib — UFCS methods on `str` (gradual `Unknown` receivers
            // too, since foreign/inferred strings often land as Unknown).
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
            // `slice` takes an exclusive end, the same as the list method; the
            // two names mean two things and each keeps its own convention
            (CTy::Str, "slice") => (
                format!("maca_str_slice({rc}, {}, {})", arg0(), arg1()),
                CTy::Str,
            ),
            (CTy::Str | CTy::Unknown, "repeat") => {
                (format!("maca_repeat({rc}, {})", arg0()), CTy::Str)
            }
            // `pad_start`/`pad_end` take an optional pad string (default " ").
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
            // `x.fixed(n)` — `x` with n decimal places. Written on a float, but
            // an int receiver is accepted and widened, so `{n:.2}` works on any
            // number rather than failing on the one case the user didn't expect.
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
            // byte-length + per-byte access + character classes: the primitives
            // the self-hosted lexer scans source with (`std/str`).
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
            // a user function called UFCS-style: `x.f(y)` → `f(x, y)`. Resolve
            // its real return type and escape a C-keyword name.
            _ => {
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

    /// A writable location for `target = value`: a nested `xs[i]` / `p.field`
    /// chain resolved to its C lvalue plus (optionally) the element type. Bare
    /// identifiers are handled directly by the caller. Returns `None` for
    /// non-assignable targets, which are then ignored (as before).
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
            _ => None,
        }
    }

    /// `base[index]` — element access. Arrays index the backing buffer
    /// (`arr.data[i]`); strings yield a one-character `str` via `maca_str_at`.
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
        let (rc, rt) = self.expr(env, rhs, None);

        // Operator overloading: when the left operand is a user type (record /
        // sum) and a function with the operator's canonical name exists, the
        // operator desugars to a call — `a + b` → `add(a, b)`. Primitives keep
        // the native operator.
        if matches!(lt, CTy::Rec(_) | CTy::Sum(_))
            && let Some(name) = overload_name(op)
            && let Some((_, ret)) = self.fns.get(name).cloned()
        {
            return (format!("{}({lc}, {rc})", cid(name)), ret);
        }

        use BinOp::*;
        match op {
            Div if matches!(lt, CTy::Str) => (format!("maca_path_join({lc}, {rc})"), CTy::Str),
            // `++` is string concat on strings, array concat on arrays.
            //
            // A non-string operand is converted, not passed through: `"h" ++ 3`
            // used to hand an `int64_t` to a `maca_str` parameter, compile
            // without complaint, and segfault at run time reading address 3.
            Concat if matches!(lt, CTy::Str) || matches!(rt, CTy::Str) => (
                format!(
                    "maca_concat({}, {})",
                    concat_operand(&lc, &lt),
                    concat_operand(&rc, &rt)
                ),
                CTy::Str,
            ),
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
    /// Lower a call argument with an optional expected type — lets an empty
    /// list literal `[]` take its element type from the callee's parameter
    /// (e.g. `scan(cs, 0, [])` where the 3rd param is `Token[]`).
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
        let mut acc: Option<String> = None;
        for p in parts {
            let piece = match p {
                StrPart::Text(t) => c_str(t),
                StrPart::Interp(e) => {
                    let (c, t) = self.expr(env, e, None);
                    to_str(&c, &t)
                }
            };
            acc = Some(match acc {
                None => piece,
                Some(a) => format!("maca_concat({a}, {piece})"),
            });
        }
        acc.unwrap_or_else(|| "\"\"".into())
    }

    // ---- json codegen -----------------------------------------------------

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
                "    maca_sb_put_json_str(&sb, {s}_to_str({access}));"
            )),
            CTy::Rec(r) => self.push(&format!("    maca_sb_puts(&sb, {r}_to_json({access}));")),
            CTy::Arr(e) => {
                let idx = self.fresh();
                self.push("    maca_sb_putc(&sb, '[');");
                self.push(&format!(
                    "    for (int64_t {idx} = 0; {idx} < {access}.len; {idx}++) {{ if ({idx}) maca_sb_putc(&sb, ',');"
                ));
                let inner = format!("{access}.data[{idx}]");
                self.emit_json_value(&inner, e);
                self.push("    }");
                self.push("    maca_sb_putc(&sb, ']');");
            }
            // a map has no ordered JSON shape without a key ordering the
            // format guarantees, and this serializer emits records
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
        self.push(&format!("    {name} v;"));
        for (fname, fty) in &fields {
            self.emit_json_read(&format!("v.{}", cid(fname)), fname, fty);
        }
        self.push("    return v;");
        self.push("}");
    }

    fn emit_json_read(&mut self, dest: &str, key: &str, t: &CTy) {
        let get = format!("maca_json_get(j, \"{key}\")");
        match t {
            CTy::Int => self.push(&format!("    {dest} = maca_json_int({get});")),
            CTy::Float | CTy::F32 => self.push(&format!("    {dest} = maca_json_float({get});")),
            CTy::Bool => self.push(&format!("    {dest} = maca_json_bool({get});")),
            CTy::Str => self.push(&format!("    {dest} = maca_json_str({get});")),
            CTy::Sum(s) => self.push(&format!("    {dest} = {s}_from_str(maca_json_str({get}));")),
            CTy::Rec(r) => self.push(&format!("    {dest} = {r}_from_json({get});")),
            CTy::Arr(e) => {
                let an = arr_name(e);
                let a = self.fresh();
                let idx = self.fresh();
                self.push(&format!(
                    "    {{ maca_json* {a} = {get}; {an} _acc = {an}_new();"
                ));
                self.push(&format!("      if ({a} && {a}->kind == MJ_ARR) for (int64_t {idx} = 0; {idx} < {a}->arr.len; {idx}++) {{"));
                let elem_read = json_read_inline(&format!("{a}->arr.items[{idx}]"), e);
                self.push(&format!("        {an}_push(&_acc, {elem_read});"));
                self.push("      }");
                self.push(&format!("      {dest} = _acc; }}"));
            }
            CTy::Map(_)
            | CTy::Unit
            | CTy::Unknown
            | CTy::Vec { .. }
            | CTy::Future
            | CTy::Closure(_)
            | CTy::Closure2(_) => self.push(&format!("    {dest} = 0;")),
        }
    }
}

/// Inline reader for an array element (`maca_json*` expression → element value).
fn json_read_inline(j: &str, t: &CTy) -> String {
    match t {
        CTy::Int => format!("maca_json_int({j})"),
        CTy::Float => format!("maca_json_float({j})"),
        CTy::Bool => format!("maca_json_bool({j})"),
        CTy::Str => format!("maca_json_str({j})"),
        CTy::Sum(s) => format!("{s}_from_str(maca_json_str({j}))"),
        CTy::Rec(r) => format!("{r}_from_json({j})"),
        _ => "0".into(),
    }
}

// ---- free helpers --------------------------------------------------------

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
            | Expr::Match { .. }
            | Expr::If { .. }
            | Expr::Block(_)
    )
}

/// The tail (last) expression of a statement block, if any — the value it yields.
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

/// The closure struct for a given arity. The runtime declares one per arity, so
/// the type a local is *declared* with has to match the one it is built as.
fn closure_ty(arity: usize, ret: CTy) -> CTy {
    if arity >= 2 {
        CTy::Closure2(Box::new(ret))
    } else {
        CTy::Closure(Box::new(ret))
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
        CTy::Unit | CTy::Unknown | CTy::Future | CTy::Closure(_) | CTy::Closure2(_) => "IntArr".into(),
    }
}

/// The monomorphized map type name for a value type — `Map str int` is
/// `IntMap`, mirroring how `int[]` is `IntArr`.
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
        CTy::Rec(_) | CTy::Vec { .. } => format!("({{ {} _z; _z; }})", c_type(t)),
        _ => "0".into(),
    }
}

fn to_str(code: &str, t: &CTy) -> String {
    match t {
        CTy::Str => code.to_string(),
        CTy::Int => format!("maca_from_int({code})"),
        CTy::Float | CTy::F32 => format!("maca_from_float({code})"),
        CTy::Bool => format!("maca_from_bool({code})"),
        // A value whose type we couldn't name — typically a call through a
        // closure parameter (`f(x)` where `f` is higher-order). Everything
        // crosses the closure ABI as `int64_t`, so render it as one; without
        // this the raw integer is handed to a `maca_str` parameter and the
        // program crashes.
        CTy::Unknown => format!("maca_from_int({code})"),
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

/// The canonical function name an operator overloads to for user types
/// (`a + b` → `add(a, b)`), or `None` for operators that never overload.
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

/// Escape a Maca identifier that collides with a C (or common C++) reserved
/// word so the emitted C stays valid. Applied uniformly at every function /
/// variable / parameter / field emission site; a no-op for ordinary names, so
/// it never changes output for non-colliding identifiers. JSON keys keep the
/// original (unescaped) name.
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
        // common C++ keywords (some C compilers / headers reserve them too)
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
        Expr::Binary { op: BinOp::Div, .. } => CTy::Str, // path join
        Expr::List(items) => {
            let elem = match items.first() {
                Some(first) => infer_cty_shallow(first),
                None => CTy::Str,
            };
            CTy::Arr(Box::new(elem))
        }
        Expr::Lambda { params, .. } => closure_ty(params.len(), CTy::Int),
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

/// `A | Circle(int) | Rect(int, int)` → variants with payload types (bare ident
/// = nullary, `Name(T, …)` = a payload-carrying variant).
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

/// The `Expr` a call argument carries.
fn arg_expr(a: &Arg) -> &Expr {
    match a {
        Arg::Pos(e) | Arg::Named { value: e, .. } | Arg::Directive { value: e, .. } => e,
    }
}

/// Whether a bind value is a record type declaration (`{ x: int, y: str }`).
fn is_record_value(e: &Expr) -> bool {
    matches!(e, Expr::Record(fs) if !fs.is_empty() && fs.iter().all(|f| matches!(f, Field::Type { .. })))
}

/// A function is generic if any parameter or the return type mentions a
/// type variable.
fn fn_is_generic(f: &FnDef) -> bool {
    f.params
        .iter()
        .any(|p| p.ty.as_ref().is_some_and(type_has_tyvar))
        || f.ret.as_ref().is_some_and(type_has_tyvar)
}

fn type_has_tyvar(t: &Type) -> bool {
    match t {
        Type::Name(segs) => segs.len() == 1 && is_tyvar(&segs[0]),
        Type::Array(i) | Type::Opt(i) | Type::Paren(i) => type_has_tyvar(i),
        Type::Apply(h, args) => type_has_tyvar(h) || args.iter().any(type_has_tyvar),
    }
}

/// A lowercase single-word type name that isn't a primitive is a type variable.
#[allow(clippy::nonminimal_bool)] // the positive conjunction reads clearer
fn is_tyvar(n: &str) -> bool {
    let b = n.as_bytes();
    !b.is_empty()
        && b[0].is_ascii_lowercase()
        && !matches!(n, "int" | "float" | "str" | "bool" | "bytes" | "unit")
        && !(matches!(b[0], b'i' | b'u' | b'f') && b.get(1).is_some_and(u8::is_ascii_digit))
}

/// A generic function's specialized C name for a concrete argument tuple, e.g.
/// `id__int`, `id__str`, `id__Box`.
fn mangle_name(name: &str, ctys: &[CTy]) -> String {
    let tags: Vec<String> = ctys.iter().map(cty_tag).collect();
    format!("{name}__{}", tags.join("_"))
}

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
        CTy::Closure(_) | CTy::Closure2(_) => "closure".into(),
        CTy::Unknown => "any".into(),
    }
}

/// Collect the identifiers a *simple-expression* lambda body references, into
/// `out`. Returns false if the body uses a form this increment doesn't hoist
/// (blocks, control flow, records, …) — the caller then declines to hoist,
/// which keeps capture analysis sound (no identifier can be missed).
/// Box a C value of type `t` into an `int64_t` for the closure-call boundary.
/// A `str` (pointer) fits via `intptr_t`; a `float`/`double` is bit-preserved.
fn box_i64(code: &str, t: &CTy) -> String {
    match t {
        CTy::Str => format!("(int64_t)(intptr_t)({code})"),
        CTy::Float => format!("maca_box_f64({code})"),
        CTy::F32 => format!("maca_box_f64((double)({code}))"),
        // A struct does not fit in the boundary word, so it crosses by
        // reference: a heap copy whose address is the boxed value. Without
        // this, `people.filter(p => p.age > 18)` emitted `(int64_t)(a_record)`
        // and the C compiler rejected the cast — a closure over a list of
        // records, which is most of what `map`/`filter` are for.
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
        // the mirror of `box_i64`: the boundary word is the address of a heap
        // copy, so read it back through that pointer
        CTy::Rec(_) | CTy::Sum(_) | CTy::Arr(_) | CTy::Map(_) | CTy::Vec { .. } => {
            format!("(*({}*)(intptr_t)({code}))", c_type(t))
        }
        _ => format!("(int64_t)({code})"),
    }
}

/// Collect the free variables of `e` — identifiers referenced but not bound by
/// an enclosing binder inside `e` (lambda params, `for`/`match` patterns, block
/// bindings). Used to compute a lambda's captures.
/// Collect the names invoked as a call callee anywhere in `e` (`f(x)` adds
/// `f`). Used to recognize a higher-order parameter: an unannotated param that
/// is called must hold a function value. A full structural walk mirroring
/// `free_vars`' reach.
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

/// Struct-shaped dependency: a record or a sum referenced by value (the caller
/// filters plain sums, which need no ordering). Recurses through arrays since
/// `MACA_DEFINE_ARRAY` embeds `sizeof(Elem)`.
fn struct_dep(t: &CTy) -> Option<String> {
    match t {
        CTy::Rec(n) | CTy::Sum(n) => Some(n.clone()),
        CTy::Arr(e) => struct_dep(e),
        _ => None,
    }
}

/// A strictly by-value type reference (arrays are heap pointers, so they do NOT
/// propagate a value cycle). Used for recursion detection / boxing.
fn value_dep(t: &CTy) -> Option<String> {
    match t {
        CTy::Rec(n) | CTy::Sum(n) => Some(n.clone()),
        _ => None,
    }
}

/// An operand of `++` where the other side is a string.
///
/// A *known* scalar is converted — `"h" ++ 3` used to hand an `int64_t` to a
/// `maca_str` parameter, compile with a warning nobody reads, and segfault
/// dereferencing address 3. A value of unknown type is passed through instead:
/// in a concatenation it is a string (`greet(n) => "hi " ++ n` with no
/// annotation on `n`), and rendering it as an integer would print its address.
fn concat_operand(code: &str, t: &CTy) -> String {
    match t {
        CTy::Int | CTy::Float | CTy::F32 | CTy::Bool => to_str(code, t),
        _ => code.to_string(),
    }
}

/// The struct name for an anonymous record's shape.
///
/// Derived from the shape rather than from a counter, so the same literal
/// written in two different functions is one struct and the two values are
/// assignable to each other. `{ host = "x", port = 80 }` becomes
/// `MacaAnon_host_str_port_int`, which is also readable in the generated C.
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
///
/// A lambda written at a call site gets its parameter type from the receiver's
/// element type. One written for a higher-order *parameter* has no such
/// context, so the body is the evidence: compared against a string, concatenated
/// with one, or sent a string-only method means `str`; compared against a float
/// means `float`. Anything else stays the integer default, which is also how a
/// pointer crosses the closure boundary.
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
            // methods only a `str` has — `length`/`slice`/`contains` are on
            // both, so they say nothing
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
///
/// Surface Maca has no function-type syntax, so a higher-order parameter is
/// recognized from use. Calling it is the obvious evidence — `pred(x)` means
/// `pred` is a function. But a parameter is just as often *forwarded*: the
/// public `any_of(xs, pred)` hands `pred` to a recursive `scan_any(xs, i, pred)`
/// that does the calling. Looking only at the immediate body typed the two ends
/// of that pair differently, and the C compiler rejected the call.
///
/// So this runs to a fixpoint: a parameter is a function value if it is called,
/// or if it is passed into a position already known to be one. Two functions
/// that forward to each other settle in one extra round.
fn closure_params(items: &[Stmt]) -> HashSet<(String, usize)> {
    let fns: Vec<&FnDef> = items
        .iter()
        .filter_map(|it| match it {
            Stmt::Fn(f) => Some(f),
            _ => None,
        })
        .collect();
    let mut out: HashSet<(String, usize)> = HashSet::new();
    // seed: a parameter that is called in its own body, or handed to a list
    // method that takes a function
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
    // propagate: `f` forwards its parameter into a known function position
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

/// Bare identifiers handed to a list method that takes a function —
/// `xs.filter(pred)`. Evidence that the name holds one, the same as calling it.
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
        // the argument index the function goes in: `reduce`/`fold` take the
        // seed first
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

/// Every `g(…, x, …)` in a body, as `(g, index, x)` for a bare-identifier
/// argument — the shape that forwards a value on unchanged.
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

/// How deeply an array type nests — `int` is 0, `int[]` is 1, `int[][]` is 2.
fn arr_depth(t: &CTy) -> usize {
    match t {
        CTy::Arr(e) => 1 + arr_depth(e),
        CTy::Map(v) => 1 + arr_depth(v),
        _ => 0,
    }
}

/// Types whose value carries a heap buffer this back end allocated and can
/// therefore name. A record's fields may be heap too, but the record does not
/// own them exclusively, so it is left out.
fn owns_heap(t: &CTy) -> bool {
    matches!(t, CTy::Arr(_) | CTy::Map(_))
}

/// Names that may outlive the statement list they are bound in.
///
/// The analysis is deliberately lopsided. Missing an escape frees a buffer
/// someone still holds; missing a *non*-escape only means the program keeps
/// memory it could have released, which is exactly what it did before any of
/// this existed. So a name counts as escaping the moment it appears anywhere
/// its value could be kept: a call argument, a list or record element, another
/// binding's value, the tail expression, a `return` or `fail`.
///
/// The one position that does *not* escape is the receiver of a method call.
/// Every array and map method in this back end builds a fresh buffer or reads
/// one — none stores its receiver — so `xs.length()` and `xs.sort()` leave `xs`
/// owned by the block that built it. Without that exemption almost nothing
/// would qualify, since a value you never look at is not worth building.
fn escaping_names(stmts: &[Stmt]) -> HashSet<String> {
    let mut out = HashSet::new();
    let n = stmts.len();
    for (i, s) in stmts.iter().enumerate() {
        match s {
            // the tail expression carries the block's value out
            Stmt::Expr(e) if i + 1 == n => note_escapes_all(e, &mut out),
            Stmt::Expr(e) => note_escapes(e, &mut out),
            Stmt::Bind(b) => {
                // a value bound to another name is now that name's problem
                note_escapes_all(&b.value, &mut out);
                note_escapes(&b.target, &mut out);
            }
            Stmt::Fn(f) => match &f.body {
                // a nested function can capture, so treat its whole body as an
                // escape context
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

/// Every name in `e` escapes — used where the expression's value is kept.
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
        // the receiver is read, not retained; its own children still count
        if !matches!(&**base, Expr::Ident(_)) {
            note_escapes(base, out);
        }
        for a in args {
            note_escapes_all(arg_value(a), out);
        }
        return;
    }
    if let Expr::Ident(n) = e {
        out.insert(n.clone());
    }
    walk_children(e, &mut |c| note_escapes(c, out));
}

fn arg_value(a: &Arg) -> &Expr {
    match a {
        Arg::Pos(e) | Arg::Named { value: e, .. } | Arg::Directive { value: e, .. } => e,
    }
}

/// Apply `f` to each direct sub-expression of `e`.
fn walk_children(e: &Expr, f: &mut dyn FnMut(&Expr)) {
    let stmts = |ss: &[Stmt], f: &mut dyn FnMut(&Expr)| {
        for s in ss {
            match s {
                Stmt::Expr(e) | Stmt::Bind(Bind { value: e, .. }) => f(e),
                _ => {}
            }
        }
    };
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
            args.iter().for_each(|a| f(arg_value(a)));
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
        _ => {}
    }
}

/// Join already-lowered string expressions into one `maca_concat` chain.
fn fold_concat(parts: Vec<String>) -> String {
    parts.into_iter().fold("\"\"".to_string(), |acc, p| {
        format!("maca_concat({acc}, {p})")
    })
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
