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
    Variant {
        tag: String,
        payload: Vec<Value>,
    },
    /// A closure: a lambda plus the scope it captured at creation time.
    Closure {
        params: Vec<Param>,
        body: Box<Expr>,
        captured: Vec<(String, Value)>,
    },
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
                    if let Expr::Ident(_) = &b.target
                        && let Some(vs) = sum_variants(&b.value)
                    {
                        for (name, arity) in vs {
                            variants.insert(name, arity);
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
            return Err(Signal::Limit(
                "execution step limit reached — possible infinite loop".into(),
            ));
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
            *self
                .edges
                .entry(caller)
                .or_default()
                .entry(f.name.clone())
                .or_default() += incl;
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

    fn assign(
        &mut self,
        target: &Expr,
        v: Value,
        scope: &mut Scope,
        depth: u64,
    ) -> Result<(), Signal> {
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
                if let Some(Value::List(xs)) = loc
                    && i >= 0
                    && (i as usize) < xs.len()
                {
                    xs[i as usize] = v;
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
    fn lvalue<'s>(
        &mut self,
        target: &Expr,
        scope: &'s mut Scope,
        depth: u64,
    ) -> Result<Option<&'s mut Value>, Signal> {
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
                    Ok(Value::Variant {
                        tag: n.clone(),
                        payload: vec![],
                    })
                } else if let Some(f) = self.fns.get(n).copied() {
                    // a top-level function referenced by name is a function
                    // value → a closure over its definition (mirrors the C
                    // backend, so higher-order params work in the playground).
                    let body = match &f.body {
                        Some(FnBody::Expr(e)) => (**e).clone(),
                        Some(FnBody::Block(stmts)) => Expr::Block(stmts.clone()),
                        None => Expr::Unit,
                    };
                    Ok(Value::Closure {
                        params: f.params.clone(),
                        body: Box::new(body),
                        captured: vec![],
                    })
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
                    Value::Record(fs) => fs
                        .into_iter()
                        .find(|(k, _)| k == name)
                        .map(|(_, v)| v)
                        .unwrap_or(Value::Unit),
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
                    Value::Str(s) => s
                        .chars()
                        .nth(i.max(0) as usize)
                        .map(|c| Value::Str(c.to_string()))
                        .unwrap_or(Value::Str(String::new())),
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
                            Field::Value { name, value } => {
                                (name.clone(), self.eval(value, scope, depth)?)
                            }
                            Field::Shorthand(n) => {
                                (n.clone(), self.eval(&Expr::Ident(n.clone()), scope, depth)?)
                            }
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
            // The playground interpreter is single-threaded, so colorblind async
            // runs eagerly: `spawn e` computes `e` now and `await` is a no-op on
            // the value. Results match the concurrent runtime; only interleaving
            // (which the interpreter doesn't model) differs.
            Expr::Spawn(x) | Expr::Await(x) => self.eval(x, scope, depth),
            // a lambda captures its enclosing scope by value
            Expr::Lambda { params, body, .. } => Ok(Value::Closure {
                params: params.clone(),
                body: Box::new((**body).clone()),
                captured: scope.clone(),
            }),
        }
    }

    /// Apply a closure value to arguments (used by first-class calls and the
    /// higher-order list methods).
    fn call_closure(&mut self, c: &Value, args: Vec<Value>, depth: u64) -> Eval {
        let Value::Closure {
            params,
            body,
            captured,
        } = c
        else {
            return Ok(Value::Unit);
        };
        if depth > DEPTH_LIMIT {
            return Err(Signal::Limit("recursion too deep".into()));
        }
        let mut sc: Scope = captured.clone();
        for (i, p) in params.iter().enumerate() {
            sc.push((p.name.clone(), args.get(i).cloned().unwrap_or(Value::Unit)));
        }
        self.eval(body, &mut sc, depth + 1)
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
        if matches!(l, Value::Record(_) | Value::Variant { .. })
            && let Some(name) = overload_name(op)
            && let Some(f) = self.fns.get(name).copied()
        {
            return self.call_fn(f, vec![l, r], depth + 1);
        }

        Ok(match op {
            BinOp::Add
            | BinOp::Sub
            | BinOp::Mul
            | BinOp::Div
            | BinOp::Mod
            | BinOp::Shl
            | BinOp::Shr => arith(op, &l, &r),
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
            // a local holding a closure value: `f = v => …; f(x)`
            if let Some((_, c @ Value::Closure { .. })) =
                scope.iter().rev().find(|(k, _)| k == name)
            {
                let c = c.clone();
                return self.call_closure(&c, vals, depth);
            }
            return self.apply_named(name, vals, depth);
        }
        Ok(Value::Unit)
    }

    /// Higher-order list methods: the receiver is `vals[0]` (a List) and the
    /// callback/value is `vals[1]`. Returns `None` if `name` isn't one.
    fn list_method(&mut self, name: &str, vals: &[Value], depth: u64) -> Option<Eval> {
        let list = match vals.first() {
            Some(Value::List(xs)) => xs.clone(),
            _ => return None,
        };
        let f = vals.get(1).cloned();
        let run = |me: &mut Self, c: &Value, x: Value| me.call_closure(c, vec![x], depth);
        let out: Eval = match name {
            "map" => {
                let Some(c) = &f else {
                    return Some(Ok(Value::List(list)));
                };
                let mut r = Vec::with_capacity(list.len());
                for x in list {
                    match run(self, c, x) {
                        Ok(v) => r.push(v),
                        Err(e) => return Some(Err(e)),
                    }
                }
                Ok(Value::List(r))
            }
            "filter" => {
                let Some(c) = &f else {
                    return Some(Ok(Value::List(list)));
                };
                let mut r = Vec::new();
                for x in list {
                    match run(self, c, x.clone()) {
                        Ok(v) => {
                            if truthy(&v) {
                                r.push(x);
                            }
                        }
                        Err(e) => return Some(Err(e)),
                    }
                }
                Ok(Value::List(r))
            }
            "reduce" | "fold" => {
                let mut acc = f.clone().unwrap_or(Value::Unit);
                let Some(c) = vals.get(2).cloned() else {
                    return Some(Ok(acc));
                };
                for x in list {
                    match self.call_closure(&c, vec![acc.clone(), x], depth) {
                        Ok(v) => acc = v,
                        Err(e) => return Some(Err(e)),
                    }
                }
                Ok(acc)
            }
            "sort" => {
                let mut r = list;
                r.sort_by(cmp_values);
                Ok(Value::List(r))
            }
            "reverse" => {
                let mut r = list;
                r.reverse();
                Ok(Value::List(r))
            }
            "push" => {
                let mut r = list;
                if let Some(x) = f {
                    r.push(x);
                }
                Ok(Value::List(r))
            }
            "pop" => {
                let mut r = list;
                r.pop();
                Ok(Value::List(r))
            }
            "contains" => Ok(Value::Bool(
                f.map(|x| list.iter().any(|e| equal(e, &x)))
                    .unwrap_or(false),
            )),
            "index_of" => Ok(Value::Int(
                f.and_then(|x| list.iter().position(|e| equal(e, &x)))
                    .map(|i| i as i64)
                    .unwrap_or(-1),
            )),
            "sum" => Ok(fold_num(&list, 0.0, |a, b| a + b)),
            "min" => Ok(fold_minmax(&list, true)),
            "max" => Ok(fold_minmax(&list, false)),
            "first" => Ok(list.first().cloned().unwrap_or(Value::Unit)),
            "last" => Ok(list.last().cloned().unwrap_or(Value::Unit)),
            // index-walk primitives, mirroring the C backend (used by the
            // self-hosted lexer/parser).
            "length" => Ok(Value::Int(list.len() as i64)),
            "get" => {
                let i = int_of(vals.get(1));
                Ok(if i >= 0 && (i as usize) < list.len() {
                    list[i as usize].clone()
                } else {
                    Value::Unit
                })
            }
            "slice" => {
                let from = int_of(vals.get(1)).max(0) as usize;
                let to = (int_of(vals.get(2)).max(0) as usize).min(list.len());
                let from = from.min(to);
                Ok(Value::List(list[from..to].to_vec()))
            }
            _ => return None,
        };
        Some(out)
    }

    fn apply_named(&mut self, name: &str, vals: Vec<Value>, depth: u64) -> Eval {
        // higher-order + list methods (UFCS on a List receiver)
        if let Some(res) = self.list_method(name, &vals, depth) {
            return res;
        }
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
            // A byte and the string holding it. Bytes, not code points: a
            // Maca string is bytes on every other target, and the playground
            // agreeing about that is the point of it being the same language.
            "chr" => {
                let b = match vals.first() {
                    Some(Value::Int(n)) if *n > 0 => (*n & 0xFF) as u8,
                    _ => return Ok(Value::Str(String::new())),
                };
                return Ok(Value::Str((b as char).to_string()));
            }
            "ord" => {
                return Ok(Value::Int(match vals.first() {
                    Some(Value::Str(s)) => s.as_bytes().first().map_or(-1, |b| *b as i64),
                    _ => -1,
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
            "repeat" => {
                let n = int_of(vals.get(1)).max(0) as usize;
                return Ok(Value::Str(str_of(vals.first()).repeat(n)));
            }
            // pad to a width with an optional pad string (default a space); a
            // string already at least that wide is returned unchanged.
            "pad_start" | "pad_end" => {
                let s = str_of(vals.first());
                let w = int_of(vals.get(1)).max(0) as usize;
                let p = vals
                    .get(2)
                    .map(|v| str_of(Some(v)))
                    .unwrap_or_else(|| " ".into());
                let p = if p.is_empty() { " ".to_string() } else { p };
                if s.chars().count() >= w {
                    return Ok(Value::Str(s));
                }
                let fill: String = p.chars().cycle().take(w - s.chars().count()).collect();
                return Ok(Value::Str(if name == "pad_start" {
                    fill + &s
                } else {
                    s + &fill
                }));
            }
            // centre within a width; an odd remainder goes on the right, so a
            // column of centred cells stays flush left.
            "pad_center" => {
                let s = str_of(vals.first());
                let w = int_of(vals.get(1)).max(0) as usize;
                let p = vals
                    .get(2)
                    .map(|v| str_of(Some(v)))
                    .unwrap_or_else(|| " ".into());
                let p = if p.is_empty() { " ".to_string() } else { p };
                let n = s.chars().count();
                if n >= w {
                    return Ok(Value::Str(s));
                }
                let left: String = p.chars().cycle().take((w - n) / 2).collect();
                let right: String = p.chars().cycle().take(w - n - (w - n) / 2).collect();
                return Ok(Value::Str(left + &s + &right));
            }
            // `x.fixed(n)` — n decimal places. Accepts an int receiver too, so
            // `{n:.2}` works on any number.
            "fixed" => {
                let x = float_of(vals.first());
                let n = int_of(vals.get(1)).clamp(0, 17) as usize;
                return Ok(Value::Str(format!("{x:.n$}")));
            }
            "contains" => {
                return Ok(Value::Bool(
                    str_of(vals.first()).contains(&str_of(vals.get(1))),
                ));
            }
            "starts_with" => {
                return Ok(Value::Bool(
                    str_of(vals.first()).starts_with(&str_of(vals.get(1))),
                ));
            }
            "ends_with" => {
                return Ok(Value::Bool(
                    str_of(vals.first()).ends_with(&str_of(vals.get(1))),
                ));
            }
            "index_of" => {
                let (h, n) = (str_of(vals.first()), str_of(vals.get(1)));
                return Ok(Value::Int(h.find(&n).map(|b| b as i64).unwrap_or(-1)));
            }
            "replace" => {
                let (s, from, to) = (
                    str_of(vals.first()),
                    str_of(vals.get(1)),
                    str_of(vals.get(2)),
                );
                let out = if from.is_empty() {
                    s
                } else {
                    s.replace(&from, &to)
                };
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
            // byte-length, per-byte access, and character classes — the scanner
            // primitives (byte-oriented to match the C runtime on ASCII source).
            "length" => return Ok(Value::Int(str_of(vals.first()).len() as i64)),
            "at" => {
                let s = str_of(vals.first());
                return Ok(Value::Str(byte_substr(&s, int_of(vals.get(1)), 1)));
            }
            "chars" => {
                let s = str_of(vals.first());
                let cs = s
                    .bytes()
                    .map(|b| Value::Str((b as char).to_string()))
                    .collect();
                return Ok(Value::List(cs));
            }
            "is_whitespace" => {
                let b = str_of(vals.first()).as_bytes().first().copied();
                return Ok(Value::Bool(
                    b.map(|c| c.is_ascii_whitespace()).unwrap_or(false),
                ));
            }
            "is_ascii_digit" => {
                let b = str_of(vals.first()).as_bytes().first().copied();
                return Ok(Value::Bool(b.map(|c| c.is_ascii_digit()).unwrap_or(false)));
            }
            "is_alpha" => {
                let b = str_of(vals.first()).as_bytes().first().copied();
                return Ok(Value::Bool(
                    b.map(|c| c.is_ascii_alphabetic()).unwrap_or(false),
                ));
            }
            // async suspension point — a no-op in the synchronous playground
            // interpreter (results match; only real-time waiting is elided).
            "sleep_ms" => return Ok(Value::Unit),
            // math prelude
            "sqrt" => return Ok(Value::Float(to_f64(&vals[0]).sqrt())),
            "floor" => return Ok(Value::Float(to_f64(&vals[0]).floor())),
            "ceil" => return Ok(Value::Float(to_f64(&vals[0]).ceil())),
            "round" => return Ok(Value::Float(to_f64(&vals[0]).round())),
            "sin" => return Ok(Value::Float(to_f64(&vals[0]).sin())),
            "cos" => return Ok(Value::Float(to_f64(&vals[0]).cos())),
            "tan" => return Ok(Value::Float(to_f64(&vals[0]).tan())),
            "log" => return Ok(Value::Float(to_f64(&vals[0]).ln())),
            "exp" => return Ok(Value::Float(to_f64(&vals[0]).exp())),
            "pow" => return Ok(Value::Float(to_f64(&vals[0]).powf(to_f64(&vals[1])))),
            "abs" => {
                return Ok(match vals.first() {
                    Some(Value::Float(f)) => Value::Float(f.abs()),
                    v => Value::Int(v.map(to_i64).unwrap_or(0).abs()),
                });
            }
            "min" => {
                return Ok(
                    if cmp_values(&vals[0], &vals[1]) == std::cmp::Ordering::Greater {
                        vals[1].clone()
                    } else {
                        vals[0].clone()
                    },
                );
            }
            "max" => {
                return Ok(
                    if cmp_values(&vals[0], &vals[1]) == std::cmp::Ordering::Less {
                        vals[1].clone()
                    } else {
                        vals[0].clone()
                    },
                );
            }
            "clamp" => {
                let (x, lo, hi) = (&vals[0], &vals[1], &vals[2]);
                return Ok(if cmp_values(x, lo) == std::cmp::Ordering::Less {
                    lo.clone()
                } else if cmp_values(x, hi) == std::cmp::Ordering::Greater {
                    hi.clone()
                } else {
                    x.clone()
                });
            }
            "sign" => {
                let x = to_f64(&vals[0]);
                return Ok(Value::Int(if x > 0.0 {
                    1
                } else if x < 0.0 {
                    -1
                } else {
                    0
                }));
            }
            "gcd" => {
                let (mut a, mut b) = (to_i64(&vals[0]).abs(), to_i64(&vals[1]).abs());
                while b != 0 {
                    let t = a % b;
                    a = b;
                    b = t;
                }
                return Ok(Value::Int(a));
            }
            _ => {}
        }
        // sum constructor
        if let Some(&arity) = self.variants.get(name) {
            let _ = arity;
            return Ok(Value::Variant {
                tag: name.to_string(),
                payload: vals,
            });
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
                    args.iter()
                        .zip(payload)
                        .all(|(p, pv)| self.matches(p, pv, scope))
                }
                _ => false,
            },
            Pattern::Record(fields) => match v {
                Value::Record(fs) => {
                    for (fname, sub) in fields {
                        let fv = fs
                            .iter()
                            .find(|(k, _)| k == fname)
                            .map(|(_, v)| v.clone())
                            .unwrap_or(Value::Unit);
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
        _ => to_f64(l)
            .partial_cmp(&to_f64(r))
            .unwrap_or(std::cmp::Ordering::Equal),
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

/// Total order for `.sort()` — numeric by value, strings lexicographically.
fn cmp_values(a: &Value, b: &Value) -> std::cmp::Ordering {
    match (a, b) {
        (Value::Str(x), Value::Str(y)) => x.cmp(y),
        _ => to_f64(a)
            .partial_cmp(&to_f64(b))
            .unwrap_or(std::cmp::Ordering::Equal),
    }
}

/// `.sum()` — Int if every element is an Int, else Float.
fn fold_num(xs: &[Value], init: f64, op: fn(f64, f64) -> f64) -> Value {
    let all_int = xs.iter().all(|v| matches!(v, Value::Int(_)));
    let acc = xs.iter().fold(init, |a, v| op(a, to_f64(v)));
    if all_int {
        Value::Int(acc as i64)
    } else {
        Value::Float(acc)
    }
}

/// `.min()` / `.max()` — preserves the element's own type; Unit on empty.
fn fold_minmax(xs: &[Value], is_min: bool) -> Value {
    let mut best: Option<&Value> = None;
    for v in xs {
        best = Some(match best {
            None => v,
            Some(b) => {
                let take = if is_min {
                    cmp_values(v, b) == std::cmp::Ordering::Less
                } else {
                    cmp_values(v, b) == std::cmp::Ordering::Greater
                };
                if take { v } else { b }
            }
        });
    }
    best.cloned().unwrap_or(Value::Unit)
}

fn equal(l: &Value, r: &Value) -> bool {
    match (l, r) {
        (Value::Int(a), Value::Int(b)) => a == b,
        (Value::Float(a), Value::Float(b)) => a == b,
        (Value::Bool(a), Value::Bool(b)) => a == b,
        (Value::Str(a), Value::Str(b)) => a == b,
        (Value::Unit, Value::Unit) => true,
        (Value::List(a), Value::List(b)) => {
            a.len() == b.len() && a.iter().zip(b).all(|(x, y)| equal(x, y))
        }
        (Value::Record(a), Value::Record(b)) => {
            a.len() == b.len()
                && a.iter().all(|(k, v)| {
                    b.iter()
                        .find(|(k2, _)| k2 == k)
                        .is_some_and(|(_, v2)| equal(v, v2))
                })
        }
        (
            Value::Variant {
                tag: t1,
                payload: p1,
            },
            Value::Variant {
                tag: t2,
                payload: p2,
            },
        ) => t1 == t2 && p1.len() == p2.len() && p1.iter().zip(p2).all(|(x, y)| equal(x, y)),
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

/// A `float` argument (0.0 if absent/non-numeric). An int is widened, so
/// `x.fixed(n)` works on any number.
fn float_of(v: Option<&Value>) -> f64 {
    match v {
        Some(Value::Float(f)) => *f,
        Some(Value::Int(n)) => *n as f64,
        _ => 0.0,
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
        Value::Closure { .. } => "<closure>".to_string(),
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

/// Variant name → arity for a `A | B(int) | C(int, int)` sum declaration.
/// Returns `None` when `value` isn't a sum type.
fn sum_variants(value: &Expr) -> Option<Vec<(String, usize)>> {
    fn collect(e: &Expr, out: &mut Vec<(String, usize)>) -> bool {
        match e {
            Expr::Binary {
                op: BinOp::Union | BinOp::Or,
                lhs,
                rhs,
            } => collect(lhs, out) && collect(rhs, out),
            Expr::Ident(n) if n.chars().next().is_some_and(|c| c.is_uppercase()) => {
                out.push((n.clone(), 0));
                true
            }
            Expr::Call { callee, args } => {
                if let Expr::Ident(n) = callee.as_ref()
                    && n.chars().next().is_some_and(|c| c.is_uppercase())
                {
                    out.push((n.clone(), args.len()));
                    return true;
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

#[cfg(test)]
mod tests {
    use super::*;

    fn output(src: &str) -> String {
        let m = maca_parser::parse(src).module;
        run(&m).output
    }

    #[test]
    fn scan_primitives_match_native() {
        // the str/array scan methods the self-hosted lexer uses, exercised
        // through the playground interpreter so it agrees with the C backend.
        let src = "main() -> int {\n\
            s = \"a1 b\"\n\
            cs = s.chars()\n\
            info(\"{s.length()} {cs.length()} {cs.get(0)} {cs.get(1)}\")\n\
            info(\"{cs.get(0).is_alpha()} {cs.get(1).is_ascii_digit()} {cs.get(2).is_whitespace()}\")\n\
            info(\"{cs.slice(0, 2).join(\"\")}\")\n\
            0\n\
        }\n";
        let out = output(src);
        assert!(out.contains("4 4 a 1"), "scan lengths/access wrong: {out}");
        assert!(out.contains("true true true"), "char classes wrong: {out}");
        assert!(out.contains("a1"), "slice+join wrong: {out}");
    }

    #[test]
    fn higher_order_fn_by_name() {
        // a function passed by name to a `pred` parameter and called there.
        let src = "even(n: int) -> bool => n % 2 == 0\n\n\
            count_if(xs: int[], i: int, pred) -> int =>\n\
                i >= xs.length() ? 0 : (pred(xs.get(i)) ? 1 : 0) + count_if(xs, i + 1, pred)\n\n\
            main() -> int {\n\
                xs = 1, 2, 3, 4, 5, 6\n\
                info(\"{count_if(xs, 0, even)}\")\n\
                0\n\
            }\n";
        assert!(output(src).contains('3'), "should count 3 evens");
    }

    #[test]
    fn recursive_record_walks() {
        // a record holding a list of itself runs in the interpreter too.
        let src = "Tree = {\n    label: str\n    kids: Tree[]\n}\n\n\
            leaf(l: str) -> Tree => Tree { label = l, kids = [] }\n\n\
            size(t: Tree) -> int => 1 + ks(t.kids, 0)\n\n\
            ks(xs: Tree[], i: int) -> int =>\n\
                i >= xs.length() ? 0 : size(xs.get(i)) + ks(xs, i + 1)\n\n\
            main() -> int {\n\
                r = Tree { label = \"r\", kids = [leaf(\"a\"), leaf(\"b\")] }\n\
                info(\"{size(r)}\")\n\
                0\n\
            }\n";
        assert!(output(src).contains('3'), "tree size should be 3");
    }
}
