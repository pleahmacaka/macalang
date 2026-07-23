//! A small tree-walking interpreter over the Maca AST, for the browser
//! playground: it *runs* a program (capturing `info`/`print` output) and
//! collects a lightweight execution profile (per-function call counts, total
//! calls, max recursion depth, evaluation steps). It is not the language's
//! semantics of record — the native C backend is — but it mirrors them closely
//! enough to demonstrate behaviour and cost in the playground.
//!
//! Values are copied (records and lists are value types, matching the native
//! lowering), so `let q = p; q.x = 1` never disturbs `p`.

use maca_parser::ast::*;
use maca_profile::FnCost;
use std::collections::HashMap;
use std::fmt::Write as _;

/// Cap on evaluation steps so a runaway loop can't hang the browser tab.
const STEP_LIMIT: u64 = 30_000_000;
/// Cap on call depth so unbounded recursion reports cleanly instead of
/// overflowing the wasm stack.
const DEPTH_LIMIT: u64 = 6_000;

#[derive(Clone, Debug)]
pub enum Value {
    Int(i64),
    Float(f64),
    Bool(bool),
    Str(String),
    Unit,
    List(Vec<Value>),
    Record(Vec<(String, Value)>),
    Variant { tag: String, payload: Vec<Value> },
}

/// Non-local control flow: loop control, a raised failure, or the step/depth
/// guard tripping (which is not catchable by `try`).
enum Signal {
    Break,
    Continue,
    Fail(String),
    Limit(String),
}

type Eval = Result<Value, Signal>;

pub struct Profile {
    /// the flame graph, rendered by the shared `maca-profile` renderer (the same
    /// one `maca profile` uses natively; here fed by interpreter step counts).
    pub flame_svg: String,
    pub total_calls: u64,
    pub max_depth: u64,
    pub steps: u64,
    pub truncated: bool,
}

pub struct RunResult {
    pub output: String,
    pub error: Option<String>,
    pub profile: Profile,
    pub exit: Option<i64>,
}

pub fn run(module: &Module) -> RunResult {
    let mut it = Interp::new(module);
    let mut exit = None;
    let mut error = None;

    // find `main`
    let main = module.items.iter().find_map(|s| match s {
        Stmt::Fn(f) if f.name == "main" => Some(f.clone()),
        _ => None,
    });
    match main {
        None => error = Some("no `main` function to run".into()),
        Some(f) => match it.call_fn(&f, Vec::new(), 0) {
            Ok(v) => {
                if let Value::Int(n) = v {
                    exit = Some(n);
                }
            }
            Err(Signal::Fail(m)) => error = Some(format!("uncaught failure: {m}")),
            Err(Signal::Limit(m)) => error = Some(m),
            Err(_) => {}
        },
    }

    // assemble the shared cost model: self cost per function + inclusive cost
    // per call edge, then render the same flame graph the native profiler draws.
    let mut fns: HashMap<String, FnCost> = HashMap::new();
    for (name, self_c) in &it.self_steps {
        fns.entry(name.clone()).or_default().self_cost = *self_c;
    }
    for (caller, edges) in &it.edges {
        let fc = fns.entry(caller.clone()).or_default();
        for (callee, cost) in edges {
            fc.calls.insert(callee.clone(), *cost);
        }
    }
    // HTML (percentage widths) so it fills the playground panel exactly — no
    // fixed intrinsic width, no horizontal scroll — while rows stay tall.
    let flame_svg = maca_profile::flamegraph_html_from(&fns, "steps");

    RunResult {
        output: it.out,
        error,
        exit,
        profile: Profile {
            flame_svg,
            total_calls: it.total_calls,
            max_depth: it.max_depth,
            steps: it.steps,
            truncated: it.steps >= STEP_LIMIT,
        },
    }
}

struct Interp<'a> {
    fns: HashMap<String, &'a FnDef>,
    /// variant name → arity (0 for a nullary variant like `Nil`)
    variants: HashMap<String, usize>,
    out: String,
    total_calls: u64,
    max_depth: u64,
    steps: u64,
    // profiling: an active-frame stack (name + self-step counter), plus the
    // accumulated self cost per function and inclusive cost per call edge.
    stack: Vec<(String, u64)>,
    self_steps: HashMap<String, u64>,
    edges: HashMap<String, HashMap<String, u64>>,
}

