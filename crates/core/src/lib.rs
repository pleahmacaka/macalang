//! maca-core: name/type/effect checking over the parser's AST.
//!
//! Pragmatic gradual checker (see docs/PLAN.md P3): unification-based inference
//! with an `any` escape hatch for unknown stdlib/foreign values, made *strict*
//! on the four diagnostics the acceptance requires:
//!   * type mismatch (annotated return / `let x: T =`),
//!   * non-exhaustive `match` on a nominal sum,
//!   * effects used in config mode (must be pure `<>`),
//!   * unknown NixOS option root in config mode.
//!
//! Function signatures are generalized into rank-1 type schemes and
//! instantiated per call site, so generics like `id(x: a) -> a` are usable at
//! many types. Full HM generalization over inferred (un-annotated) bindings and
//! real row unification are future hardening.

mod ty;

use maca_parser::ast::*;
use std::collections::{BTreeMap, HashMap, HashSet};
use ty::{EffSet, Infer, Scheme, Ty, EXN, IO, NET, OS};

pub use ty::show;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    Program,
    Config,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DiagKind {
    TypeMismatch,
    NonExhaustive,
    EffectInConfig,
    UnknownOption,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Diagnostic {
    pub kind: DiagKind,
    pub msg: String,
}

/// Check a module. An empty result means it passed.
pub fn check(module: &Module, mode: Mode) -> Vec<Diagnostic> {
    let mut c = Checker::new(mode);
    c.collect(module);
    c.run(module);
    c.diags
}

/// Prelude functions that carry the `io` effect.
const IO_FNS: &[&str] = &[
    "info", "warn", "err", "debug", "notice", "crit", "alert", "emerg", "panic", "print", "input",
];
/// UFCS method names treated as `io` (file/stdio side effects).
const IO_METHODS: &[&str] = &["read", "write", "exists", "remove", "append", "create"];

/// Top-level NixOS / home-manager option namespaces we recognize.
const NIXOS_ROOTS: &[&str] = &[
    "networking",
    "system",
    "services",
    "users",
    "user",
    "environment",
    "programs",
    "fonts",
    "boot",
    "hardware",
    "security",
    "nix",
    "nixpkgs",
    "virtualisation",
    "systemd",
    "i18n",
    "time",
    "sound",
    "xdg",
    "home",
    "imports",
    "console",
    "powerManagement",
    "documentation",
    "location",
];

type Env = Vec<(String, Ty)>;

struct Checker {
    mode: Mode,
    inf: Infer,
    globals: HashMap<String, Scheme>,
    records: HashMap<String, BTreeMap<String, Ty>>,
    sums: HashMap<String, Vec<String>>,
    /// User-declared function name -> (fixed param count, is variadic). Used to
    /// catch call-arity mistakes; variadic functions are exempt.
    fn_arity: HashMap<String, (usize, bool)>,
    type_decls: HashSet<usize>, // item indices that are type declarations
    local_names: HashSet<String>,
    diags: Vec<Diagnostic>,
}

impl Checker {
    fn new(mode: Mode) -> Self {
        Checker {
            mode,
            inf: Infer::default(),
            globals: HashMap::new(),
            records: HashMap::new(),
            sums: HashMap::new(),
            fn_arity: HashMap::new(),
            type_decls: HashSet::new(),
            local_names: HashSet::new(),
            diags: Vec::new(),
        }
    }

    fn diag(&mut self, kind: DiagKind, msg: impl Into<String>) {
        self.diags.push(Diagnostic { kind, msg: msg.into() });
    }

    // ---- pass 1: collect declarations ------------------------------------

    fn collect(&mut self, m: &Module) {
        for f in IO_FNS {
            self.globals.insert((*f).into(), Scheme::mono(Ty::Fn(vec![Ty::Any], Box::new(Ty::Unit))));
        }
        for (n, t) in [
            ("int", Ty::Fn(vec![Ty::Any], Box::new(Ty::Int))),
            ("float", Ty::Fn(vec![Ty::Any], Box::new(Ty::Float))),
            ("str", Ty::Fn(vec![Ty::Any], Box::new(Ty::Str))),
            ("len", Ty::Fn(vec![Ty::Any], Box::new(Ty::Int))),
            ("input", Ty::Fn(vec![], Box::new(Ty::Str))),
        ] {
            self.globals.insert(n.into(), Scheme::mono(t));
        }

        for (i, item) in m.items.iter().enumerate() {
            match item {
                Stmt::Import(im) => self.collect_import(im),
                Stmt::Alias { name, .. } => {
                    self.globals.insert(name.clone(), Scheme::mono(Ty::Any));
                    self.local_names.insert(name.clone());
                }
                Stmt::Fn(f) => {
                    let scheme = self.sig_scheme(f);
                    self.globals.insert(f.name.clone(), scheme);
                    self.local_names.insert(f.name.clone());
                    let variadic = f.params.iter().any(|p| p.variadic);
                    self.fn_arity.insert(f.name.clone(), (f.params.len(), variadic));
                }
                Stmt::Bind(b) => {
                    if let Expr::Ident(name) = &b.target {
                        self.local_names.insert(name.clone());
                        if let Some(vars) = sum_variants(&b.value) {
                            self.sums.insert(name.clone(), vars.clone());
                            for v in vars {
                                self.globals
                                    .insert(v, Scheme::mono(Ty::Con(name.clone(), vec![])));
                            }
                            self.type_decls.insert(i);
                        } else if let Some(fields) = record_type(&b.value) {
                            self.records.insert(name.clone(), fields);
                            self.type_decls.insert(i);
                        } else {
                            self.globals.insert(name.clone(), Scheme::mono(Ty::Any));
                        }
                    }
                }
                Stmt::Expr(_) => {}
            }
        }
    }

    fn collect_import(&mut self, im: &Import) {
        fn bind(c: &mut Checker, n: String) {
            c.globals.insert(n.clone(), Scheme::mono(Ty::Any));
            c.local_names.insert(n);
        }
        match im {
            Import::Module(segs) => {
                if let Some(last) = segs.last() {
                    bind(self, last.clone());
                }
            }
            Import::Names { names, .. } => {
                for n in names {
                    bind(self, n.clone());
                }
            }
            Import::Bare(n) | Import::Foreign { lang: n, .. } => bind(self, n.clone()),
            Import::Path(_) => {}
        }
    }

    /// Generalize a function signature into a polymorphic scheme: each distinct
    /// lowercase type-variable name (`a`, `k`, `value`) becomes one fresh
    /// inference variable shared across every occurrence, and all such variables
    /// are quantified.
    fn sig_scheme(&mut self, f: &FnDef) -> Scheme {
        let mut vars: HashMap<String, Ty> = HashMap::new();
        let params: Vec<Ty> = f
            .params
            .iter()
            .map(|p| p.ty.as_ref().map_or(Ty::Any, |t| self.ast_ty_v(t, &mut vars)))
            .collect();
        let ret = f.ret.as_ref().map_or(Ty::Any, |t| self.ast_ty_v(t, &mut vars));
        let ids = vars.values().filter_map(|t| if let Ty::Var(i) = t { Some(*i) } else { None }).collect();
        Scheme { vars: ids, ty: Ty::Fn(params, Box::new(ret)) }
    }

    /// Like [`ast_ty`] but maps type-variable names through `vars`, minting one
    /// fresh inference variable per distinct name.
    fn ast_ty_v(&mut self, t: &Type, vars: &mut HashMap<String, Ty>) -> Ty {
        match t {
            Type::Name(segs) if segs.len() == 1 && ty::is_type_var_name(&segs[0]) => {
                vars.entry(segs[0].clone()).or_insert_with(|| self.inf.fresh()).clone()
            }
            Type::Apply(h, args) => match &**h {
                Type::Name(segs) => {
                    Ty::Con(segs.join("."), args.iter().map(|a| self.ast_ty_v(a, vars)).collect())
                }
                _ => Ty::Any,
            },
            Type::Array(inner) => Ty::array(self.ast_ty_v(inner, vars)),
            Type::Opt(inner) => Ty::Opt(Box::new(self.ast_ty_v(inner, vars))),
            Type::Paren(inner) => self.ast_ty_v(inner, vars),
            Type::Name(_) => ast_ty(t),
        }
    }

    // ---- pass 2: check ----------------------------------------------------

    fn run(&mut self, m: &Module) {
        for (i, item) in m.items.iter().enumerate() {
            match item {
                Stmt::Fn(f) => self.check_fn(f),
                Stmt::Bind(b) if !self.type_decls.contains(&i) => self.check_bind(b),
                Stmt::Alias { value, .. } => {
                    if self.mode == Mode::Config {
                        self.check_config_effects(value);
                    }
                }
                _ => {}
            }
        }
    }

    fn check_fn(&mut self, f: &FnDef) {
        let Some(body) = &f.body else { return };
        let mut env: Env = f
            .params
            .iter()
            .map(|p| (p.name.clone(), p.ty.as_ref().map_or(Ty::Any, ast_ty)))
            .collect();
        let bty = match body {
            FnBody::Block(stmts) => self.infer_block(&mut env, stmts),
            FnBody::Expr(e) => self.infer(&mut env, e),
        };
        if let Some(ret) = &f.ret {
            let rt = ast_ty(ret);
            if let Err(e) = self.inf.unify(&bty, &rt) {
                self.diag(DiagKind::TypeMismatch, format!("in `{}`: {e}", f.name));
            }
        }
    }

    fn check_bind(&mut self, b: &Bind) {
        if self.mode == Mode::Config {
            self.check_config_effects(&b.value);
            self.check_option_root(b);
            return;
        }
        let mut env: Env = Vec::new();
        let vty = self.infer(&mut env, &b.value);
        if let Some(t) = b.tys.first() {
            let at = ast_ty(t);
            if let Err(e) = self.inf.unify(&vty, &at) {
                let name = target_name(&b.target);
                self.diag(DiagKind::TypeMismatch, format!("in binding `{name}`: {e}"));
            }
        }
    }

    // ---- config-mode checks ----------------------------------------------

    fn check_config_effects(&mut self, e: &Expr) {
        let effs = self.eff(e);
        if !effs.is_empty() {
            self.diag(
                DiagKind::EffectInConfig,
                format!("config must be pure but this uses effect(s): {}", effs.names().join(", ")),
            );
        }
    }

    fn check_option_root(&mut self, b: &Bind) {
        let Expr::Field { .. } = &b.target else { return };
        if let Some(r) = root_ident(&b.target) {
            if !NIXOS_ROOTS.contains(&r.as_str()) && !self.local_names.contains(&r) {
                self.diag(DiagKind::UnknownOption, format!("unknown NixOS option namespace `{r}`"));
            }
        }
    }

    fn eff(&self, e: &Expr) -> EffSet {
        let mut acc = EffSet::empty();
        match e {
            Expr::Call { callee, args } => {
                acc = acc.union(self.eff(callee));
                for a in args {
                    acc = acc.union(self.eff_arg(a));
                }
                match &**callee {
                    Expr::Ident(n) if IO_FNS.contains(&n.as_str()) => {
                        acc = acc.union(EffSet::of(IO))
                    }
                    Expr::Field { name, .. } if IO_METHODS.contains(&name.as_str()) => {
                        acc = acc.union(EffSet::of(IO))
                    }
                    Expr::Field { base, .. } => {
                        if let Expr::Ident(m) = &**base {
                            if m == "net" || m == "http" || m == "socket" {
                                acc = acc.union(EffSet::of(NET));
                            } else if m == "os" || m == "process" {
                                acc = acc.union(EffSet::of(OS));
                            }
                        }
                    }
                    _ => {}
                }
            }
            Expr::Try(x) => acc = self.eff(x).union(EffSet::of(EXN)),
            Expr::Fail(x) => acc = self.eff(x).union(EffSet::of(EXN)),
            Expr::Reify(x) => acc = self.eff(x),
            Expr::Field { base, .. } => acc = self.eff(base),
            Expr::Binary { lhs, rhs, .. } => acc = self.eff(lhs).union(self.eff(rhs)),
            Expr::Unary { expr, .. } => acc = self.eff(expr),
            Expr::Ternary { cond, then, els } => {
                acc = self.eff(cond).union(self.eff(then)).union(self.eff(els))
            }
            Expr::List(es) => {
                for x in es {
                    acc = acc.union(self.eff(x));
                }
            }
            Expr::Record(fs) | Expr::Ctor { fields: fs, .. } => {
                for f in fs {
                    acc = acc.union(self.eff_field(f));
                }
            }
            Expr::If { cond, then, els } => {
                acc = self.eff(cond).union(self.eff_stmts(then));
                if let Some(e) = els {
                    acc = acc.union(self.eff_stmts(e));
                }
            }
            Expr::Match { scrut, arms } => {
                acc = self.eff(scrut);
                for a in arms {
                    acc = acc.union(self.eff(&a.body));
                }
            }
            Expr::For { iter, body, .. } => acc = self.eff(iter).union(self.eff_stmts(body)),
            Expr::Lambda { body, .. } => acc = self.eff(body),
            Expr::With { base, fields } => {
                acc = self.eff(base);
                for f in fields {
                    acc = acc.union(self.eff_field(f));
                }
            }
            Expr::Assign { value, .. } => acc = self.eff(value),
            Expr::Block(stmts) => acc = self.eff_stmts(stmts),
            _ => {}
        }
        acc
    }

    fn eff_arg(&self, a: &Arg) -> EffSet {
        match a {
            Arg::Pos(e) | Arg::Named { value: e, .. } | Arg::Directive { value: e, .. } => {
                self.eff(e)
            }
        }
    }
    fn eff_field(&self, f: &Field) -> EffSet {
        match f {
            Field::Value { value, .. } | Field::Bare(value) => self.eff(value),
            _ => EffSet::empty(),
        }
    }
    fn eff_stmts(&self, stmts: &[Stmt]) -> EffSet {
        let mut acc = EffSet::empty();
        for s in stmts {
            acc = acc.union(match s {
                Stmt::Expr(e) => self.eff(e),
                Stmt::Bind(b) => self.eff(&b.value),
                Stmt::Alias { value, .. } => self.eff(value),
                _ => EffSet::empty(),
            });
        }
        acc
    }

    // ---- inference --------------------------------------------------------

    fn infer_block(&mut self, env: &mut Env, stmts: &[Stmt]) -> Ty {
        let base = env.len();
        let mut last = Ty::Unit;
        for s in stmts {
            match s {
                Stmt::Bind(b) => {
                    let vty = self.infer(env, &b.value);
                    if let Some(t) = b.tys.first() {
                        let at = ast_ty(t);
                        if let Err(e) = self.inf.unify(&vty, &at) {
                            let name = target_name(&b.target);
                            self.diag(DiagKind::TypeMismatch, format!("in `{name}`: {e}"));
                        }
                    }
                    if let Expr::Ident(n) = &b.target {
                        env.push((n.clone(), vty));
                    }
                    last = Ty::Unit;
                }
                Stmt::Expr(e) => last = self.infer(env, e),
                Stmt::Fn(f) => {
                    let params =
                        f.params.iter().map(|p| p.ty.as_ref().map_or(Ty::Any, ast_ty)).collect();
                    let ret = f.ret.as_ref().map_or(Ty::Any, ast_ty);
                    env.push((f.name.clone(), Ty::Fn(params, Box::new(ret))));
                    self.check_fn(f);
                    last = Ty::Unit;
                }
                _ => last = Ty::Unit,
            }
        }
        env.truncate(base);
        last
    }

    fn infer(&mut self, env: &mut Env, e: &Expr) -> Ty {
        match e {
            Expr::Int(_) => Ty::Int,
            Expr::Float(_) => Ty::Float,
            Expr::Bool(_) => Ty::Bool,
            Expr::Unit => Ty::Unit,
            Expr::Path(_) => Ty::Con("Path".into(), vec![]),
            Expr::Str(parts) => {
                for p in parts {
                    if let StrPart::Interp(x) = p {
                        self.infer(env, x);
                    }
                }
                Ty::Str
            }
            Expr::Ident(n) => match lookup(env, n) {
                Some(t) => t,
                None => match self.globals.get(n).cloned() {
                    Some(s) => self.inf.instantiate(&s),
                    None => Ty::Any,
                },
            },
            Expr::List(es) => {
                let el = self.inf.fresh();
                for x in es {
                    let xt = self.infer(env, x);
                    let _ = self.inf.unify(&el, &xt);
                }
                Ty::array(el)
            }
            Expr::Record(fields) => self.infer_record(env, fields),
            Expr::Ctor { name, fields } => {
                for f in fields {
                    self.infer_field(env, f);
                }
                Ty::Con(name.clone(), vec![])
            }
            Expr::Call { callee, args } => {
                // Arity: a direct call of a user function (not shadowed by a
                // local of the same name) must pass the declared number of
                // arguments unless the function is variadic.
                if let Expr::Ident(name) = &**callee
                    && lookup(env, name).is_none()
                    && let Some(&(arity, variadic)) = self.fn_arity.get(name)
                    && !variadic
                    && args.len() != arity
                {
                    self.diag(
                        DiagKind::TypeMismatch,
                        format!("call to `{name}` expects {arity} argument(s), got {}", args.len()),
                    );
                }
                let ct = self.infer(env, callee);
                let ats: Vec<Ty> = args.iter().map(|a| self.infer_arg(env, a)).collect();
                match self.inf.resolve(&ct) {
                    Ty::Fn(params, ret) => {
                        // A concrete/concrete clash between a declared parameter
                        // and its argument is a real error (`Any`/vars never
                        // clash, so unknown-stdlib calls stay silent).
                        for (i, (p, a)) in params.iter().zip(&ats).enumerate() {
                            if let Err(e) = self.inf.unify(p, a) {
                                let name = target_name(callee);
                                self.diag(
                                    DiagKind::TypeMismatch,
                                    format!("in call to `{name}` (argument {}): {e}", i + 1),
                                );
                            }
                        }
                        *ret
                    }
                    _ => Ty::Any,
                }
            }
            Expr::Field { base, name } => {
                let bt = self.infer(env, base);
                self.field_ty(&bt, name)
            }
            Expr::Unary { expr, .. } => self.infer(env, expr),
            Expr::Binary { op, lhs, rhs } => {
                let lt = self.infer(env, lhs);
                let rt = self.infer(env, rhs);
                self.binary_ty(*op, lt, rt)
            }
            Expr::Ternary { cond, then, els } => {
                let ct = self.infer(env, cond);
                let _ = self.inf.unify(&ct, &Ty::Bool);
                let tt = self.infer(env, then);
                let et = self.infer(env, els);
                if let Err(e) = self.inf.unify(&tt, &et) {
                    self.diag(DiagKind::TypeMismatch, format!("ternary branches disagree: {e}"));
                }
                tt
            }
            Expr::If { cond, then, els } => {
                let ct = self.infer(env, cond);
                let _ = self.inf.unify(&ct, &Ty::Bool);
                let tt = self.infer_block(env, then);
                if let Some(e) = els {
                    let et = self.infer_block(env, e);
                    if let Err(err) = self.inf.unify(&tt, &et) {
                        self.diag(
                            DiagKind::TypeMismatch,
                            format!("`if` branches disagree: {err}"),
                        );
                    }
                    tt
                } else {
                    Ty::Unit
                }
            }
            Expr::Match { scrut, arms } => {
                let st = self.infer(env, scrut);
                let result = self.inf.fresh();
                for a in arms {
                    let base = env.len();
                    self.bind_pattern(env, &a.pat, &st);
                    if let Some(g) = &a.guard {
                        let gt = self.infer(env, g);
                        let _ = self.inf.unify(&gt, &Ty::Bool);
                    }
                    let bt = self.infer(env, &a.body);
                    let _ = self.inf.unify(&result, &bt);
                    env.truncate(base);
                }
                self.check_exhaustive(&st, arms);
                result
            }
            Expr::For { pat, iter, body } => {
                let it = self.infer(env, iter);
                let el = match self.inf.resolve(&it) {
                    Ty::Con(n, args) if n == "Array" && args.len() == 1 => args[0].clone(),
                    _ => Ty::Any,
                };
                let base = env.len();
                self.bind_pattern(env, pat, &el);
                self.infer_block(env, body);
                env.truncate(base);
                Ty::Unit
            }
            Expr::Lambda { params, body } => {
                let base = env.len();
                let pts: Vec<Ty> = params
                    .iter()
                    .map(|p| {
                        let t = p.ty.as_ref().map_or_else(|| self.inf.fresh(), ast_ty);
                        env.push((p.name.clone(), t.clone()));
                        t
                    })
                    .collect();
                let bt = self.infer(env, body);
                env.truncate(base);
                Ty::Fn(pts, Box::new(bt))
            }
            Expr::With { base, fields } => {
                let bt = self.infer(env, base);
                for f in fields {
                    self.infer_field(env, f);
                }
                bt
            }
            Expr::Try(x) => {
                let xt = self.infer(env, x);
                match self.inf.resolve(&xt) {
                    Ty::Opt(t) => *t,
                    t => t,
                }
            }
            Expr::Fail(_) => self.inf.fresh(),
            Expr::Reify(x) => {
                self.infer(env, x);
                Ty::Any
            }
            Expr::Assign { target, value } => {
                self.infer(env, target);
                self.infer(env, value);
                Ty::Unit
            }
            Expr::Block(stmts) => self.infer_block(env, stmts),
        }
    }

    fn infer_arg(&mut self, env: &mut Env, a: &Arg) -> Ty {
        match a {
            Arg::Pos(e) | Arg::Named { value: e, .. } | Arg::Directive { value: e, .. } => {
                self.infer(env, e)
            }
        }
    }

    fn infer_field(&mut self, env: &mut Env, f: &Field) -> (String, Ty) {
        match f {
            Field::Value { name, value } => (name.clone(), self.infer(env, value)),
            Field::Shorthand(n) => (n.clone(), lookup(env, n).unwrap_or(Ty::Any)),
            Field::Bare(e) => (String::new(), self.infer(env, e)),
            Field::Type { name, ty } => (name.clone(), ast_ty(ty)),
        }
    }

    fn infer_record(&mut self, env: &mut Env, fields: &[Field]) -> Ty {
        let mut map = BTreeMap::new();
        for f in fields {
            let (k, t) = self.infer_field(env, f);
            if !k.is_empty() {
                map.insert(k, t);
            }
        }
        Ty::Rec { fields: map, open: true }
    }

    fn field_ty(&self, base: &Ty, name: &str) -> Ty {
        match self.inf.resolve(base) {
            Ty::Rec { fields, .. } => fields.get(name).cloned().unwrap_or(Ty::Any),
            Ty::Con(n, _) => {
                self.records.get(&n).and_then(|fs| fs.get(name).cloned()).unwrap_or(Ty::Any)
            }
            _ => Ty::Any,
        }
    }

    fn binary_ty(&mut self, op: BinOp, lt: Ty, rt: Ty) -> Ty {
        use BinOp::*;
        match op {
            Eq | Ne | Lt | Gt | Le | Ge | And | Or => Ty::Bool,
            Add | Sub | Mul | Div | Concat => {
                let _ = self.inf.unify(&lt, &rt);
                self.inf.resolve(&lt)
            }
            Union | Pipe => Ty::Any,
        }
    }

    fn bind_pattern(&mut self, env: &mut Env, p: &Pattern, scrut: &Ty) {
        match p {
            Pattern::Bind(n) => {
                if self.is_variant(n) {
                    return;
                }
                env.push((n.clone(), scrut.clone()));
            }
            Pattern::Ctor { args, .. } => {
                for a in args {
                    self.bind_pattern(env, a, &Ty::Any);
                }
            }
            Pattern::Record(fields) => {
                for (n, sub) in fields {
                    match sub {
                        Some(sp) => self.bind_pattern(env, sp, &Ty::Any),
                        None => env.push((n.clone(), Ty::Any)),
                    }
                }
            }
            Pattern::List { elems, rest } => {
                let el = match self.inf.resolve(scrut) {
                    Ty::Con(n, args) if n == "Array" && args.len() == 1 => args[0].clone(),
                    _ => Ty::Any,
                };
                for e in elems {
                    self.bind_pattern(env, e, &el);
                }
                if let Some(r) = rest {
                    self.bind_pattern(env, r, &Ty::array(el));
                }
            }
            Pattern::Or(alts) => {
                if let Some(first) = alts.first() {
                    self.bind_pattern(env, first, scrut);
                }
            }
            _ => {}
        }
    }

    fn is_variant(&self, name: &str) -> bool {
        self.sums.values().any(|vs| vs.iter().any(|v| v == name))
    }

    fn check_exhaustive(&mut self, scrut: &Ty, arms: &[Arm]) {
        let Ty::Con(name, _) = self.inf.resolve(scrut) else { return };
        let Some(variants) = self.sums.get(&name).cloned() else { return };
        let mut covered: HashSet<String> = HashSet::new();
        let mut catchall = false;
        for a in arms {
            if a.guard.is_some() {
                continue;
            }
            cover(&a.pat, &variants, &mut covered, &mut catchall);
        }
        if !catchall {
            let missing: Vec<_> =
                variants.iter().filter(|v| !covered.contains(*v)).cloned().collect();
            if !missing.is_empty() {
                self.diag(
                    DiagKind::NonExhaustive,
                    format!("match on `{name}` is not exhaustive; missing: {}", missing.join(", ")),
                );
            }
        }
    }
}

fn cover(p: &Pattern, variants: &[String], covered: &mut HashSet<String>, catchall: &mut bool) {
    match p {
        Pattern::Wild => *catchall = true,
        Pattern::Bind(n) => {
            if variants.iter().any(|v| v == n) {
                covered.insert(n.clone());
            } else {
                *catchall = true;
            }
        }
        Pattern::Ctor { name, .. } => {
            covered.insert(name.clone());
        }
        Pattern::Or(alts) => {
            for a in alts {
                cover(a, variants, covered, catchall);
            }
        }
        _ => {}
    }
}

// ---- helpers -------------------------------------------------------------

fn lookup(env: &Env, name: &str) -> Option<Ty> {
    env.iter().rev().find(|(n, _)| n == name).map(|(_, t)| t.clone())
}

fn ast_ty(t: &Type) -> Ty {
    match t {
        Type::Name(segs) => {
            let n = segs.join(".");
            match n.as_str() {
                "int" => Ty::Int,
                "float" => Ty::Float,
                "str" => Ty::Str,
                "bool" => Ty::Bool,
                "bytes" => Ty::Bytes,
                "unit" | "()" => Ty::Unit,
                // Outside a generalizing signature, an un-bound type variable is
                // treated gradually (`any`) rather than as a nominal type.
                _ if segs.len() == 1 && ty::is_type_var_name(&n) => Ty::Any,
                _ => Ty::Con(n, vec![]),
            }
        }
        Type::Apply(h, args) => match &**h {
            Type::Name(segs) => Ty::Con(segs.join("."), args.iter().map(ast_ty).collect()),
            _ => Ty::Any,
        },
        Type::Array(t) => Ty::array(ast_ty(t)),
        Type::Opt(t) => Ty::Opt(Box::new(ast_ty(t))),
        Type::Paren(t) => ast_ty(t),
    }
}

/// `A | B | C` where every leaf is an identifier → sum variant list.
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

/// A record literal made entirely of `name: Type` fields → a record type decl.
fn record_type(e: &Expr) -> Option<BTreeMap<String, Ty>> {
    let Expr::Record(fields) = e else { return None };
    if fields.is_empty() || !fields.iter().all(|f| matches!(f, Field::Type { .. })) {
        return None;
    }
    let mut map = BTreeMap::new();
    for f in fields {
        if let Field::Type { name, ty } = f {
            map.insert(name.clone(), ast_ty(ty));
        }
    }
    Some(map)
}

fn target_name(e: &Expr) -> String {
    match e {
        Expr::Ident(n) => n.clone(),
        Expr::Field { base, name } => format!("{}.{name}", target_name(base)),
        _ => "?".into(),
    }
}

fn root_ident(e: &Expr) -> Option<String> {
    match e {
        Expr::Ident(n) => Some(n.clone()),
        Expr::Field { base, .. } => root_ident(base),
        _ => None,
    }
}
