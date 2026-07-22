//! maca-backend-c: lower the AST to C.
//!
//! Type-directed lowering. Records become C structs, nullary sums become
//! enums, `T[]` becomes a monomorphized dynamic array (via the runtime's
//! `MACA_DEFINE_ARRAY` macro), and `std/json` encode/decode are generated
//! per record/sum type. Strings are heap `maca_str`; interpolation folds to
//! `maca_concat`. `match` lowers to an if-else chain, `?` is transparent, and
//! `fail msg` calls `maca_fail` — a clean stderr message + `exit(1)` (the
//! runtime file/JSON helpers still abort; catch-via-`reify` comes later).
//!
//! GNU statement expressions (`({ ...; v; })`, supported by `zig cc`/clang) are
//! used for inline list literals.

use maca_parser::ast::*;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

pub fn emit(m: &Module) -> String {
    let mut cx = Cx::new(m);
    cx.collect();
    cx.emit_all();
    cx.out
}

/// Whether the generated C uses the async runtime (so the driver links it).
pub fn needs_async(c_src: &str) -> bool {
    c_src.contains("maca_parallel_i64") || c_src.contains("maca_cancel")
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
    /// SIMD vector, e.g. `f32x8` → { name: "f32x8", scalar_c: "float", lanes: 8 }
    Vec { name: String, scalar_c: String, lanes: usize },
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
    sums: BTreeMap<String, Vec<String>>,     // sum name -> variants
    variant_of: HashMap<String, String>,     // variant -> sum name
    records: BTreeMap<String, Vec<(String, CTy)>>, // record name -> ordered fields
    rec_order: Vec<String>,                  // topo order
    modules: HashSet<String>,                // imported module names (json, dirs, …)
    lets: Vec<(String, CTy, Expr)>,          // top-level `let`/value bindings
    let_names: HashSet<String>,
    fns: HashMap<String, (Vec<CTy>, CTy)>,   // fn name -> (param types, ret)
    arr_elems: HashSet<CTy>,                  // array element types to instantiate
    vecs: BTreeSet<(String, String, usize)>, // SIMD vector types (name, scalar_c, lanes)
    tmp: usize,
}

impl<'a> Cx<'a> {
    fn new(m: &'a Module) -> Self {
        Cx {
            m,
            out: String::new(),
            sums: BTreeMap::new(),
            variant_of: HashMap::new(),
            records: BTreeMap::new(),
            rec_order: Vec::new(),
            modules: HashSet::new(),
            lets: Vec::new(),
            let_names: HashSet::new(),
            fns: HashMap::new(),
            arr_elems: HashSet::new(),
            vecs: BTreeSet::new(),
            tmp: 0,
        }
    }