type Scope = Vec<(String, Value)>;

impl<'a> Interp<'a> {
    fn new(m: &'a Module) -> Self {
        let mut fns = HashMap::new();
        let mut variants = HashMap::new();
        for s in &m.items {
            match s {
                Stmt::Fn(f) if f.body.is_some() => {
                    fns.insert(f.name.clone(), f);
                }
                Stmt::Bind(b) => {
                    if let Expr::Ident(_) = &b.target {
                        if let Some(vs) = sum_variants(&b.value) {
                            for (name, arity) in vs {
                                variants.insert(name, arity);
                            }
                        }
                    }
                }
                _ => {}
            }
        }
        Interp {
            fns,
            variants,
            out: String::new(),
            total_calls: 0,
            max_depth: 0,
            steps: 0,
            stack: Vec::new(),
            self_steps: HashMap::new(),
            edges: HashMap::new(),
        }
    }

    fn tick(&mut self) -> Result<(), Signal> {
        self.steps += 1;
        // attribute this step to the function currently executing (self cost)
        if let Some(top) = self.stack.last_mut() {
            top.1 += 1;
        }
        if self.steps >= STEP_LIMIT {
            return Err(Signal::Limit("execution step limit reached — possible infinite loop".into()));
        }
        Ok(())
    }

    fn call_fn(&mut self, f: &FnDef, args: Vec<Value>, depth: u64) -> Eval {
        if depth >= DEPTH_LIMIT {
            return Err(Signal::Limit("maximum recursion depth reached".into()));
        }
        self.total_calls += 1;
        if depth > self.max_depth {
            self.max_depth = depth;
        }

        let mut scope: Scope = Vec::new();
        for (p, v) in f.params.iter().zip(args) {
            scope.push((p.name.clone(), v));
        }
        // profiling: push a frame, run, then fold its self cost and the
        // inclusive cost of this activation onto the caller's call edge.
        let before = self.steps;
        self.stack.push((f.name.clone(), 0));
        let result = match &f.body {
            Some(FnBody::Expr(e)) => self.eval(e, &mut scope, depth),
            Some(FnBody::Block(stmts)) => self.block(stmts, &mut scope, depth),
            None => Ok(Value::Unit),
        };
        let self_c = self.stack.pop().map(|(_, c)| c).unwrap_or(0);
        *self.self_steps.entry(f.name.clone()).or_default() += self_c;
        let incl = self.steps - before;
        if let Some((caller, _)) = self.stack.last() {
            let caller = caller.clone();
            *self.edges.entry(caller).or_default().entry(f.name.clone()).or_default() += incl;
        }
        result
    }

    /// Evaluate a block; its value is the last expression (or Unit).
    fn block(&mut self, stmts: &[Stmt], scope: &mut Scope, depth: u64) -> Eval {
        let base = scope.len();
        let mut last = Value::Unit;
        for (i, s) in stmts.iter().enumerate() {
            let is_last = i + 1 == stmts.len();
            match s {
                Stmt::Bind(b) => {
                    let v = self.eval(&b.value, scope, depth)?;
                    match &b.target {
                        // a bare `x = e` first introducing `x` declares a new
                        // binding (mutable or const); an assignment to an existing
                        // name reassigns it (the checker rejects const reassigns).
                        Expr::Ident(n) if !scope.iter().any(|(k, _)| k == n) => {
                            scope.push((n.clone(), v));
                        }
                        _ => self.assign(&b.target, v, scope, depth)?,
                    }
                    last = Value::Unit;
                }
                Stmt::Expr(e) => {
                    let v = self.eval(e, scope, depth)?;
                    if is_last {
                        last = v;
                    }
                }
                _ => {}
            }
        }
        scope.truncate(base);
        Ok(last)
    }

