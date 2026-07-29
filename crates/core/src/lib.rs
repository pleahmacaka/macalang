//! maca-core: name/type/effect checking over the parser's AST.
//!
//! Pragmatic gradual checker (see docs/SPEC.md): unification-based inference
//! with an `any` escape hatch for unknown stdlib/foreign values, made *strict*
//! on the four diagnostics the acceptance requires:
//!   * type mismatch (annotated return / `let x: T =`),
//!   * non-exhaustive `match` on a nominal sum,
//!   * effects used in config mode (must be pure `<>`),
//!   * unknown NixOS option root in config mode.
//!
//! Function signatures are generalized into rank-1 type schemes and
//! instantiated per call site, so generics like `id(x: a) -> a` are usable at
//! many types. Inference over un-annotated bindings stays monomorphic, and
//! effect rows unify by set rather than by row variable — both bought
//! deliberately, because the strictness this checker owes its users is on the
//! diagnostics above, and rank-1 plus gradual `any` reaches them.

mod ty;

use maca_parser::ast::*;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use ty::{ASYNC, EXN, EffSet, IO, Infer, NET, OS, Scheme, Ty};

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
    /// Reassignment of a constant. A bare `x = 1` binds a constant; only a
    /// `let x = 1` binding is mutable, so a later `x = 2` on a bare binding is
    /// an error.
    Immutable,
    /// A direct call to a name that is not defined anywhere — no local, no
    /// user function, no import/alias, no builtin. Caught here so it surfaces
    /// as a clean diagnostic instead of a broken-C link error at codegen.
    UndefinedName,
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
///
/// Public because the freestanding-target check refuses exactly these: the
/// reason they are impure is that they write to a console, and a bare-metal
/// image has none. It kept its own copy, which was missing `panic`.
pub const IO_FNS: &[&str] = &[
    "info", "warn", "err", "debug", "notice", "crit", "alert", "emerg", "panic", "print", "input",
];
/// UFCS method names treated as `io` (file/stdio side effects).
const IO_METHODS: &[&str] = &["read", "write", "exists", "remove", "append", "create"];
/// Prelude functions that carry the `async` effect (suspension points).
const ASYNC_FNS: &[&str] = &["sleep_ms"];