    fn note_vec(&mut self, t: &CTy) {
        if let CTy::Vec { name, scalar_c, lanes } = t {
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

    fn collect(&mut self) {
        // first pass: sums + records + modules + fn signatures + lets
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
                            for v in &vars {
                                self.variant_of.insert(v.clone(), name.clone());
                            }
                            self.sums.insert(name.clone(), vars);
                        }
                    }
                }
                _ => {}
            }
        }
        // records need the sum set to classify field types
        for item in &self.m.items {
            if let Stmt::Bind(b) = item {
                if let Expr::Ident(name) = &b.target {
                    if let Some(fields) = self.record_fields(&b.value) {
                        self.records.insert(name.clone(), fields);
                    }
                }
            }
        }
        // fn signatures, lets
        for item in &self.m.items {
            match item {
                Stmt::Fn(f) => {
                    let params = f.params.iter().map(|p| self.cty_opt(&p.ty)).collect::<Vec<_>>();
                    let ret = f.ret.as_ref().map_or(CTy::Unit, |t| self.cty(t));
                    for p in &params {
                        self.note_arr(p);
                        self.note_vec(p);
                    }
                    self.note_arr(&ret);
                    self.note_vec(&ret);
                    self.fns.insert(f.name.clone(), (params, ret));
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
        let fields: Vec<CTy> =
            self.records.values().flatten().map(|(_, t)| t.clone()).collect();
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
                Stmt::Bind(b) => self.walk_expr(&b.value),
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
                    if name == "splat" {
                        if let Expr::Ident(v) = base.as_ref() {
                            if let Some((sc, ln)) = parse_vec_c(v) {
                                self.vecs.insert((v.clone(), sc, ln));
                            }
                        }
                    }
                }
                self.walk_expr(callee);
                for a in args {
                    match a {
                        Arg::Pos(x) | Arg::Named { value: x, .. } | Arg::Directive { value: x, .. } => {
                            self.walk_expr(x)
                        }
                    }
                }
            }
            Expr::Ctor { fields, .. } | Expr::Record(fields) => {
                for f in fields {
                    if let Field::Value { value, .. } | Field::Bare(value) = f {
                        self.walk_expr(value);
                    }
                }
            }
            Expr::Field { base, .. } | Expr::Unary { expr: base, .. } | Expr::Try(base)
            | Expr::Fail(base) | Expr::Reify(base) => self.walk_expr(base),
            Expr::Binary { lhs, rhs, .. } | Expr::Assign { target: lhs, value: rhs } => {
                self.walk_expr(lhs);
                self.walk_expr(rhs);
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
    fn shallow_cty(&self, e: &Expr) -> CTy {
        match e {
            Expr::Int(_) => CTy::Int,
            Expr::Float(_) => CTy::Float,
            Expr::Bool(_) => CTy::Bool,
            Expr::Str(_) | Expr::Path(_) => CTy::Str,
            Expr::Ctor { name, .. } => CTy::Rec(name.clone()),
            Expr::Ident(n) => {
                self.variant_of.get(n).map(|s| CTy::Sum(s.clone())).unwrap_or(CTy::Unknown)
            }
            Expr::Call { callee, .. } => match &**callee {
                Expr::Ident(n) => self.fns.get(n).map(|(_, r)| r.clone()).unwrap_or(CTy::Unknown),
                _ => CTy::Unknown,
            },
            _ => CTy::Unknown,
        }
    }

    fn note_arr(&mut self, t: &CTy) {
        if let CTy::Arr(e) = t {
            self.arr_elems.insert((**e).clone());
            self.note_arr(e);
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
                    return CTy::Vec { name: n.into(), scalar_c, lanes };
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

        // enums + to_str/from_str
        let sums: Vec<(String, Vec<String>)> =
            self.sums.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
        for (name, vars) in &sums {
            let variants =
                vars.iter().map(|v| format!("{name}_{v}")).collect::<Vec<_>>().join(", ");
            self.push(&format!("typedef enum {{ {variants} }} {name};"));
        }
        for (name, vars) in &sums {
            self.push(&format!("static maca_str {name}_to_str({name} v) {{"));
            self.push("    switch (v) {");
            for v in vars {
                self.push(&format!("        case {name}_{v}: return \"{v}\";"));
            }
            self.push("    }");
            self.push("    return \"\";");
            self.push("}");
            self.push(&format!("static {name} {name}_from_str(maca_str s) {{"));
            for v in vars {
                self.push(&format!("    if (maca_str_eq(s, \"{v}\")) return {name}_{v};"));
            }
            self.push(&format!("    return {name}_{};", vars[0]));
            self.push("}");
        }
        self.push("");

        // primitive-element arrays
        let mut emitted_arr: HashSet<CTy> = HashSet::new();
        let elems: Vec<CTy> = self.arr_elems.iter().cloned().collect();
        for e in &elems {
            if !matches!(e, CTy::Rec(_)) {
                self.push(&format!("MACA_DEFINE_ARRAY({}, {})", arr_name(e), c_type(e)));
                emitted_arr.insert(e.clone());
            }
        }
        self.push("");

        // records in topo order, interleaving record-element arrays
        let order = self.rec_order.clone();
        for name in &order {
            let fields = self.records[name].clone();
            for (_, t) in &fields {
                if let CTy::Arr(e) = t {
                    if matches!(**e, CTy::Rec(_)) && !emitted_arr.contains(e) {
                        self.push(&format!("MACA_DEFINE_ARRAY({}, {})", arr_name(e), c_type(e)));
                        emitted_arr.insert((**e).clone());
                    }
                }
            }
            self.push(&format!("typedef struct {{"));
            for (fname, t) in &fields {
                self.push(&format!("    {} {fname};", c_type(t)));
            }
            self.push(&format!("}} {name};"));
        }
        self.push("");

        // json forward decls + defs
        for name in &order {
            self.push(&format!("static maca_str {name}_to_json({name} v);"));
            self.push(&format!("static {name} {name}_from_json(maca_json* j);"));
        }
        self.push("");
        for name in &order {
            self.emit_to_json(name);
            self.emit_from_json(name);
        }

        // top-level let accessors
        let lets = self.lets.clone();
        for (name, cty, init) in &lets {
            let ret = if *cty == CTy::Unknown { infer_cty_shallow(init) } else { cty.clone() };
            let mut env: Env = Vec::new();
            let (code, _) = self.expr(&mut env, init, None);
            self.push(&format!("static {} mv_{name}(void) {{ return {code}; }}", c_type(&ret)));
        }
        self.push("");

        // user fn forward decls (SIMD kernels are defined on the LLVM path, so
        // they are declared `extern` here and skipped in the def loop)
        for item in &self.m.items {
            if let Stmt::Fn(f) = item {
                if f.name == "main" {
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

        // user fn defs (skip SIMD kernels — the LLVM backend defines them)
        for item in self.m.items.clone() {
            if let Stmt::Fn(f) = &item {
                if f.body.is_some() && !self.is_simd_fn(&f.name) {
                    self.emit_fn(f);
                    self.push("");
                }
            }
        }
    }

    fn fn_sig(&self, f: &FnDef) -> String {
        let (params, ret) = &self.fns[&f.name];
        let ps: Vec<String> = f
            .params
            .iter()
            .zip(params)
            .map(|(p, t)| format!("{} {}", c_type(t), p.name))
            .collect();
        format!("{} {}({})", c_type(ret), f.name, if ps.is_empty() { "void".into() } else { ps.join(", ") })
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
            self.push("int main(int argc, char** argv) {");
            self.push("    maca_init();");
            // build args: str[]
            if let Some((pname, _)) = f.params.first().map(|p| (p.name.clone(), ())) {
                env.push((pname.clone(), CTy::Arr(Box::new(CTy::Str))));
                self.push(&format!("    {} {pname} = {}_new();", arr_name(&CTy::Str), arr_name(&CTy::Str)));
                self.push(&format!("    for (int _i = 1; _i < argc; _i++) {}_push(&{pname}, argv[_i]);", arr_name(&CTy::Str)));
            }
            match &f.body {
                Some(FnBody::Block(stmts)) => self.block(&mut env, stmts, &Sink::Return(CTy::Int), 1),
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
                let sink = if ret == CTy::Unit { Sink::Discard } else { Sink::Return(ret.clone()) };
                self.block(&mut env, stmts, &sink, 1);
            }
            Some(FnBody::Expr(e)) => {
                let (c, _) = self.expr(&mut env, e, Some(&ret));
                if ret == CTy::Unit {
                    self.push(&format!("    (void)({c});"));
                } else {
                    self.push(&format!("    return {c};"));
                }
            }
            None => {}
        }
        self.push("}");
    }
}

type Env = Vec<(String, CTy)>;

fn lookup(env: &Env, n: &str) -> Option<CTy> {
    env.iter().rev().find(|(k, _)| k == n).map(|(_, t)| t.clone())
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
        for (i, s) in stmts.iter().enumerate() {
            let last = i + 1 == stmts.len();
            match s {
                Stmt::Bind(b) if b.is_let => {
                    if let Expr::Ident(name) = &b.target {
                        let ann = b.tys.first().map(|t| self.cty(t));
                        if is_control(&b.value) {
                            // `let x = if/match/block …` — declare, then assign
                            // the result in each branch tail via an Assign sink.
                            let ty = ann.clone().unwrap_or_else(|| self.result_cty(env, &b.value));
                            self.indent(ind);
                            self.push(&format!("{} {name};", c_type(&ty)));
                            env.push((name.clone(), ty.clone()));
                            self.stmt_expr(env, &b.value, &Sink::Assign(name.clone(), ty), ind);
                        } else {
                            let (code, cty) = self.expr(env, &b.value, ann.as_ref());
                            let ty = ann.unwrap_or(cty);
                            self.indent(ind);
                            self.push(&format!("{} {name} = {code};", c_type(&ty)));
                            env.push((name.clone(), ty));
                        }
                    }
                }
                // reassignment: a non-`let` bind to an in-scope name (`i = i + 1`),
                // which makes counters and `while` loops usable.
                Stmt::Bind(b) => {
                    if let Expr::Ident(name) = &b.target {
                        if let Some(ty) = lookup(env, name) {
                            let (code, _) = self.expr(env, &b.value, Some(&ty));
                            self.indent(ind);
                            self.push(&format!("{name} = {code};"));
                        }
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
        env.truncate(base);
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
                let t = tail_expr(then).map(|e| self.result_cty(env, e)).unwrap_or(CTy::Unknown);
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
            Expr::Block(stmts) => {
                tail_expr(stmts).map(|e| self.result_cty(env, e)).unwrap_or(CTy::Unit)
            }
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
            Expr::For { pat, iter, body } => {
                // a `for` loop has no value; its body is always in statement position
                let (ic, ity) = self.expr(env, iter, None);
                let elem = match ity {
                    CTy::Arr(e) => *e,
                    _ => CTy::Unknown,
                };
                let it = self.fresh();
                let idx = self.fresh();
                let var = if let Pattern::Bind(n) = pat { n.clone() } else { self.fresh() };
                let an = arr_name(&elem);
                self.indent(ind);
                self.push(&format!("{{ {an} {it} = {ic}; for (int64_t {idx} = 0; {idx} < {it}.len; {idx}++) {{"));
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
        for (i, arm) in arms.iter().enumerate() {
            let (cond, binds) = self.pattern_cond(&sv, &sty, &elem, &arm.pat);
            let kw = if i == 0 { "if" } else { "else if" };
            self.indent(ind);
            if cond == "1" {
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
            // A nullary variant may reach here as a bare `Bind` (bare `Red`) or a
            // `Ctor` (`Red()`); against a sum scrutinee it is a tag test, not a
            // binding — mirror the checker's `is_variant` disambiguation.
            Pattern::Bind(n) | Pattern::Ctor { name: n, args: _ }
                if matches!(sty, CTy::Sum(s) if self.sums.get(s).is_some_and(|vs| vs.iter().any(|v| v == n))) =>
            {
                let CTy::Sum(s) = sty else { unreachable!() };
                (format!("{sv} == {s}_{n}"), vec![])
            }
            Pattern::Bind(n) => {
                // bind whole scrutinee
                (
                    "1".into(),
                    vec![(n.clone(), format!("{} {n} = {sv};", c_type(sty)), sty.clone())],
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
                if let Some(r) = rest {
                    if let Pattern::Bind(rn) = &**r {
                        binds.push((
                            rn.clone(),
                            format!(
                                "{an} {rn} = {an}_slice({sv}, {n});",
                                an = arr_name(elem)
                            ),
                            CTy::Arr(Box::new(elem.clone())),
                        ));
                    }
                }
                (conds.join(" && "), binds)
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
            Expr::List(es) => self.list(env, es, expected),
            Expr::Call { callee, args } => self.call(env, callee, args, expected),
            Expr::Field { base, name } => self.field(env, base, name),
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
                (format!("({c} ? {t} : {e2})"), expected.cloned().unwrap_or(tt))
            }
            Expr::Try(x) => self.expr(env, x, expected),
            // `fail msg` — a clean error exit with the message (was a bare abort()
            // that discarded the message and raised SIGABRT). `maca_fail` is
            // noreturn; the trailing `0` only keeps the expression well-typed.
            Expr::Fail(msg) => {
                let (mc, _) = self.expr(env, msg, Some(&CTy::Str));
                (format!("(maca_fail({mc}), 0)"), expected.cloned().unwrap_or(CTy::Unit))
            }
            _ => ("0 /* unsupported */".into(), CTy::Unknown),
        }
    }

    fn ident(&mut self, env: &Env, n: &str) -> (String, CTy) {
        if let Some(t) = lookup(env, n) {
            return (n.to_string(), t);
        }
        if let Some(sum) = self.variant_of.get(n).cloned() {
            return (format!("{sum}_{n}"), CTy::Sum(sum));
        }
        if self.let_names.contains(n) {
            let ty = self
                .lets
                .iter()
                .find(|(name, _, _)| name == n)
                .map(|(_, t, init)| if *t == CTy::Unknown { infer_cty_shallow(init) } else { t.clone() })
                .unwrap_or(CTy::Unknown);
            return (format!("mv_{n}()"), ty);
        }
        (n.to_string(), CTy::Unknown)
    }

    fn ctor(&mut self, env: &mut Env, name: &str, fields: &[Field]) -> (String, CTy) {
        let decl = self.records.get(name).cloned();
        let Some(decl) = decl else {
            return ("0 /* unknown ctor */".into(), CTy::Unknown);
        };
        let mut parts = Vec::new();
        for (fname, fty) in &decl {
            let val = fields.iter().find_map(|f| match f {
                Field::Value { name: n, value } if n == fname => Some((value.clone(), false)),
                Field::Shorthand(n) if n == fname => {
                    Some((Expr::Ident(n.clone()), true))
                }
                _ => None,
            });
            let code = match val {
                Some((v, _)) => self.expr(env, &v, Some(fty)).0,
                None => zero_value(fty),
            };
            parts.push(format!(".{fname} = {code}"));
        }
        (format!("(({name}){{ {} }})", parts.join(", ")), CTy::Rec(name.into()))
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

    fn call(
        &mut self,
        env: &mut Env,
        callee: &Expr,
        args: &[Arg],
        expected: Option<&CTy>,
    ) -> (String, CTy) {
        // module.member(...) and UFCS receiver.method(...)
        if let Expr::Field { base, name } = callee {
            if let Expr::Ident(m) = &**base {
                // `f32x8.splat(k)` → a broadcast compound literal
                if name == "splat" {
                    if let Some((_, lanes)) = parse_vec_c(m) {
                        let k = self.arg(env, &args[0]);
                        let elems = vec![k.as_str(); lanes].join(", ");
                        let (sc, ln) = parse_vec_c(m).unwrap();
                        return (
                            format!("(({m}){{ {elems} }})"),
                            CTy::Vec { name: m.clone(), scalar_c: sc, lanes: ln },
                        );
                    }
                }
                if self.modules.contains(m) {
                    return self.module_call(env, m, name, args, expected);
                }
            }
            // UFCS: receiver.method(args)
            let (rc, rty) = self.expr(env, base, None);
            let a: Vec<String> = args.iter().map(|x| self.arg(env, x)).collect();
            return self.ufcs(&rc, &rty, name, &a);
        }
        if let Expr::Ident(name) = callee {
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
            let a: Vec<String> = args.iter().map(|x| self.arg(env, x)).collect();
            if let Some(cfn) = console_fn(name) {
                return (format!("{cfn}({})", a.join(", ")), CTy::Unit);
            }
            if let Some((_, ret)) = self.fns.get(name).cloned() {
                return (format!("{name}({})", a.join(", ")), ret);
            }
            let _ = expected;
            return (format!("{name}({})", a.join(", ")), CTy::Unknown);
        }
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
                    Some(CTy::Rec(r)) => {
                        (format!("{r}_from_json(maca_json_parse({c}))"), CTy::Rec(r.clone()))
                    }
                    _ => (format!("maca_json_parse({c})"), CTy::Unknown),
                }
            }
            _ => {
                let a: Vec<String> = args.iter().map(|x| self.arg(env, x)).collect();
                (format!("/* {module}.{member} */ ({})", a.join(", ")), CTy::Unknown)
            }
        }
    }

    fn ufcs(&self, rc: &str, rty: &CTy, method: &str, a: &[String]) -> (String, CTy) {
        match (rty, method) {
            (_, "read") => (format!("maca_read({rc})"), CTy::Str),
            (_, "exists") => (format!("maca_path_exists({rc})"), CTy::Bool),
            (_, "write") => (format!("(maca_write({rc}, {}), 0)", a.first().cloned().unwrap_or_default()), CTy::Unit),
            (CTy::Arr(e), "join") if matches!(**e, CTy::Str) => (
                format!("maca_join({rc}.data, {rc}.len, {})", a.first().cloned().unwrap_or_default()),
                CTy::Str,
            ),
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
            _ => (format!("{method}({rc}{}{})", if a.is_empty() { "" } else { ", " }, a.join(", ")), CTy::Unknown),
        }
    }

    fn field(&mut self, env: &mut Env, base: &Expr, name: &str) -> (String, CTy) {
        if let Expr::Ident(m) = base {
            if self.modules.contains(m) {
                if m == "dirs" && name == "data" {
                    return ("maca_dirs_data()".into(), CTy::Str);
                }
                return (format!("/* {m}.{name} */ \"\""), CTy::Unknown);
            }
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
        (format!("({bc}).{name}"), fty)
    }

    fn binary(&mut self, env: &mut Env, op: BinOp, lhs: &Expr, rhs: &Expr) -> (String, CTy) {
        let (lc, lt) = self.expr(env, lhs, None);
        let (rc, _rt) = self.expr(env, rhs, None);

        // Operator overloading: when the left operand is a user type (record /
        // sum) and a function with the operator's canonical name exists, the
        // operator desugars to a call — `a + b` → `add(a, b)`. Primitives keep
        // the native operator.
        if matches!(lt, CTy::Rec(_) | CTy::Sum(_)) {
            if let Some(name) = overload_name(op) {
                if let Some((_, ret)) = self.fns.get(name).cloned() {
                    return (format!("{name}({lc}, {rc})"), ret);
                }
            }
        }

        use BinOp::*;
        match op {
            Div if matches!(lt, CTy::Str) => (format!("maca_path_join({lc}, {rc})"), CTy::Str),
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
        match a {
            Arg::Pos(e) | Arg::Named { value: e, .. } | Arg::Directive { value: e, .. } => {
                self.expr(env, e, None)
            }
        }
    }

    fn interp(&mut self, env: &mut Env, parts: &[StrPart]) -> String {
        if parts.is_empty() {
            return "\"\"".into();
        }
        if parts.len() == 1 {
            if let StrPart::Text(t) = &parts[0] {
                return c_str(t);
            }
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
            self.emit_json_value(&format!("v.{fname}"), fty);
        }
        self.push("    maca_sb_putc(&sb, '}');");
        self.push("    return maca_sb_finish(&sb);");
        self.push("}");
    }

    fn emit_json_value(&mut self, access: &str, t: &CTy) {
        match t {
            CTy::Int => self.push(&format!("    maca_sb_puts(&sb, maca_from_int({access}));")),
            CTy::Float | CTy::F32 => {
                self.push(&format!("    maca_sb_puts(&sb, maca_from_float({access}));"))
            }
            CTy::Bool => {
                self.push(&format!("    maca_sb_puts(&sb, {access} ? \"true\" : \"false\");"))
            }
            CTy::Str => self.push(&format!("    maca_sb_put_json_str(&sb, {access});")),
            CTy::Sum(s) => {
                self.push(&format!("    maca_sb_put_json_str(&sb, {s}_to_str({access}));"))
            }
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
            CTy::Unit | CTy::Unknown | CTy::Vec { .. } => {
                self.push("    maca_sb_puts(&sb, \"null\");")
            }
        }
    }

    fn emit_from_json(&mut self, name: &str) {
        let fields = self.records[name].clone();
        self.push(&format!("static {name} {name}_from_json(maca_json* j) {{"));
        self.push(&format!("    {name} v;"));
        for (fname, fty) in &fields {
            self.emit_json_read(&format!("v.{fname}"), fname, fty);
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
            CTy::Sum(s) => {
                self.push(&format!("    {dest} = {s}_from_str(maca_json_str({get}));"))
            }
            CTy::Rec(r) => self.push(&format!("    {dest} = {r}_from_json({get});")),
            CTy::Arr(e) => {
                let an = arr_name(e);
                let a = self.fresh();
                let idx = self.fresh();
                self.push(&format!("    {{ maca_json* {a} = {get}; {an} _acc = {an}_new();"));
                self.push(&format!("      if ({a} && {a}->kind == MJ_ARR) for (int64_t {idx} = 0; {idx} < {a}->arr.len; {idx}++) {{"));
                let elem_read = json_read_inline(&format!("{a}->arr.items[{idx}]"), e);
                self.push(&format!("        {an}_push(&_acc, {elem_read});"));
                self.push("      }");
                self.push(&format!("      {dest} = _acc; }}"));
            }
            CTy::Unit | CTy::Unknown | CTy::Vec { .. } => {
                self.push(&format!("    {dest} = 0;"))
            }
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
        CTy::Vec { name, .. } => name.clone(),
        CTy::Unknown => "int64_t".into(),
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
        CTy::Unit | CTy::Unknown => "IntArr".into(),
    }
}

fn zero_value(t: &CTy) -> String {
    match t {
        CTy::Str => "\"\"".into(),
        CTy::Bool => "false".into(),
        CTy::F32 | CTy::Float => "0.0".into(),
        CTy::Arr(e) => format!("{}_new()", arr_name(e)),
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

fn sum_variants(e: &Expr) -> Option<Vec<String>> {
    fn go(e: &Expr, out: &mut Vec<String>) -> bool {
        match e {
            Expr::Ident(n) => {
                out.push(n.clone());
                true
            }
            Expr::Binary { op: BinOp::Union, lhs, rhs } => go(lhs, out) && go(rhs, out),
            _ => false,
        }
    }
    let mut out = Vec::new();
    match e {
        Expr::Binary { op: BinOp::Union, .. } if go(e, &mut out) => Some(out),
        _ => None,
    }
}

fn rec_dep(t: &CTy) -> Option<String> {
    match t {
        CTy::Rec(n) => Some(n.clone()),
        CTy::Arr(e) => rec_dep(e),
        _ => None,
    }
}