    fn assign(&mut self, target: &Expr, v: Value, scope: &mut Scope, depth: u64) -> Result<(), Signal> {
        match target {
            Expr::Ident(n) => {
                if let Some(slot) = scope.iter_mut().rev().find(|(k, _)| k == n) {
                    slot.1 = v;
                }
                Ok(())
            }
            Expr::Index { base, index } => {
                let i = match self.eval(index, scope, depth)? {
                    Value::Int(i) => i,
                    _ => 0,
                };
                let loc = self.lvalue(base, scope, depth)?;
                if let Some(Value::List(xs)) = loc {
                    if i >= 0 && (i as usize) < xs.len() {
                        xs[i as usize] = v;
                    }
                }
                Ok(())
            }
            Expr::Field { base, name } => {
                let loc = self.lvalue(base, scope, depth)?;
                if let Some(Value::Record(fs)) = loc {
                    if let Some(slot) = fs.iter_mut().find(|(k, _)| k == name) {
                        slot.1 = v;
                    } else {
                        fs.push((name.clone(), v));
                    }
                }
                Ok(())
            }
            _ => Ok(()),
        }
    }

    /// A mutable reference to the storage a target denotes (for in-place writes).
    fn lvalue<'s>(&mut self, target: &Expr, scope: &'s mut Scope, depth: u64) -> Result<Option<&'s mut Value>, Signal> {
        match target {
            Expr::Ident(n) => Ok(scope.iter_mut().rev().find(|(k, _)| k == n).map(|(_, v)| v)),
            Expr::Field { base, name } => {
                let inner = self.lvalue(base, scope, depth)?;
                Ok(inner.and_then(|v| match v {
                    Value::Record(fs) => fs.iter_mut().find(|(k, _)| k == name).map(|(_, v)| v),
                    _ => None,
                }))
            }
            Expr::Index { base, index } => {
                // index must be evaluated before the mutable borrow of `base`
                let i = match self.eval(index, scope, depth)? {
                    Value::Int(i) => i as usize,
                    _ => 0,
                };
                let inner = self.lvalue(base, scope, depth)?;
                Ok(inner.and_then(|v| match v {
                    Value::List(xs) if i < xs.len() => Some(&mut xs[i]),
                    _ => None,
                }))
            }
            _ => Ok(None),
        }
    }

    fn eval(&mut self, e: &Expr, scope: &mut Scope, depth: u64) -> Eval {
        self.tick()?;
        match e {
            Expr::Int(n) => Ok(Value::Int(*n)),
            Expr::Float(f) => Ok(Value::Float(*f)),
            Expr::Bool(b) => Ok(Value::Bool(*b)),
            Expr::Unit => Ok(Value::Unit),
            Expr::Path(p) => Ok(Value::Str(p.clone())),
            Expr::Str(parts) => {
                let mut s = String::new();
                for p in parts {
                    match p {
                        StrPart::Text(t) => s.push_str(t),
                        StrPart::Interp(e) => {
                            let v = self.eval(e, scope, depth)?;
                            s.push_str(&display(&v));
                        }
                    }
                }
                Ok(Value::Str(s))
            }
            Expr::Ident(n) => {
                if let Some((_, v)) = scope.iter().rev().find(|(k, _)| k == n) {
                    Ok(v.clone())
                } else if self.variants.get(n) == Some(&0) {
                    Ok(Value::Variant { tag: n.clone(), payload: vec![] })
                } else {
                    Ok(Value::Unit)
                }
            }
            Expr::List(es) => {
                let mut xs = Vec::with_capacity(es.len());
                for e in es {
                    xs.push(self.eval(e, scope, depth)?);
                }
                Ok(Value::List(xs))
            }
            Expr::Record(fields) | Expr::Ctor { fields, .. } => {
                let mut fs = Vec::new();
                for f in fields {
                    if let Field::Value { name, value } = f {
                        let v = self.eval(value, scope, depth)?;
                        fs.push((name.clone(), v));
                    } else if let Field::Shorthand(n) = f {
                        let v = self.eval(&Expr::Ident(n.clone()), scope, depth)?;
                        fs.push((n.clone(), v));
                    }
                }
                Ok(Value::Record(fs))
            }
            Expr::Field { base, name } => {
                let b = self.eval(base, scope, depth)?;
                Ok(match b {
                    Value::Record(fs) => fs.into_iter().find(|(k, _)| k == name).map(|(_, v)| v).unwrap_or(Value::Unit),
                    _ => Value::Unit,
                })
            }
            Expr::Index { base, index } => {
                let b = self.eval(base, scope, depth)?;
                let i = self.eval(index, scope, depth)?;
                let i = match i {
                    Value::Int(i) => i,
                    _ => 0,
                };
                Ok(match b {
                    Value::List(xs) if i >= 0 && (i as usize) < xs.len() => xs[i as usize].clone(),
                    Value::Str(s) => s.chars().nth(i.max(0) as usize).map(|c| Value::Str(c.to_string())).unwrap_or(Value::Str(String::new())),
                    _ => Value::Unit,
                })
            }
            Expr::Range { lo, hi } => {
                // inclusive: lo..hi counts lo, lo+1, …, hi
                let a = to_i64(&self.eval(lo, scope, depth)?);
                let b = to_i64(&self.eval(hi, scope, depth)?);
                let mut xs = Vec::new();
                let mut i = a;
                while i <= b {
                    self.tick()?; // per-element, so a runaway range trips the step limit cleanly
                    xs.push(Value::Int(i));
                    i += 1;
                }
                Ok(Value::List(xs))
            }
            Expr::Unary { op, expr } => {
                let v = self.eval(expr, scope, depth)?;
                Ok(match (op, v) {
                    (UnOp::Neg, Value::Int(n)) => Value::Int(-n),
                    (UnOp::Neg, Value::Float(f)) => Value::Float(-f),
                    (UnOp::Not, Value::Bool(b)) => Value::Bool(!b),
                    (_, v) => v,
                })
            }
            Expr::Binary { op, lhs, rhs } => self.binary(*op, lhs, rhs, scope, depth),
            Expr::Ternary { cond, then, els } => {
                if truthy(&self.eval(cond, scope, depth)?) {
                    self.eval(then, scope, depth)
                } else {
                    self.eval(els, scope, depth)
                }
            }
            Expr::If { cond, then, els } => {
                if truthy(&self.eval(cond, scope, depth)?) {
                    self.block(then, scope, depth)
                } else if let Some(e) = els {
                    self.block(e, scope, depth)
                } else {
                    Ok(Value::Unit)
                }
            }
            Expr::Block(stmts) => self.block(stmts, scope, depth),
            Expr::Match { scrut, arms } => {
                let v = self.eval(scrut, scope, depth)?;
                for arm in arms {
                    let base = scope.len();
                    if self.matches(&arm.pat, &v, scope) {
                        let ok = match &arm.guard {
                            Some(g) => truthy(&self.eval(g, scope, depth)?),
                            None => true,
                        };
                        if ok {
                            let r = self.eval(&arm.body, scope, depth);
                            scope.truncate(base);
                            return r;
                        }
                    }
                    scope.truncate(base);
                }
                Ok(Value::Unit)
            }
            Expr::While { cond, body } => {
                while truthy(&self.eval(cond, scope, depth)?) {
                    self.tick()?;
                    match self.block(body, scope, depth) {
                        Ok(_) => {}
                        Err(Signal::Break) => break,
                        Err(Signal::Continue) => continue,
                        Err(e) => return Err(e),
                    }
                }
                Ok(Value::Unit)
            }
            Expr::For { pat, iter, body } => {
                let it = self.eval(iter, scope, depth)?;
                if let Value::List(xs) = it {
                    for x in xs {
                        self.tick()?;
                        let base = scope.len();
                        self.matches(pat, &x, scope);
                        match self.block(body, scope, depth) {
                            Ok(_) => {}
                            Err(Signal::Break) => {
                                scope.truncate(base);
                                break;
                            }
                            Err(Signal::Continue) => {
                                scope.truncate(base);
                                continue;
                            }
                            Err(e) => return Err(e),
                        }
                        scope.truncate(base);
                    }
                }
                Ok(Value::Unit)
            }
            Expr::Break => Err(Signal::Break),
            Expr::Continue => Err(Signal::Continue),
            Expr::With { base, fields } => {
                let mut b = self.eval(base, scope, depth)?;
                if let Value::Record(fs) = &mut b {
                    for f in fields {
                        let (name, val) = match f {
                            Field::Value { name, value } => (name.clone(), self.eval(value, scope, depth)?),
                            Field::Shorthand(n) => (n.clone(), self.eval(&Expr::Ident(n.clone()), scope, depth)?),
                            _ => continue,
                        };
                        if let Some(slot) = fs.iter_mut().find(|(k, _)| *k == name) {
                            slot.1 = val;
                        } else {
                            fs.push((name, val));
                        }
                    }
                }
                Ok(b)
            }
            Expr::Assign { target, value } => {
                let v = self.eval(value, scope, depth)?;
                self.assign(target, v.clone(), scope, depth)?;
                Ok(v)
            }
            Expr::Call { callee, args } => self.call(callee, args, scope, depth),
            Expr::Try(x) => self.eval(x, scope, depth),
            Expr::Fail(m) => {
                let msg = display(&self.eval(m, scope, depth)?);
                Err(Signal::Fail(msg))
            }
            Expr::Reify(x) => match self.eval(x, scope, depth) {
                Ok(_) => Ok(Value::Str(String::new())),
                Err(Signal::Fail(m)) => Ok(Value::Str(m)),
                Err(e) => Err(e),
            },
            Expr::Lambda { .. } => Ok(Value::Unit),
        }
    }

    fn binary(&mut self, op: BinOp, lhs: &Expr, rhs: &Expr, scope: &mut Scope, depth: u64) -> Eval {
        // short-circuit boolean operators
        if matches!(op, BinOp::And | BinOp::Or) {
            let l = truthy(&self.eval(lhs, scope, depth)?);
            return match op {
                BinOp::And => {
                    if !l {
                        Ok(Value::Bool(false))
                    } else {
                        Ok(Value::Bool(truthy(&self.eval(rhs, scope, depth)?)))
                    }
                }
                _ => {
                    if l {
                        Ok(Value::Bool(true))
                    } else {
                        Ok(Value::Bool(truthy(&self.eval(rhs, scope, depth)?)))
                    }
                }
            };
        }
        let l = self.eval(lhs, scope, depth)?;
        let r = self.eval(rhs, scope, depth)?;

        // operator overloading: a user function named for the operator, when the
        // left operand is a record or variant.
        if matches!(l, Value::Record(_) | Value::Variant { .. }) {
            if let Some(name) = overload_name(op) {
                if let Some(f) = self.fns.get(name).copied() {
                    return self.call_fn(f, vec![l, r], depth + 1);
                }
            }
        }

        Ok(match op {
            BinOp::Add | BinOp::Sub | BinOp::Mul | BinOp::Div | BinOp::Mod | BinOp::Shl | BinOp::Shr => {
                arith(op, &l, &r)
            }
            BinOp::Concat => match (&l, &r) {
                (Value::List(a), Value::List(b)) => {
                    let mut v = a.clone();
                    v.extend(b.clone());
                    Value::List(v)
                }
                _ => Value::Str(format!("{}{}", display(&l), display(&r))),
            },
            BinOp::Eq => Value::Bool(equal(&l, &r)),
            BinOp::Ne => Value::Bool(!equal(&l, &r)),
            BinOp::Lt | BinOp::Gt | BinOp::Le | BinOp::Ge => compare(op, &l, &r),
            _ => Value::Unit,
        })
    }

    fn call(&mut self, callee: &Expr, args: &[Arg], scope: &mut Scope, depth: u64) -> Eval {
        // UFCS: `recv.method(a)` → `method(recv, a)`
        if let Expr::Field { base, name } = callee {
            let recv = self.eval(base, scope, depth)?;
            let mut vals = vec![recv];
            for a in args {
                vals.push(self.eval(arg_expr(a), scope, depth)?);
            }
            return self.apply_named(name, vals, depth);
        }
        if let Expr::Ident(name) = callee {
            let mut vals = Vec::with_capacity(args.len());
            for a in args {
                vals.push(self.eval(arg_expr(a), scope, depth)?);
            }
            return self.apply_named(name, vals, depth);
        }
        Ok(Value::Unit)
    }

    fn apply_named(&mut self, name: &str, vals: Vec<Value>, depth: u64) -> Eval {
        // builtins
        match name {
            "info" | "warn" | "err" | "notice" | "debug" | "crit" | "alert" | "emerg" | "panic" => {
                let s = vals.first().map(display).unwrap_or_default();
                self.out.push_str(&s);
                self.out.push('\n');
                return Ok(Value::Unit);
            }
            "print" => {
                let s = vals.first().map(display).unwrap_or_default();
                self.out.push_str(&s);
                return Ok(Value::Unit);
            }
            "len" => {
                return Ok(Value::Int(match vals.first() {
                    Some(Value::List(xs)) => xs.len() as i64,
                    Some(Value::Str(s)) => s.chars().count() as i64,
                    _ => 0,
                }));
            }
            "str" => return Ok(Value::Str(vals.first().map(display).unwrap_or_default())),
            "int" => {
                return Ok(Value::Int(match vals.first() {
                    Some(Value::Int(n)) => *n,
                    Some(Value::Float(f)) => *f as i64,
                    Some(Value::Str(s)) => s.trim().parse().unwrap_or(0),
                    Some(Value::Bool(b)) => *b as i64,
                    _ => 0,
                }));
            }
            "float" => {
                return Ok(Value::Float(match vals.first() {
                    Some(Value::Float(f)) => *f,
                    Some(Value::Int(n)) => *n as f64,
                    Some(Value::Str(s)) => s.trim().parse().unwrap_or(0.0),
                    _ => 0.0,
                }));
            }
            // string stdlib (UFCS on `str`) — byte-oriented, mirroring the C
            // backend so the playground and native builds agree on ASCII text.
            "trim" => return Ok(Value::Str(str_of(vals.first()).trim().to_string())),
            "upper" => return Ok(Value::Str(str_of(vals.first()).to_uppercase())),
            "lower" => return Ok(Value::Str(str_of(vals.first()).to_lowercase())),
            "contains" => {
                return Ok(Value::Bool(str_of(vals.first()).contains(&str_of(vals.get(1)))));
            }
            "starts_with" => {
                return Ok(Value::Bool(str_of(vals.first()).starts_with(&str_of(vals.get(1)))));
            }
            "ends_with" => {
                return Ok(Value::Bool(str_of(vals.first()).ends_with(&str_of(vals.get(1)))));
            }
            "index_of" => {
                let (h, n) = (str_of(vals.first()), str_of(vals.get(1)));
                return Ok(Value::Int(h.find(&n).map(|b| b as i64).unwrap_or(-1)));
            }
            "replace" => {
                let (s, from, to) = (str_of(vals.first()), str_of(vals.get(1)), str_of(vals.get(2)));
                let out = if from.is_empty() { s } else { s.replace(&from, &to) };
                return Ok(Value::Str(out));
            }
            "substr" => {
                let s = str_of(vals.first());
                let start = int_of(vals.get(1));
                let len = int_of(vals.get(2));
                return Ok(Value::Str(byte_substr(&s, start, len)));
            }
            "split" => {
                let (s, sep) = (str_of(vals.first()), str_of(vals.get(1)));
                let parts: Vec<Value> = if sep.is_empty() {
                    vec![Value::Str(s)]
                } else {
                    s.split(&sep).map(|p| Value::Str(p.to_string())).collect()
                };
                return Ok(Value::List(parts));
            }
            "join" => {
                let sep = str_of(vals.get(1));
                if let Some(Value::List(xs)) = vals.first() {
                    let joined = xs.iter().map(display).collect::<Vec<_>>().join(&sep);
                    return Ok(Value::Str(joined));
                }
                return Ok(Value::Str(String::new()));
            }
            _ => {}
        }
        // sum constructor
        if let Some(&arity) = self.variants.get(name) {
            let _ = arity;
            return Ok(Value::Variant { tag: name.to_string(), payload: vals });
        }
        // user function
        if let Some(f) = self.fns.get(name).copied() {
            return self.call_fn(f, vals, depth + 1);
        }
        Ok(Value::Unit)
    }

    /// Try to bind `pat` against `v`, pushing bindings onto `scope`. Returns
    /// whether it matched (partial bindings on a non-match are truncated by the
    /// caller via `scope` base).
    fn matches(&mut self, pat: &Pattern, v: &Value, scope: &mut Scope) -> bool {
        match pat {
            Pattern::Wild => true,
            Pattern::Bind(n) => {
                // a bare capitalized name may be a nullary variant tag
                if self.variants.get(n) == Some(&0) {
                    matches!(v, Value::Variant { tag, .. } if tag == n)
                } else {
                    scope.push((n.clone(), v.clone()));
                    true
                }
            }
            Pattern::Int(n) => matches!(v, Value::Int(m) if m == n),
            Pattern::Float(f) => matches!(v, Value::Float(g) if g == f),
            Pattern::Bool(b) => matches!(v, Value::Bool(c) if c == b),
            Pattern::Str(s) => matches!(v, Value::Str(t) if t == s),
            Pattern::Or(alts) => alts.iter().any(|p| self.matches(p, v, scope)),
            Pattern::Ctor { name, args } => match v {
                Value::Variant { tag, payload } if tag == name => {
                    if args.len() != payload.len() {
                        return false;
                    }
                    args.iter().zip(payload).all(|(p, pv)| self.matches(p, pv, scope))
                }
                _ => false,
            },
            Pattern::Record(fields) => match v {
                Value::Record(fs) => {
                    for (fname, sub) in fields {
                        let fv = fs.iter().find(|(k, _)| k == fname).map(|(_, v)| v.clone()).unwrap_or(Value::Unit);
                        match sub {
                            None => scope.push((fname.clone(), fv)),
                            Some(p) => {
                                if !self.matches(p, &fv, scope) {
                                    return false;
                                }
                            }
                        }
                    }
                    true
                }
                _ => false,
            },
            Pattern::List { elems, rest } => match v {
                Value::List(xs) => {
                    if rest.is_none() && xs.len() != elems.len() {
                        return false;
                    }
                    if rest.is_some() && xs.len() < elems.len() {
                        return false;
                    }
                    for (p, x) in elems.iter().zip(xs.iter()) {
                        if !self.matches(p, x, scope) {
                            return false;
                        }
                    }
                    if let Some(r) = rest {
                        let tail = Value::List(xs[elems.len()..].to_vec());
                        if !self.matches(r, &tail, scope) {
                            return false;
                        }
                    }
                    true
                }
                _ => false,
            },
        }
    }
}

