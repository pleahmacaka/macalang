mod diagnostic;
mod ty;

pub use diagnostic::{
    Applicability, Position, Severity, Span, Suggestion, code_word_span, position, resolve_span,
    span_at,
};

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
    /// Reassignment of a constant.
    Immutable,
    /// A direct call to a name that is not defined anywhere.
    UndefinedName,
}

/// Every keyword a reader might reach for that Maca does not have, and the form Maca uses instead.
///
/// The checker turns these into diagnostics and `maca spec --llm` prints them
/// as the mistakes-to-avoid section, so the advice a model is given and the
/// advice the compiler gives cannot drift apart.
pub const PHANTOM_KEYWORDS: &[(&str, &str)] = &[
    (
        "let",
        "write `x = e` for a variable, `const x = e` for a constant; no `let`/`var` keyword",
    ),
    (
        "fn",
        "write the signature straight out: `name(arg: T) -> R { … }` or `name(arg: T) -> R => e`; no `fn` keyword",
    ),
    (
        "type",
        "declare a type by binding it: `Name = { field: T }` for a record, `Name = A | B` for a sum; no `type` keyword",
    ),
    (
        "async",
        "async is an inferred effect, so any function can `spawn` and `await`; no `async` keyword to write",
    ),
    (
        "null",
        "a sum type with an empty variant says what the absence means, and `match` makes you handle it; Maca has no null",
    ),
];

/// Whether the whole fix for a phantom keyword is to delete it.
///
/// `let x = e` becomes `x = e` and `fn f()` becomes `f()`, so the keyword is
/// the entire mistake and removing it is safe without a human reading it.
/// `null` is not: what absence means is a variant only the author can name, so
/// that one is reported and never applied.
pub fn deleting_it_is_the_fix(name: &str) -> bool {
    matches!(
        name,
        "let" | "var" | "fn" | "func" | "def" | "type" | "async"
    )
}

/// The hint for a phantom keyword, including the spellings that mean the same mistake.
pub fn phantom_hint(name: &str) -> Option<&'static str> {
    let canonical = match name {
        "let" | "var" => "let",
        "fn" | "func" | "def" => "fn",
        "type" => "type",
        "async" | "await_" => "async",
        "null" | "nil" | "None" | "undefined" => "null",
        _ => return None,
    };
    PHANTOM_KEYWORDS
        .iter()
        .find(|(k, _)| *k == canonical)
        .map(|(_, hint)| *hint)
}

#[derive(Clone, Debug, PartialEq)]
pub struct Diagnostic {
    pub kind: DiagKind,
    pub msg: String,
    /// What to do about it, when that is more than the message can carry.
    pub note: Option<String>,
    /// The name this is about, so a span is resolved from a field rather than scraped out of the prose.
    pub anchor: Option<String>,
    /// The byte range, when the reporting site knew it.
    pub span: Option<(usize, usize)>,
    pub suggestions: Vec<Suggestion>,
}

impl Diagnostic {
    /// A diagnostic of `kind` saying `msg`, with nothing attached yet.
    pub fn new(kind: DiagKind, msg: impl Into<String>) -> Self {
        Diagnostic {
            kind,
            msg: msg.into(),
            note: None,
            anchor: None,
            span: None,
            suggestions: Vec::new(),
        }
    }

    pub fn with_anchor(mut self, name: impl Into<String>) -> Self {
        self.anchor = Some(name.into());
        self
    }

    pub fn with_note(mut self, note: impl Into<String>) -> Self {
        self.note = Some(note.into());
        self
    }

    pub fn with_suggestion(mut self, s: Suggestion) -> Self {
        self.suggestions.push(s);
        self
    }
}

/// Check a module.
pub fn check(module: &Module, mode: Mode) -> Vec<Diagnostic> {
    let mut c = Checker::new(mode);
    c.collect(module);
    c.run(module);
    c.diags
}

/// Prelude functions that carry the `io` effect.
pub const IO_FNS: &[&str] = &[
    "info", "warn", "err", "debug", "notice", "crit", "alert", "emerg", "panic", "print", "input",
];
/// UFCS method names treated as `io` (file/stdio side effects).
const IO_METHODS: &[&str] = &["read", "write", "exists", "remove", "append", "create"];
/// Prelude functions that carry the `async` effect (suspension points).
const ASYNC_FNS: &[&str] = &["sleep_ms"];