/// Every method a `str` receiver accepts. Not a wish-list: `crates/core/tests`
/// compiles a program using all of them, so a name here that the back end can't
/// lower fails the build — and one it *can* lower that is missing here would be
/// rejected as a typo. Note the absences: `str` has no `get` and no `join`.
pub const STR_METHODS: &[&str] = &[
    "length",
    "split",
    "trim",
    "upper",
    "lower",
    "contains",
    "starts_with",
    "ends_with",
    "replace",
    "substr",
    "slice",
    "index_of",
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

/// Every method a `Map str V` receiver accepts, gated the same way.
pub const MAP_METHODS: &[&str] = &["set", "get", "has", "remove", "keys", "length"];

/// Every method a `T[]` receiver accepts, gated the same way.
///
/// Some are refined further by the element type — `join` wants `str[]`, `sum`
/// wants numbers — and the back end reports that. This list is about catching a
/// misspelt name, which is a different job from checking the element type.
pub const LIST_METHODS: &[&str] = &[
    "map", "filter", "reduce", "fold", "sort", "reverse", "push", "pop", "slice", "contains",
    "index_of", "sum", "min", "max", "first", "last", "get", "length", "parallel", "join",
];

/// Top-level NixOS / home-manager option namespaces we recognize.
///
/// Public because the language server completes from it and the MCP server
/// answers `maca.options` from it. Each kept its own copy until the three
/// disagreed: the checker accepted `virtualisation.docker.enable` while the
/// LSP would never offer `virtualisation`, and neither of the other two knew
/// about `imports`.
pub const NIXOS_ROOTS: &[&str] = &[
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
    /// sum variant -> its payload field types (empty for a nullary variant)
    variant_payloads: HashMap<String, Vec<Ty>>,
    /// User-declared function name -> (fixed param count, is variadic). Used to
    /// catch call-arity mistakes; variadic functions are exempt.
    fn_arity: HashMap<String, (usize, bool)>,
    type_decls: HashSet<usize>, // item indices that are type declarations
    local_names: HashSet<String>,
    /// Per-function mutability of local names: `let x` → true, bare `x = e` →
    /// false (a constant). Reassigning a `false` entry is an error. Cleared and
    /// restored around each function body.
    mut_of: HashMap<String, bool>,
    /// The program pulls in Rust via `import rust` (a path or a raw block whose
    /// contents the checker can't see), so unknown lowercase calls are treated as
    /// gradual foreign items rather than `UndefinedName` (§2.4).
    gradual_foreign: bool,
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
            variant_payloads: HashMap::new(),
            fn_arity: HashMap::new(),
            type_decls: HashSet::new(),
            local_names: HashSet::new(),
            mut_of: HashMap::new(),
            gradual_foreign: false,
            diags: Vec::new(),
        }
    }

    fn diag(&mut self, kind: DiagKind, msg: impl Into<String>) {
        self.diags.push(Diagnostic {
            kind,
            msg: msg.into(),
        });
    }

    /// A keyword Maca doesn't have, used as if it did.
    ///
    /// `return x` parses as the identifier `return` next to `x` and reaches the
    /// C backend, which mangles the C keyword and emits `return_mc` — so the
    /// first thing a newcomer sees is `'return_mc' undeclared`, from a compiler
    /// they didn't invoke, about a file they didn't write. These are the words
    /// people actually reach for; each gets told what Maca does instead.
    ///
    /// Only *undefined* names land here, so a program that legitimately binds
    /// `type` or `func` keeps working.
    fn phantom_keyword(&mut self, name: &str) {
        if self.mode != Mode::Program || self.gradual_foreign {
            return;
        }
        let hint = match name {
            "return" => "Maca has no `return` — a function's last expression is its value",
            "let" | "var" => "Maca has no `let`/`var` — write `x = e`, or `const x = e`",
            "fn" | "func" | "def" => "Maca has no `fn` — write `name(arg: T) -> R { … }` or `=> e`",
            "type" => "Maca has no `type` — write `Name = { field: T }` or `Name = A | B`",
            "async" | "await_" => {
                "Maca has no `async` — async is an inferred effect; use `spawn`/`await`"
            }
            "null" | "nil" | "None" | "undefined" => {
                "Maca has no null — use a sum type with an empty variant"
            }
            "true_" | "false_" => return,
            _ => return,
        };
        self.diag(DiagKind::UndefinedName, format!("`{name}`: {hint}"));
    }

    // ---- pass 1: collect declarations ------------------------------------

    fn collect(&mut self, m: &Module) {
        for f in IO_FNS {
            self.globals.insert(
                (*f).into(),
                Scheme::mono(Ty::Fn(vec![Ty::Any], Box::new(Ty::Unit))),
            );
        }
        for (n, t) in [
            ("int", Ty::Fn(vec![Ty::Any], Box::new(Ty::Int))),
            ("float", Ty::Fn(vec![Ty::Any], Box::new(Ty::Float))),
            ("str", Ty::Fn(vec![Ty::Any], Box::new(Ty::Str))),
            ("len", Ty::Fn(vec![Ty::Any], Box::new(Ty::Int))),
            // A byte and the one-character string holding it. Maca strings are
            // bytes, so these are the pair every decoder needs: percent
            // escapes, base64, a character class written as arithmetic.
            ("chr", Ty::Fn(vec![Ty::Int], Box::new(Ty::Str))),
            ("real_path", Ty::Fn(vec![Ty::Str], Box::new(Ty::Str))),
            // whether stdout is a terminal — what colour output has to ask before
            // writing an escape code into a file somebody will read later
            ("is_tty", Ty::Fn(vec![], Box::new(Ty::Bool))),
            ("ord", Ty::Fn(vec![Ty::Str], Box::new(Ty::Int))),
            ("input", Ty::Fn(vec![], Box::new(Ty::Str))),
            // `sleep_ms(ms)` — an async suspension point (the async effect is
            // added in `eff`); yields nothing.
            ("sleep_ms", Ty::Fn(vec![Ty::Int], Box::new(Ty::Unit))),
            // math prelude (always available, no import)
            ("sqrt", Ty::Fn(vec![Ty::Any], Box::new(Ty::Float))),
            ("floor", Ty::Fn(vec![Ty::Any], Box::new(Ty::Float))),
            ("ceil", Ty::Fn(vec![Ty::Any], Box::new(Ty::Float))),
            ("round", Ty::Fn(vec![Ty::Any], Box::new(Ty::Float))),
            ("sin", Ty::Fn(vec![Ty::Any], Box::new(Ty::Float))),
            ("cos", Ty::Fn(vec![Ty::Any], Box::new(Ty::Float))),
            ("tan", Ty::Fn(vec![Ty::Any], Box::new(Ty::Float))),
            ("log", Ty::Fn(vec![Ty::Any], Box::new(Ty::Float))),
            ("exp", Ty::Fn(vec![Ty::Any], Box::new(Ty::Float))),
            ("pow", Ty::Fn(vec![Ty::Any, Ty::Any], Box::new(Ty::Float))),
            ("abs", Ty::Fn(vec![Ty::Any], Box::new(Ty::Any))),
            ("min", Ty::Fn(vec![Ty::Any, Ty::Any], Box::new(Ty::Any))),
            ("max", Ty::Fn(vec![Ty::Any, Ty::Any], Box::new(Ty::Any))),
            (
                "clamp",
                Ty::Fn(vec![Ty::Any, Ty::Any, Ty::Any], Box::new(Ty::Any)),
            ),
            ("sign", Ty::Fn(vec![Ty::Any], Box::new(Ty::Int))),
            ("gcd", Ty::Fn(vec![Ty::Int, Ty::Int], Box::new(Ty::Int))),
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
                    self.fn_arity
                        .insert(f.name.clone(), (f.params.len(), variadic));
                }
                Stmt::Bind(b) => {
                    if let Expr::Ident(name) = &b.target {
                        self.local_names.insert(name.clone());
                        if let Some(vars) = sum_variants(&b.value) {
                            self.sums.insert(
                                name.clone(),
                                vars.iter().map(|(n, _)| n.clone()).collect(),
                            );
                            let sum = Ty::Con(name.clone(), vec![]);
                            for (v, ptys) in vars {
                                let payloads: Vec<Ty> = ptys.iter().map(ast_ty).collect();
                                // a payload variant is a constructor *function*;
                                // a nullary variant is just a value of the sum
                                let scheme = if payloads.is_empty() {
                                    Scheme::mono(sum.clone())
                                } else {
                                    Scheme::mono(Ty::Fn(payloads.clone(), Box::new(sum.clone())))
                                };
                                self.globals.insert(v.clone(), scheme);
                                self.variant_payloads.insert(v, payloads);
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
            Import::Foreign { lang, .. } => {
                // `import rust` brings in items the checker can't see (a crate
                // path or a raw block), so relax undefined-name checking.
                if lang == "rust" {
                    self.gradual_foreign = true;
                }
                bind(self, lang.clone());
            }
            Import::Bare(n) => bind(self, n.clone()),
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
            .map(|p| {
                p.ty.as_ref()
                    .map_or(Ty::Any, |t| self.ast_ty_v(t, &mut vars))
            })
            .collect();
        let ret = f
            .ret
            .as_ref()
            .map_or(Ty::Any, |t| self.ast_ty_v(t, &mut vars));
        let ids = vars
            .values()
            .filter_map(|t| if let Ty::Var(i) = t { Some(*i) } else { None })
            .collect();
        Scheme {
            vars: ids,
            ty: Ty::Fn(params, Box::new(ret)),
        }
    }

    /// Like [`ast_ty`] but maps type-variable names through `vars`, minting one
    /// fresh inference variable per distinct name.
    fn ast_ty_v(&mut self, t: &Type, vars: &mut HashMap<String, Ty>) -> Ty {
        match t {
            Type::Name(segs) if segs.len() == 1 && ty::is_type_var_name(&segs[0]) => vars
                .entry(segs[0].clone())
                .or_insert_with(|| self.inf.fresh())
                .clone(),
            Type::Apply(h, args) => match &**h {
                Type::Name(segs) => Ty::Con(
                    segs.join("."),
                    args.iter().map(|a| self.ast_ty_v(a, vars)).collect(),
                ),
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
                Stmt::Alias { value, .. } if self.mode == Mode::Config => {
                    self.check_config_effects(value);
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
        // fresh mutability scope for this body; params are exempt (mutable).
        let saved_mut = std::mem::take(&mut self.mut_of);
        for p in &f.params {
            self.mut_of.insert(p.name.clone(), true);
        }
        let bty = match body {
            FnBody::Block(stmts) => self.infer_block(&mut env, stmts),
            FnBody::Expr(e) => self.infer(&mut env, e),
        };
        self.mut_of = saved_mut;
        if let Some(ret) = &f.ret {
            let rt = self.relax_ann(ast_ty(ret));
            if let Err(e) = self.inf.unify(&bty, &rt) {
                self.diag(DiagKind::TypeMismatch, format!("in `{}`: {e}", f.name));
            }
        }
    }

    /// Relax an *annotation* type: a bare, capitalized nominal that isn't a
    /// declared record/sum is a foreign / interop type (a Java interface, a Nix
    /// value, …) — treat it gradually as `any` rather than a strict nominal, so
    /// `Mod : ModInitializer = { … }` type-checks.
    fn relax_ann(&self, t: Ty) -> Ty {
        match &t {
            Ty::Con(n, args)
                if args.is_empty()
                    && !self.records.contains_key(n)
                    && !self.sums.contains_key(n)
                    && n.chars().next().is_some_and(|c| c.is_ascii_uppercase()) =>
            {
                Ty::Any
            }
            _ => t,
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
            let at = self.relax_ann(ast_ty(t));
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
                format!(
                    "config must be pure but this uses effect(s): {}",
                    effs.names().join(", ")
                ),
            );
        }
    }

    fn check_option_root(&mut self, b: &Bind) {
        let Expr::Field { .. } = &b.target else {
            return;
        };
        if let Some(r) = root_ident(&b.target)
            && !NIXOS_ROOTS.contains(&r.as_str())
            && !self.local_names.contains(&r)
        {
            self.diag(
                DiagKind::UnknownOption,
                format!("unknown NixOS option namespace `{r}`"),
            );
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
                    Expr::Ident(n) if ASYNC_FNS.contains(&n.as_str()) => {
                        acc = acc.union(EffSet::of(ASYNC))
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
            // `reify`/`try` catches failures, so it discharges the `exn` effect
            Expr::Reify(x) => acc = EffSet(self.eff(x).0 & !EXN),
            // colorblind async: `await`/`spawn` add the `async` effect (inferred,
            // never written — there is no `async` keyword).
            Expr::Await(x) | Expr::Spawn(x) => acc = self.eff(x).union(EffSet::of(ASYNC)),
            Expr::Field { base, .. } => acc = self.eff(base),
            Expr::Index { base, index } => acc = self.eff(base).union(self.eff(index)),
            Expr::Range { lo, hi } => acc = self.eff(lo).union(self.eff(hi)),
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
            Expr::While { cond, body } => acc = self.eff(cond).union(self.eff_stmts(body)),
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
                    // Mutability: a bare lowercase `x = e` binds a mutable local;
                    // `const x`, `x = e as const`, or a Capitalized name binds a
                    // constant (`is_const`). Reassigning a constant is an error;
                    // reassigning a mutable is fine. A bare reassignment of a
                    // global is left to the global.
                    if let Expr::Ident(n) = &b.target {
                        match self.mut_of.get(n).copied() {
                            Some(false) => self.diag(
                                DiagKind::Immutable,
                                format!("cannot reassign constant `{n}` — declare it mutable with `{n} = …` (no `const`)"),
                            ),
                            Some(true) => {
                                if b.is_const {
                                    self.mut_of.insert(n.clone(), false); // re-bound as const
                                }
                            }
                            None => {
                                if b.is_const || !self.globals.contains_key(n) {
                                    self.mut_of.insert(n.clone(), !b.is_const);
                                }
                            }
                        }
                    }
                    let mut vty = self.infer(env, &b.value);
                    if let Some(t) = b.tys.first() {
                        let at = self.relax_ann(ast_ty(t));
                        if let Err(e) = self.inf.unify(&vty, &at) {
                            let name = target_name(&b.target);
                            self.diag(DiagKind::TypeMismatch, format!("in `{name}`: {e}"));
                        }
                        // The annotation is what the binding *is*. Keeping the
                        // inferred type instead loses it whenever the value is
                        // gradual — `counts: Map str int = map()` would stay
                        // `any`, and a misspelt method on it would sail past
                        // the closed-method-set check to the linker.
                        vty = at;
                    }
                    if let Expr::Ident(n) = &b.target {
                        env.push((n.clone(), vty));
                    }
                    last = Ty::Unit;
                }
                Stmt::Expr(e) => last = self.infer(env, e),
                Stmt::Fn(f) => {
                    let params = f
                        .params
                        .iter()
                        .map(|p| p.ty.as_ref().map_or(Ty::Any, ast_ty))
                        .collect();
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
                    None => {
                        self.phantom_keyword(n);
                        Ty::Any
                    }
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
                self.check_record_fields(name, fields);
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
                        format!(
                            "call to `{name}` expects {arity} argument(s), got {}",
                            args.len()
                        ),
                    );
                }
                // Undefined direct call: a bare lowercase `foo(...)` that is
                // neither a local, a user function, an import/alias, nor a
                // builtin resolves to nothing the native backend can emit — it
                // would become a broken-C call. Flag it here as a clean error.
                // (Capitalized names are constructors; UFCS `x.m(...)` and calls
                // through a value stay gradual.)
                if self.mode == Mode::Program
                    && !self.gradual_foreign
                    && let Expr::Ident(name) = &**callee
                    && name
                        .chars()
                        .next()
                        .is_some_and(|c| c.is_lowercase() || c == '_')
                    && lookup(env, name).is_none()
                    && !self.globals.contains_key(name)
                    && !maca_parser::is_backend_intrinsic(name)
                {
                    self.diag(
                        DiagKind::UndefinedName,
                        format!("call to undefined function `{name}`"),
                    );
                }
                // UFCS on a known primitive receiver: a method outside the
                // closed set is a typo, not gradual foreign code.
                if let Expr::Field { base, name } = &**callee {
                    let bt = self.infer(env, base);
                    self.check_method(&bt, name);
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
            Expr::Index { base, index } => {
                let bt = self.infer(env, base);
                let it = self.infer(env, index);
                let _ = self.inf.unify(&it, &Ty::Int);
                match self.inf.resolve(&bt) {
                    Ty::Con(n, args) if n == "Array" && args.len() == 1 => args[0].clone(),
                    // `str[i]` yields a single-character `str`; unknown/gradual
                    // bases stay `any` so foreign collections aren't rejected
                    Ty::Con(n, _) if n == "str" || n == "bytes" => Ty::Con(n, vec![]),
                    _ => Ty::Any,
                }
            }
            Expr::Range { lo, hi } => {
                let lt = self.infer(env, lo);
                let rt = self.infer(env, hi);
                if let Err(e) = self.inf.unify(&lt, &Ty::Int) {
                    self.diag(
                        DiagKind::TypeMismatch,
                        format!("range start must be `int`: {e}"),
                    );
                }
                if let Err(e) = self.inf.unify(&rt, &Ty::Int) {
                    self.diag(
                        DiagKind::TypeMismatch,
                        format!("range end must be `int`: {e}"),
                    );
                }
                Ty::array(Ty::Int)
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
                    self.diag(
                        DiagKind::TypeMismatch,
                        format!("ternary branches disagree: {e}"),
                    );
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
            Expr::While { cond, body } => {
                let ct = self.infer(env, cond);
                if let Err(e) = self.inf.unify(&ct, &Ty::Bool) {
                    self.diag(DiagKind::TypeMismatch, format!("`while` condition: {e}"));
                }
                let base = env.len();
                self.infer_block(env, body);
                env.truncate(base);
                Ty::Unit
            }
            Expr::Break | Expr::Continue => Ty::Unit,
            Expr::Lambda { params, body, .. } => {
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
            // `spawn e : Future a` where `e : a`; `await (Future a) : a`. No
            // `async` keyword — the async effect is inferred by `eff` above.
            Expr::Spawn(x) => {
                let a = self.infer(env, x);
                Ty::Con("Future".into(), vec![a])
            }
            Expr::Await(x) => {
                let t = self.infer(env, x);
                match self.inf.resolve(&t) {
                    Ty::Con(n, args) if n == "Future" && args.len() == 1 => args[0].clone(),
                    // gradual: awaiting an unknown/foreign value yields `any`
                    _ => Ty::Any,
                }
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
        Ty::Rec {
            fields: map,
            open: true,
        }
    }

    /// A record literal has to name every field the record declares, and no
    /// field it doesn't.
    ///
    /// A missing one used to be silently zero — `""` for a `str`, `0` for an
    /// `int` — so a page whose copy lives in a record shipped a link with no
    /// text and a heading with no words, compiling clean the whole way. A
    /// *misspelt* one is worse: the value goes nowhere and the field it was
    /// meant for stays empty.
    ///
    /// `base with { f = v }` is the update form and is deliberately partial;
    /// only a construction is checked.
    fn check_record_fields(&mut self, name: &str, fields: &[Field]) {
        if self.mode != Mode::Program {
            return;
        }
        let Some(declared) = self.records.get(name).cloned() else {
            return;
        };
        // `Field::Bare` is a positional child in the UI syntax, not a named
        // field; a literal carrying one isn't a record construction.
        if fields.iter().any(|f| matches!(f, Field::Bare(_))) {
            return;
        }
        let given: BTreeSet<&str> = fields
            .iter()
            .filter_map(|f| match f {
                Field::Value { name, .. } | Field::Type { name, .. } => Some(name.as_str()),
                Field::Shorthand(n) => Some(n.as_str()),
                Field::Bare(_) => None,
            })
            .collect();

        let missing: Vec<&str> = declared
            .keys()
            .map(String::as_str)
            .filter(|k| !given.contains(k))
            .collect();
        if !missing.is_empty() {
            self.diag(
                DiagKind::TypeMismatch,
                format!("`{name}` is missing field(s): {}", missing.join(", ")),
            );
        }

        let unknown: Vec<&str> = given
            .iter()
            .copied()
            .filter(|k| !declared.contains_key(*k))
            .collect();
        for k in unknown {
            let near = nearest(k, &declared.keys().map(String::as_str).collect::<Vec<_>>());
            let hint = near.map_or(String::new(), |n| format!("; did you mean `{n}`?"));
            self.diag(
                DiagKind::TypeMismatch,
                format!("`{name}` has no field `{k}`{hint}"),
            );
        }
    }

    /// A UFCS method call on a receiver whose type we actually know.
    ///
    /// Method calls are gradual on purpose — `any` receivers reach foreign and
    /// unknown-stdlib code, and rejecting those would make the escape hatch
    /// useless. But when the receiver is a `str` or a `T[]`, the set of methods
    /// is closed and a name outside it is a typo. It used to sail through the
    /// checker and die in the linker:
    ///
    /// ```text
    /// undefined reference to `slice'
    /// ```
    ///
    /// which is a message about a C symbol, for a Maca programmer who wrote
    /// `s.slice(1, 3)` and wanted `substr`.
    fn check_method(&mut self, recv: &Ty, name: &str) {
        if self.mode != Mode::Program || self.gradual_foreign {
            return;
        }
        // a user-defined function of the same name is a legitimate UFCS target
        if self.globals.contains_key(name) {
            return;
        }
        let (known, what) = match self.inf.resolve(recv) {
            Ty::Str => (STR_METHODS, "str"),
            Ty::Con(n, _) if n == "Array" => (LIST_METHODS, "list"),
            Ty::Con(n, _) if n == "Map" => (MAP_METHODS, "map"),
            _ => return,
        };
        if known.contains(&name) {
            return;
        }
        let near = nearest(name, known);
        self.diag(
            DiagKind::UndefinedName,
            match near {
                Some(alt) => format!("`{what}` has no method `{name}` — did you mean `{alt}`?"),
                None => format!("`{what}` has no method `{name}`"),
            },
        );
    }

    fn field_ty(&self, base: &Ty, name: &str) -> Ty {
        match self.inf.resolve(base) {
            Ty::Rec { fields, .. } => fields.get(name).cloned().unwrap_or(Ty::Any),
            Ty::Con(n, _) => self
                .records
                .get(&n)
                .and_then(|fs| fs.get(name).cloned())
                .unwrap_or(Ty::Any),
            _ => Ty::Any,
        }
    }

    fn binary_ty(&mut self, op: BinOp, lt: Ty, rt: Ty) -> Ty {
        use BinOp::*;
        // Operator overloading: on a user type (record/sum), an operator resolves
        // to a same-named function (`a + b` → `add`), taking its return type.
        if let Ty::Con(n, _) = self.inf.resolve(&lt)
            && (self.records.contains_key(&n) || self.sums.contains_key(&n))
            && let Some(name) = overload_fn_name(op)
            && let Some(scheme) = self.globals.get(name).cloned()
        {
            let t = self.inf.instantiate(&scheme);
            if let Ty::Fn(_, ret) = self.inf.resolve(&t) {
                return *ret;
            }
        }
        match op {
            Eq | Ne | Lt | Gt | Le | Ge | And | Or => Ty::Bool,
            Add | Sub | Mul | Div | Concat => {
                let _ = self.inf.unify(&lt, &rt);
                self.inf.resolve(&lt)
            }
            // integer-only operators
            Mod | Shl | Shr => {
                let _ = self.inf.unify(&lt, &Ty::Int);
                let _ = self.inf.unify(&rt, &Ty::Int);
                Ty::Int
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
                // a pattern-bound name (loop var, match capture) is exempt from
                // the constant rule — treat it as mutable.
                self.mut_of.insert(n.clone(), true);
            }
            Pattern::Ctor { name, args } => {
                // bind each payload sub-pattern at the variant's declared type
                let tys = self.variant_payloads.get(name).cloned().unwrap_or_default();
                for (i, a) in args.iter().enumerate() {
                    self.bind_pattern(env, a, tys.get(i).unwrap_or(&Ty::Any));
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
        let Ty::Con(name, _) = self.inf.resolve(scrut) else {
            return;
        };
        let Some(variants) = self.sums.get(&name).cloned() else {
            return;
        };
        let mut covered: HashSet<String> = HashSet::new();
        let mut catchall = false;
        for a in arms {
            if a.guard.is_some() {
                continue;
            }
            cover(&a.pat, &variants, &mut covered, &mut catchall);
        }
        if !catchall {
            let missing: Vec<_> = variants
                .iter()
                .filter(|v| !covered.contains(*v))
                .cloned()
                .collect();
            if !missing.is_empty() {
                self.diag(
                    DiagKind::NonExhaustive,
                    format!(
                        "match on `{name}` is not exhaustive; missing: {}",
                        missing.join(", ")
                    ),
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

/// Operator → overload function name for user types (mirror of the C backend).
fn overload_fn_name(op: BinOp) -> Option<&'static str> {
    use BinOp::*;
    Some(match op {
        Add => "add",
        Sub => "sub",
        Mul => "mul",
        Div => "div",
        Concat => "concat",
        Eq => "eq",
        Ne => "ne",
        Lt => "lt",
        Gt => "gt",
        Le => "le",
        Ge => "ge",
        _ => return None,
    })
}

fn lookup(env: &Env, name: &str) -> Option<Ty> {
    env.iter()
        .rev()
        .find(|(n, _)| n == name)
        .map(|(_, t)| t.clone())
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

/// `A | Circle(int) | Rect(int, int)` → variants with their payload types.
/// A leaf is either a bare ident (nullary) or a call `Name(T, …)` whose args
/// are type names.
fn sum_variants(e: &Expr) -> Option<Vec<(String, Vec<Type>)>> {
    fn go(e: &Expr, out: &mut Vec<(String, Vec<Type>)>) -> bool {
        match e {
            Expr::Ident(n) => {
                out.push((n.clone(), vec![]));
                true
            }
            Expr::Call { callee, args } => {
                if let Expr::Ident(n) = &**callee {
                    let tys = args.iter().map(|a| type_of_arg(arg_expr(a))).collect();
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

/// A payload type written as a name (`int`, `str`, `Foo`, `mod.Bar`) → `Type`.
fn type_of_arg(e: &Expr) -> Type {
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

/// The closest name in `pool` to `name`, if one is close enough to suggest.
///
/// Plain edit distance, capped so a wrong guess isn't offered: "did you mean"
/// is only useful when it is usually right.
fn nearest<'a>(name: &str, pool: &[&'a str]) -> Option<&'a str> {
    let limit = match name.len() {
        0..=3 => 1,
        4..=7 => 2,
        _ => 3,
    };
    pool.iter()
        .map(|c| (edit_distance(name, c), *c))
        .filter(|(d, _)| *d <= limit)
        .min_by_key(|(d, c)| (*d, c.len()))
        .map(|(_, c)| c)
}

/// Levenshtein distance, two rows at a time.
fn edit_distance(a: &str, b: &str) -> usize {
    let (a, b): (Vec<char>, Vec<char>) = (a.chars().collect(), b.chars().collect());
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    let mut cur = vec![0usize; b.len() + 1];
    for (i, ca) in a.iter().enumerate() {
        cur[0] = i + 1;
        for (j, cb) in b.iter().enumerate() {
            let sub = prev[j] + usize::from(ca != cb);
            cur[j + 1] = sub.min(prev[j + 1] + 1).min(cur[j] + 1);
        }
        std::mem::swap(&mut prev, &mut cur);
    }
    prev[b.len()]
}