fn arith(op: BinOp, l: &Value, r: &Value) -> Value {
    if let (Value::Float(_), _) | (_, Value::Float(_)) = (l, r) {
        let a = to_f64(l);
        let b = to_f64(r);
        return Value::Float(match op {
            BinOp::Add => a + b,
            BinOp::Sub => a - b,
            BinOp::Mul => a * b,
            BinOp::Div => {
                if b == 0.0 {
                    0.0
                } else {
                    a / b
                }
            }
            _ => a,
        });
    }
    let a = to_i64(l);
    let b = to_i64(r);
    Value::Int(match op {
        BinOp::Add => a.wrapping_add(b),
        BinOp::Sub => a.wrapping_sub(b),
        BinOp::Mul => a.wrapping_mul(b),
        BinOp::Div => {
            if b == 0 {
                0
            } else {
                a.wrapping_div(b)
            }
        }
        BinOp::Mod => {
            if b == 0 {
                0
            } else {
                a.wrapping_rem(b)
            }
        }
        BinOp::Shl => a.wrapping_shl(b as u32),
        BinOp::Shr => a.wrapping_shr(b as u32),
        _ => a,
    })
}

fn compare(op: BinOp, l: &Value, r: &Value) -> Value {
    let ord = match (l, r) {
        (Value::Str(a), Value::Str(b)) => a.cmp(b),
        _ => to_f64(l).partial_cmp(&to_f64(r)).unwrap_or(std::cmp::Ordering::Equal),
    };
    use std::cmp::Ordering::*;
    Value::Bool(match op {
        BinOp::Lt => ord == Less,
        BinOp::Gt => ord == Greater,
        BinOp::Le => ord != Greater,
        BinOp::Ge => ord != Less,
        _ => false,
    })
}