/// Every method a `str` receiver accepts.
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
pub const LIST_METHODS: &[&str] = &[
    "map",
    "filter",
    "reduce",
    "fold",
    "sort",
    "sort_by",
    "reverse",
    "push",
    "pop",
    "set",
    "insert",
    "remove",
    "slice",
    "contains",
    "index_of",
    "index_of_by",
    "enumerate",
    "sum",
    "min",
    "max",
    "first",
    "last",
    "get",
    "length",
    "parallel",
    "join",
];

/// Top-level NixOS / home-manager option namespaces we recognize.
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
    /// sum variant -> its payload field types (empty for a nullary variant).
    variant_payloads: HashMap<String, Vec<Ty>>,
    /// User-declared function name -> (fixed param count, is variadic).
    fn_arity: HashMap<String, (usize, bool)>,
    type_decls: HashSet<usize>,
    local_names: HashSet<String>,
    /// Per-function mutability of local names: `let x` → true, bare `x = e` → false (a constant).
    mut_of: HashMap<String, bool>,
    /// The program pulls in Rust via `import rust` (a path or a raw block whose contents the checker can't see).
    gradual_foreign: bool,
    /// Set for exactly one `infer` call: the callee of a direct `f(…)`.
    callee_pos: bool,
    /// The function a `return` leaves: its name and its declared result type.
    ret_of: Vec<(String, Option<Ty>)>,
    /// Nested function names the current block defines below the point being checked.
    nested_later: HashSet<String>,
    /// Every name some function declares inside its own body, so a read from outside it can say so.
    fn_locals: HashSet<String>,
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
            callee_pos: false,
            ret_of: Vec::new(),
            nested_later: HashSet::new(),
            fn_locals: HashSet::new(),
            diags: Vec::new(),
        }
    }

    fn diag(&mut self, kind: DiagKind, msg: impl Into<String>) {
        self.diags.push(Diagnostic::new(kind, msg));
    }

    /// A keyword Maca doesn't have, used as if it did.
    fn phantom_keyword(&mut self, name: &str) -> bool {
        if self.mode != Mode::Program || self.gradual_foreign {
            return false;
        }
        let Some(hint) = phantom_hint(name) else {
            return false;
        };
        let d =
            Diagnostic::new(DiagKind::UndefinedName, format!("`{name}`: {hint}")).with_anchor(name);
        self.diags.push(match deleting_it_is_the_fix(name) {
            true => d.with_suggestion(Suggestion {
                message: format!("delete `{name}`"),
                span: None,
                replacement: String::new(),
                applicability: Applicability::MachineApplicable,
            }),
            false => d,
        });
        true
    }

    /// A name read here that some other function keeps to itself, which a view's own state makes easy to reach for.
    fn check_name_in_scope(&mut self, name: &str) {
        if self.mode != Mode::Program || self.gradual_foreign {
            return;
        }
        if maca_parser::is_backend_intrinsic(name) {
            return;
        }
        let msg = if self.nested_later.contains(name) {
            format!(
                "`{name}` is defined further down this block; a nested \
                 function is in scope from where it is written, so move \
                 `{name}` above its first use"
            )
        } else if self.fn_locals.contains(name) {
            format!(
                "`{name}` is a local of another function, and a local is \
                 reachable only inside the one that declares it; state two \
                 functions share is written at the top level"
            )
        } else {
            return;
        };
        self.diag(DiagKind::UndefinedName, msg);
    }

    /// The three things a `...rest: T` parameter has to be.
    fn check_variadic(&mut self, f: &FnDef) {
        let Some(at) = f.params.iter().position(|p| p.variadic) else {
            return;
        };
        if at + 1 != f.params.len() {
            let after = &f.params[at + 1].name;
            self.diag(
                DiagKind::TypeMismatch,
                format!(
                    "`{}`: a variadic parameter must be last, but `{after}` \
                     follows `...{}`",
                    f.name, f.params[at].name,
                ),
            );
        }
        for p in f.params.iter().filter(|p| p.variadic) {
            if p.ty.is_none() {
                self.diag(
                    DiagKind::TypeMismatch,
                    format!(
                        "`{}`: a variadic parameter needs its element type; \
                         write `...{}: T`",
                        f.name, p.name,
                    ),
                );
            }
        }
        if f.name == "main" {
            self.diag(
                DiagKind::TypeMismatch,
                "`main` cannot be variadic: its arguments come from the \
                 command line; declare `main(argv: str[])`"
                    .to_string(),
            );
        }
    }

    /// A variadic function is callable and nothing else.
    fn check_not_variadic_value(&mut self, name: &str) {
        if self.fn_arity.get(name).is_some_and(|&(_, v)| v) {
            self.diag(
                DiagKind::TypeMismatch,
                format!(
                    "`{name}` is variadic, so it cannot be used as a function \
                     value; call it, or declare it `{name}(xs: T[])` and pass \
                     a list"
                ),
            );
        }
    }

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
            ("chr", Ty::Fn(vec![Ty::Int], Box::new(Ty::Str))),
            ("real_path", Ty::Fn(vec![Ty::Str], Box::new(Ty::Str))),
            ("is_tty", Ty::Fn(vec![], Box::new(Ty::Bool))),
            ("ord", Ty::Fn(vec![Ty::Str], Box::new(Ty::Int))),
            ("input", Ty::Fn(vec![], Box::new(Ty::Str))),
            ("sleep_ms", Ty::Fn(vec![Ty::Int], Box::new(Ty::Unit))),
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
                    collect_fn_locals(f, &mut self.fn_locals);
                    self.check_variadic(f);
                    let variadic = f.params.iter().any(|p| p.variadic);
                    let arity = f.params.len() - usize::from(variadic);
                    self.fn_arity.insert(f.name.clone(), (arity, variadic));
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
                if lang == "rust" {
                    self.gradual_foreign = true;
                }
                bind(self, lang.clone());
            }
            Import::Bare(n) => bind(self, n.clone()),
            Import::Path(_) => {}
        }
    }

    /// Generalize a function signature into a polymorphic scheme.
    fn sig_scheme(&mut self, f: &FnDef) -> Scheme {
        let mut vars: HashMap<String, Ty> = HashMap::new();
        let params: Vec<Ty> = f
            .params
            .iter()
            .map(|p| {
                let t =
                    p.ty.as_ref()
                        .map_or(Ty::Any, |t| self.ast_ty_v(t, &mut vars));
                if p.variadic { Ty::array(t) } else { t }
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

    /// Like [`ast_ty`] but maps type-variable names through `vars`, minting one fresh inference variable per distinct name.
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
            Type::Fn(ps, r) => {
                let ps = ps.iter().map(|p| self.ast_ty_v(p, vars)).collect();
                Ty::Fn(ps, Box::new(self.ast_ty_v(r, vars)))
            }
            Type::Name(_) => ast_ty(t),
        }
    }

    fn run(&mut self, m: &Module) {
        for (i, item) in m.items.iter().enumerate() {
            match item {
                Stmt::Fn(f) => self.check_fn(f),
                Stmt::Bind(b) if !self.type_decls.contains(&i) => self.check_bind(b),
                Stmt::Alias { value, .. } if self.mode == Mode::Config => {
                    self.check_config_effects(value);
                }
                Stmt::Expr(e) if self.mode == Mode::Config => {
                    self.check_config_effects(e);
                }
                _ => {}
            }
        }
    }

    fn check_fn(&mut self, f: &FnDef) {
        self.check_fn_in(&Env::new(), f, false);
    }

    /// Check a function body, with `outer` the enclosing scope a nested definition reads and writes.
    fn check_fn_in(&mut self, outer: &Env, f: &FnDef, nested: bool) {
        let Some(body) = &f.body else { return };
        let mut env: Env = outer.to_vec();
        env.extend(f.params.iter().map(|p| (p.name.clone(), param_ty(p))));
        let saved_mut = if nested {
            self.mut_of.clone()
        } else {
            std::mem::take(&mut self.mut_of)
        };
        for p in &f.params {
            self.mut_of.insert(p.name.clone(), true);
        }
        let declared = f.ret.as_ref().map(|t| {
            let t = ast_ty(t);
            self.relax_ann(t)
        });
        self.ret_of.push((f.name.clone(), declared.clone()));
        self.check_return_sites(&f.name, body);
        let bty = match body {
            FnBody::Block(stmts) => self.infer_block(&mut env, stmts),
            FnBody::Expr(e) => self.infer(&mut env, e),
        };
        self.ret_of.pop();
        self.mut_of = saved_mut;
        if let Some(rt) = declared
            && let Err(e) = self.unify_ann(&rt, &bty)
        {
            self.diag(DiagKind::TypeMismatch, format!("in `{}`: {e}", f.name));
        }
    }

    /// A nested definition is a value bound where it is written, so it cannot name itself.
    fn check_nested_self_reference(&mut self, f: &FnDef) {
        let Some(body) = &f.body else { return };
        let mut names = HashSet::new();
        match body {
            FnBody::Expr(e) => walk_names(e, &mut |n| {
                names.insert(n.to_string());
            }),
            FnBody::Block(stmts) => {
                for s in stmts {
                    walk_stmt(s, &mut |e| {
                        if let Expr::Ident(n) = e {
                            names.insert(n.clone());
                        }
                    });
                }
            }
        }
        if names.contains(&f.name) {
            self.diag(
                DiagKind::UndefinedName,
                format!(
                    "`{}` is a nested function, which is a value bound where it is \
                     written, so it cannot name itself; move `{}` to the top level, \
                     where a function is in scope everywhere",
                    f.name, f.name
                ),
            );
        }
    }

    /// Report every `return` written where there is no function for it to leave.
    fn check_return_sites(&mut self, name: &str, body: &FnBody) {
        match body {
            FnBody::Block(stmts) => self.returns_in_stmts(name, stmts),
            FnBody::Expr(e) => self.returns_in_value(name, e),
        }
    }

    fn returns_in_stmts(&mut self, name: &str, stmts: &[Stmt]) {
        for s in stmts {
            match s {
                Stmt::Expr(e) => self.returns_in_stmt(name, e),
                Stmt::Bind(b) if is_control(&b.value) => self.returns_in_stmt(name, &b.value),
                Stmt::Bind(b) => self.returns_in_value(name, &b.value),
                Stmt::Alias { value, .. } => self.returns_in_value(name, value),
                Stmt::Fn(_) | Stmt::Import(_) => {}
            }
        }
    }

    /// A place a `return` can stand: the backends lower it to the host language's own `return`.
    fn returns_in_stmt(&mut self, name: &str, e: &Expr) {
        match e {
            Expr::Return(v) => {
                if let Some(x) = v {
                    self.returns_in_value(name, x);
                }
            }
            Expr::Block(stmts) => self.returns_in_stmts(name, stmts),
            Expr::If { cond, then, els } => {
                self.returns_in_value(name, cond);
                self.returns_in_stmts(name, then);
                if let Some(e) = els {
                    self.returns_in_stmts(name, e);
                }
            }
            Expr::Match { scrut, arms } => {
                self.returns_in_value(name, scrut);
                for a in arms {
                    if let Some(g) = &a.guard {
                        self.returns_in_value(name, g);
                    }
                    self.returns_in_stmt(name, &a.body);
                }
            }
            Expr::For { iter, body, .. } => {
                self.returns_in_value(name, iter);
                self.returns_in_stmts(name, body);
            }
            Expr::While { cond, body } => {
                self.returns_in_value(name, cond);
                self.returns_in_stmts(name, body);
            }
            other => self.returns_in_value(name, other),
        }
    }

    /// What a `return` carries, against what the function it leaves declared.
    fn check_returned(&mut self, given: Option<Ty>) {
        let Some((name, declared)) = self.ret_of.last().cloned() else {
            return;
        };
        match (declared, given) {
            (Some(want), Some(got)) => {
                if let Err(e) = self.unify_ann(&want, &got) {
                    self.diag(DiagKind::TypeMismatch, format!("in `{name}`: {e}"));
                }
            }
            (None, Some(_)) => self.diag(
                DiagKind::TypeMismatch,
                format!(
                    "`{name}` declares no result, so `return <value>` has nothing to \
                     return it to; write `{name}(…) -> T`"
                ),
            ),
            (Some(want), None) if !matches!(self.inf.resolve(&want), Ty::Unit | Ty::Any) => self
                .diag(
                    DiagKind::TypeMismatch,
                    format!(
                        "`{name}` returns `{}`, so a bare `return` leaves without one; \
                         write `return <value>`",
                        show(&self.inf.resolve(&want))
                    ),
                ),
            _ => {}
        }
    }

    /// Everywhere below here is an expression, so a `return` in it has nowhere to go.
    fn returns_in_value(&mut self, name: &str, e: &Expr) {
        if has_return(e) {
            self.diag(
                DiagKind::TypeMismatch,
                format!(
                    "in `{name}`: this `return` stands inside an expression, where \
                     there is no statement to leave from; `return` goes on a line of \
                     its own, or in an `if`/`match` branch"
                ),
            );
        }
    }

    /// Unify an annotation with the value it annotates.
    fn unify_ann(&mut self, want: &Ty, got: &Ty) -> Result<(), String> {
        let (w, g) = self.meet_records(want, got);
        self.inf.unify(&w, &g)
    }

    /// A declared record type and a structurally identical record literal are one type.
    fn meet_records(&self, want: &Ty, got: &Ty) -> (Ty, Ty) {
        let w = self.inf.resolve(want);
        let g = self.inf.resolve(got);
        match (&w, &g) {
            (_, Ty::Rec { .. }) if self.declared_shape(&w).is_some() => {
                let decl = self.declared_shape(&w).expect("just checked");
                self.meet_fields(&decl, &g)
            }
            (Ty::Rec { .. }, _) if self.declared_shape(&g).is_some() => {
                let decl = self.declared_shape(&g).expect("just checked");
                let (decl, lit) = self.meet_fields(&decl, &w);
                (lit, decl)
            }
            (Ty::Con(n, wa), Ty::Con(m, ga)) if n == m && wa.len() == ga.len() => {
                let met: Vec<(Ty, Ty)> = wa
                    .iter()
                    .zip(ga)
                    .map(|(x, y)| self.meet_records(x, y))
                    .collect();
                (
                    Ty::Con(n.clone(), met.iter().map(|(a, _)| a.clone()).collect()),
                    Ty::Con(m.clone(), met.iter().map(|(_, b)| b.clone()).collect()),
                )
            }
            (Ty::Opt(x), Ty::Opt(y)) => {
                let (a, b) = self.meet_records(x, y);
                (Ty::Opt(Box::new(a)), Ty::Opt(Box::new(b)))
            }
            _ => (w, g),
        }
    }

    /// The fields a named record declares, as a closed structural type.
    fn declared_shape(&self, t: &Ty) -> Option<Ty> {
        let Ty::Con(n, args) = t else { return None };
        if !args.is_empty() {
            return None;
        }
        Some(Ty::Rec {
            fields: self.records.get(n)?.clone(),
            open: false,
        })
    }

    /// Line a declared record's fields up with a literal's, recursively, and close the literal.
    fn meet_fields(&self, decl: &Ty, lit: &Ty) -> (Ty, Ty) {
        let (Ty::Rec { fields: df, .. }, Ty::Rec { fields: lf, .. }) = (decl, lit) else {
            return (decl.clone(), lit.clone());
        };
        let mut d = BTreeMap::new();
        let mut l = lf.clone();
        for (k, dt) in df {
            match lf.get(k) {
                Some(lt) => {
                    let (a, b) = self.meet_records(dt, lt);
                    d.insert(k.clone(), a);
                    l.insert(k.clone(), b);
                }
                None => {
                    d.insert(k.clone(), dt.clone());
                }
            }
        }
        (
            Ty::Rec {
                fields: d,
                open: false,
            },
            Ty::Rec {
                fields: l,
                open: false,
            },
        )
    }

    /// Relax an *annotation* type: a bare, capitalized nominal that isn't a declared record/sum is a foreign / interop type (a Java interface, a Nix value, …).
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
            if let Err(e) = self.unify_ann(&at, &vty) {
                let name = target_name(&b.target);
                self.diag(DiagKind::TypeMismatch, format!("in binding `{name}`: {e}"));
            }
        }
    }

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
            Expr::Reify(x) => acc = EffSet(self.eff(x).0 & !EXN),
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
            Expr::Return(Some(x)) => acc = self.eff(x),
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
                Stmt::Fn(f) => match &f.body {
                    Some(FnBody::Expr(e)) => self.eff(e),
                    Some(FnBody::Block(ss)) => self.eff_stmts(ss),
                    None => EffSet::empty(),
                },
                _ => EffSet::empty(),
            });
        }
        acc
    }

    fn infer_block(&mut self, env: &mut Env, stmts: &[Stmt]) -> Ty {
        let base = env.len();
        let mut last = Ty::Unit;
        let later: HashSet<String> = stmts
            .iter()
            .filter_map(|s| match s {
                Stmt::Fn(f) => Some(f.name.clone()),
                _ => None,
            })
            .collect();
        let saved_later = std::mem::replace(&mut self.nested_later, later);
        for s in stmts {
            match s {
                Stmt::Bind(b) => {
                    if let Expr::Ident(n) = &b.target {
                        match self.mut_of.get(n).copied() {
                            Some(false) => self.diag(
                                DiagKind::Immutable,
                                format!("cannot reassign constant `{n}`; declare it mutable with `{n} = …` (no `const`)"),
                            ),
                            Some(true) => {
                                if b.is_const {
                                    self.mut_of.insert(n.clone(), false);
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
                        if let Err(e) = self.unify_ann(&at, &vty) {
                            let name = target_name(&b.target);
                            self.diag(DiagKind::TypeMismatch, format!("in `{name}`: {e}"));
                        }
                        vty = at;
                    }
                    if let Expr::Ident(n) = &b.target {
                        env.push((n.clone(), vty));
                    }
                    last = Ty::Unit;
                }
                Stmt::Expr(e) => last = self.infer(env, e),
                Stmt::Fn(f) => {
                    self.nested_later.remove(&f.name);
                    self.check_nested_self_reference(f);
                    let params = f.params.iter().map(param_ty).collect();
                    let ret = f.ret.as_ref().map_or(Ty::Any, ast_ty);
                    env.push((f.name.clone(), Ty::Fn(params, Box::new(ret))));
                    let outer = env.clone();
                    self.check_fn_in(&outer, f, true);
                    self.mut_of.insert(f.name.clone(), false);
                    last = Ty::Unit;
                }
                _ => last = Ty::Unit,
            }
        }
        self.nested_later = saved_later;
        env.truncate(base);
        last
    }

    fn infer(&mut self, env: &mut Env, e: &Expr) -> Ty {
        let callee_pos = std::mem::take(&mut self.callee_pos);
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
            Expr::Ident(n) => {
                if !callee_pos && lookup(env, n).is_none() {
                    self.check_not_variadic_value(n);
                }
                match lookup(env, n) {
                    Some(t) => t,
                    None => match self.globals.get(n).cloned() {
                        Some(s) => self.inf.instantiate(&s),
                        None => {
                            if !self.phantom_keyword(n) && !callee_pos {
                                self.check_name_in_scope(n);
                            }
                            Ty::Any
                        }
                    },
                }
            }
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
                let mut fixed = None;
                if let Expr::Ident(name) = &**callee
                    && lookup(env, name).is_none()
                    && let Some(&(arity, variadic)) = self.fn_arity.get(name)
                {
                    if variadic {
                        fixed = Some(arity);
                    }
                    let wrong = if variadic {
                        args.len() < arity
                    } else {
                        args.len() != arity
                    };
                    if wrong {
                        let least = if variadic { "at least " } else { "" };
                        self.diag(
                            DiagKind::TypeMismatch,
                            format!(
                                "call to `{name}` expects {least}{arity} argument(s), got {}",
                                args.len()
                            ),
                        );
                    }
                }
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
                    let msg = if self.nested_later.contains(name) {
                        format!(
                            "`{name}` is defined further down this block; a nested \
                             function is in scope from where it is written, so move \
                             `{name}` above its first use"
                        )
                    } else {
                        format!("call to undefined function `{name}`")
                    };
                    self.diag(DiagKind::UndefinedName, msg);
                }
                if let Expr::Field { base, name } = &**callee {
                    let bt = self.infer(env, base);
                    self.check_method(&bt, name);
                }
                self.callee_pos = matches!(&**callee, Expr::Ident(_));
                let ct = self.infer(env, callee);
                let ats: Vec<Ty> = args.iter().map(|a| self.infer_arg(env, a)).collect();
                match self.inf.resolve(&ct) {
                    Ty::Fn(params, ret) => {
                        let params = match fixed {
                            Some(n) if params.len() == n + 1 => {
                                let elem = match self.inf.resolve(&params[n]) {
                                    Ty::Con(c, a) if c == "Array" && a.len() == 1 => a[0].clone(),
                                    other => other,
                                };
                                let mut ps = params[..n].to_vec();
                                ps.resize(ats.len().max(n), elem);
                                ps
                            }
                            _ => params,
                        };
                        for (i, (p, a)) in params.iter().zip(&ats).enumerate() {
                            if let Err(e) = self.unify_ann(p, a) {
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
                    Ty::Con(n, _) if n == "str" || n == "bytes" => Ty::Con(n, vec![]),
                    _ => Ty::Any,
                }
            }
            Expr::Range { lo, hi } => {
                let lt = self.infer(env, lo);
                let rt = self.infer(env, hi);
                if let Err(e) = self.inf.unify(&Ty::Int, &lt) {
                    self.diag(
                        DiagKind::TypeMismatch,
                        format!("range start must be `int`: {e}"),
                    );
                }
                if let Err(e) = self.inf.unify(&Ty::Int, &rt) {
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
                if let Err(e) = self.inf.unify(&Ty::Bool, &ct) {
                    self.diag(DiagKind::TypeMismatch, format!("`while` condition: {e}"));
                }
                let base = env.len();
                self.infer_block(env, body);
                env.truncate(base);
                Ty::Unit
            }
            Expr::Break | Expr::Continue => Ty::Unit,
            Expr::Return(v) => {
                let given = v.as_ref().map(|x| self.infer(env, x));
                self.check_returned(given);
                self.inf.fresh()
            }
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
            Expr::Spawn(x) => {
                let a = self.infer(env, x);
                Ty::Con("Future".into(), vec![a])
            }
            Expr::Await(x) => {
                let t = self.infer(env, x);
                match self.inf.resolve(&t) {
                    Ty::Con(n, args) if n == "Future" && args.len() == 1 => args[0].clone(),
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

    /// A record literal has to name every field the record declares, and no field it doesn't.
    fn check_record_fields(&mut self, name: &str, fields: &[Field]) {
        if self.mode != Mode::Program {
            return;
        }
        let Some(declared) = self.records.get(name).cloned() else {
            return;
        };
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
    fn check_method(&mut self, recv: &Ty, name: &str) {
        if self.mode != Mode::Program || self.gradual_foreign {
            return;
        }
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
                Some(alt) => format!("`{what}` has no method `{name}`; did you mean `{alt}`?"),
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
                self.check_pattern_name(n, scrut);
                env.push((n.clone(), scrut.clone()));
                self.mut_of.insert(n.clone(), true);
            }
            Pattern::Ctor { name, args } => {
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

    /// A Capitalized name in a pattern is a constructor, so one nothing declares is a misspelling rather than a name that matches everything.
    fn check_pattern_name(&mut self, name: &str, scrut: &Ty) {
        if self.mode != Mode::Program || !name.starts_with(char::is_uppercase) {
            return;
        }
        let of = match self.inf.resolve(scrut) {
            Ty::Con(n, _) => self.sums.get(&n).cloned().unwrap_or_default(),
            _ => Vec::new(),
        };
        let choices: Vec<&str> = match of.is_empty() {
            true => self.sums.values().flatten().map(String::as_str).collect(),
            false => of.iter().map(String::as_str).collect(),
        };
        let tail = nearest(name, &choices).map_or(
            "a pattern that binds what it matched is lowercase".to_string(),
            |alt| format!("did you mean `{alt}`?"),
        );
        self.diag(
            DiagKind::UndefinedName,
            format!(
                "`{name}` is capitalized, so it is a constructor, and nothing declares \
                 one by that name: {tail}"
            ),
        );
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

/// Every name `f` declares inside itself: its parameters, its bindings, and the same for anything nested in it.
fn collect_fn_locals(f: &FnDef, out: &mut HashSet<String>) {
    for p in &f.params {
        out.insert(p.name.clone());
    }
    if let Some(FnBody::Block(stmts)) = &f.body {
        collect_bound_names(stmts, out);
    }
}

fn collect_bound_names(stmts: &[Stmt], out: &mut HashSet<String>) {
    for s in stmts {
        match s {
            Stmt::Bind(b) => {
                if let Expr::Ident(n) = &b.target {
                    out.insert(n.clone());
                }
            }
            Stmt::Fn(nested) => {
                out.insert(nested.name.clone());
                collect_fn_locals(nested, out);
            }
            _ => {}
        }
    }
}

/// Does this expression reach a statement context, where a `return` can stand?
fn is_control(e: &Expr) -> bool {
    matches!(
        e,
        Expr::If { .. }
            | Expr::Match { .. }
            | Expr::For { .. }
            | Expr::While { .. }
            | Expr::Block(_)
    )
}

/// Is there a `return` in here that belongs to the function being checked, rather than to a nested definition of its own?
fn has_return(e: &Expr) -> bool {
    match e {
        Expr::Return(_) => true,
        Expr::Str(parts) => parts
            .iter()
            .any(|p| matches!(p, StrPart::Interp(x) if has_return(x))),
        Expr::List(xs) => xs.iter().any(has_return),
        Expr::Record(fs) | Expr::Ctor { fields: fs, .. } => fs.iter().any(has_return_field),
        Expr::With { base, fields } => has_return(base) || fields.iter().any(has_return_field),
        Expr::Call { callee, args } => {
            has_return(callee) || args.iter().any(|a| has_return(arg_expr(a)))
        }
        Expr::Field { base, .. } => has_return(base),
        Expr::Index { base, index } => has_return(base) || has_return(index),
        Expr::Range { lo, hi } => has_return(lo) || has_return(hi),
        Expr::Unary { expr, .. } => has_return(expr),
        Expr::Binary { lhs, rhs, .. } => has_return(lhs) || has_return(rhs),
        Expr::Ternary { cond, then, els } => {
            has_return(cond) || has_return(then) || has_return(els)
        }
        Expr::If { cond, then, els } => {
            has_return(cond)
                || has_return_stmts(then)
                || els.as_ref().is_some_and(|e| has_return_stmts(e))
        }
        Expr::Match { scrut, arms } => {
            has_return(scrut)
                || arms
                    .iter()
                    .any(|a| a.guard.as_ref().is_some_and(has_return) || has_return(&a.body))
        }
        Expr::For { iter, body, .. } => has_return(iter) || has_return_stmts(body),
        Expr::While { cond, body } => has_return(cond) || has_return_stmts(body),
        Expr::Lambda { body, .. } => has_return(body),
        Expr::Try(x) | Expr::Fail(x) | Expr::Reify(x) | Expr::Await(x) | Expr::Spawn(x) => {
            has_return(x)
        }
        Expr::Assign { target, value } => has_return(target) || has_return(value),
        Expr::Block(ss) => has_return_stmts(ss),
        _ => false,
    }
}

fn has_return_field(f: &Field) -> bool {
    match f {
        Field::Value { value, .. } | Field::Bare(value) => has_return(value),
        Field::Type { .. } | Field::Shorthand(_) => false,
    }
}

fn has_return_stmts(stmts: &[Stmt]) -> bool {
    stmts.iter().any(|s| match s {
        Stmt::Expr(e) | Stmt::Alias { value: e, .. } => has_return(e),
        Stmt::Bind(b) => has_return(&b.value),
        Stmt::Fn(_) | Stmt::Import(_) => false,
    })
}

/// The type a parameter has *inside the body*.
fn param_ty(p: &Param) -> Ty {
    let t = p.ty.as_ref().map_or(Ty::Any, ast_ty);
    if p.variadic { Ty::array(t) } else { t }
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
        Type::Fn(ps, r) => Ty::Fn(ps.iter().map(ast_ty).collect(), Box::new(ast_ty(r))),
    }
}

/// `A | Circle(int) | Rect(int, int)` → variants with their payload types.
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