fn equal(l: &Value, r: &Value) -> bool {
    match (l, r) {
        (Value::Int(a), Value::Int(b)) => a == b,
        (Value::Float(a), Value::Float(b)) => a == b,
        (Value::Bool(a), Value::Bool(b)) => a == b,
        (Value::Str(a), Value::Str(b)) => a == b,
        (Value::Unit, Value::Unit) => true,
        (Value::List(a), Value::List(b)) => a.len() == b.len() && a.iter().zip(b).all(|(x, y)| equal(x, y)),
        (Value::Record(a), Value::Record(b)) => {
            a.len() == b.len()
                && a.iter().all(|(k, v)| b.iter().find(|(k2, _)| k2 == k).is_some_and(|(_, v2)| equal(v, v2)))
        }
        (Value::Variant { tag: t1, payload: p1 }, Value::Variant { tag: t2, payload: p2 }) => {
            t1 == t2 && p1.len() == p2.len() && p1.iter().zip(p2).all(|(x, y)| equal(x, y))
        }
        _ => false,
    }
}

fn truthy(v: &Value) -> bool {
    matches!(v, Value::Bool(true)) || matches!(v, Value::Int(n) if *n != 0)
}
fn to_i64(v: &Value) -> i64 {
    match v {
        Value::Int(n) => *n,
        Value::Float(f) => *f as i64,
        Value::Bool(b) => *b as i64,
        _ => 0,
    }
}
fn to_f64(v: &Value) -> f64 {
    match v {
        Value::Float(f) => *f,
        Value::Int(n) => *n as f64,
        _ => 0.0,
    }
}

/// Render a value the way `str(...)` / string interpolation would.
/// A `Value` as its string form, for the string-stdlib builtins (`str`
/// receivers/args stringify; non-strings fall back to their display form).
fn str_of(v: Option<&Value>) -> String {
    v.map(display).unwrap_or_default()
}

/// An `int` argument for the string-stdlib builtins (0 if absent/non-int).
fn int_of(v: Option<&Value>) -> i64 {
    match v {
        Some(Value::Int(n)) => *n,
        Some(Value::Float(f)) => *f as i64,
        _ => 0,
    }
}

/// `substr` over a byte range, clamped and snapped to char boundaries so it
/// never panics on multibyte input (exact byte semantics for ASCII, matching C).
fn byte_substr(s: &str, start: i64, len: i64) -> String {
    let n = s.len() as i64;
    let start = start.clamp(0, n);
    let len = len.max(0).min(n - start);
    let (mut a, mut e) = (start as usize, (start + len) as usize);
    while a < s.len() && !s.is_char_boundary(a) {
        a += 1;
    }
    while e < s.len() && !s.is_char_boundary(e) {
        e += 1;
    }
    s[a..e.max(a)].to_string()
}

fn display(v: &Value) -> String {
    match v {
        Value::Int(n) => n.to_string(),
        Value::Float(f) => {
            if f.fract() == 0.0 && f.is_finite() {
                format!("{f:.1}")
            } else {
                format!("{f}")
            }
        }
        Value::Bool(b) => b.to_string(),
        Value::Str(s) => s.clone(),
        Value::Unit => String::new(),
        Value::List(xs) => {
            let mut s = String::from("[");
            for (i, x) in xs.iter().enumerate() {
                if i > 0 {
                    s.push_str(", ");
                }
                s.push_str(&display(x));
            }
            s.push(']');
            s
        }
        Value::Record(fs) => {
            let mut s = String::from("{ ");
            for (i, (k, v)) in fs.iter().enumerate() {
                if i > 0 {
                    s.push_str(", ");
                }
                let _ = write!(s, "{k} = {}", display(v));
            }
            s.push_str(" }");
            s
        }
        Value::Variant { tag, payload } => {
            if payload.is_empty() {
                tag.clone()
            } else {
                let mut s = format!("{tag}(");
                for (i, p) in payload.iter().enumerate() {
                    if i > 0 {
                        s.push_str(", ");
                    }
                    s.push_str(&display(p));
                }
                s.push(')');
                s
            }
        }
    }
}

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

fn arg_expr(a: &Arg) -> &Expr {
    match a {
        Arg::Pos(e) | Arg::Named { value: e, .. } | Arg::Directive { value: e, .. } => e,
    }
}

/// Variant name → arity for a `A | B(int) | C(int, int)` sum declaration.
/// Returns `None` when `value` isn't a sum type.
fn sum_variants(value: &Expr) -> Option<Vec<(String, usize)>> {
    fn collect(e: &Expr, out: &mut Vec<(String, usize)>) -> bool {
        match e {
            Expr::Binary { op: BinOp::Union | BinOp::Or, lhs, rhs } => {
                collect(lhs, out) && collect(rhs, out)
            }
            Expr::Ident(n) if n.chars().next().is_some_and(|c| c.is_uppercase()) => {
                out.push((n.clone(), 0));
                true
            }
            Expr::Call { callee, args } => {
                if let Expr::Ident(n) = callee.as_ref() {
                    if n.chars().next().is_some_and(|c| c.is_uppercase()) {
                        out.push((n.clone(), args.len()));
                        return true;
                    }
                }
                false
            }
            _ => false,
        }
    }
    let mut out = Vec::new();
    if collect(value, &mut out) && out.len() >= 2 {
        Some(out)
    } else {
        None
    }
}
