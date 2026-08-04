use maca_parser::ast::*;
use std::collections::{HashMap, HashSet};

/// Runtime helpers that hand back a string of their own making.
const FRESH_METHODS: &[&str] = &[
    "trim",
    "upper",
    "lower",
    "replace",
    "substr",
    "slice",
    "repeat",
    "pad_start",
    "pad_end",
    "pad_center",
    "at",
    "fixed",
    "join",
];

/// Free functions that return a string this runtime just built.
const FRESH_FNS: &[&str] = &[
    "str",
    "chr",
    "read_file",
    "read_line",
    "read_stdin",
    "capture",
    "env",
    "cwd",
    "now_iso",
    "format_time",
    "real_path",
    "fixed",
    "attr",
    "flag",
    "element",
    "styles",
];

/// Calls that read their arguments and keep none of them.
const BORROWING_FNS: &[&str] = &[
    "info",
    "warn",
    "err",
    "notice",
    "crit",
    "alert",
    "emerg",
    "debug",
    "len",
    "int",
    "float",
    "bool",
    "str",
    "ord",
    "chr",
    "assert",
    "assert_eq",
    "write_file",
    "file_exists",
    "is_dir",
    "file_size",
    "modified_ms",
    "remove_file",
    "remove_dir",
    "make_dir",
    "copy_bytes",
    "read_file",
    "real_path",
    "list_dir",
    "exec",
    "chdir",
    "env",
    "attr",
    "flag",
    "element",
    "fixed",
];

/// Methods that read their receiver and arguments and keep neither.
const BORROWING_METHODS: &[&str] = &[
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
    "join",
    "sum",
    "has",
    "keys",
];

/// What the module's own functions do with a string they return.
pub struct Fresh {
    fns: HashSet<String>,
    /// Names the module defines.
    defined: HashSet<String>,
}

impl Fresh {
    /// Work out which functions return a string the caller becomes the only owner of.
    pub fn of(m: &Module) -> Fresh {
        let mut defined = HashSet::new();
        collect_defined(&m.items, &mut defined);
        let bodies = fn_bodies(&m.items);
        let mut fresh = Fresh {
            fns: HashSet::new(),
            defined,
        };
        loop {
            let mut grew = false;
            for (name, params, body) in &bodies {
                if fresh.fns.contains(name) {
                    continue;
                }
                if fresh.body_returns_fresh(params, body) {
                    fresh.fns.insert(name.clone());
                    grew = true;
                }
            }
            if !grew {
                return fresh;
            }
        }
    }

    /// The names the module defines, which shadow every table in this file.
    pub fn defined(&self) -> &HashSet<String> {
        &self.defined
    }

    /// Does `e`, evaluated here, produce a string nothing else is holding?
    pub fn allocates(&self, e: &Expr) -> bool {
        match e {
            Expr::Str(_) | Expr::Path(_) => true,
            Expr::Binary {
                op: BinOp::Concat | BinOp::Div,
                ..
            } => true,
            Expr::Try(inner) | Expr::Reify(inner) => self.allocates(inner),
            Expr::Ternary { then, els, .. } => self.allocates(then) && self.allocates(els),
            Expr::Call { callee, .. } => match callee.as_ref() {
                Expr::Ident(f) if self.defined.contains(f) => self.fns.contains(f),
                Expr::Ident(f) => FRESH_FNS.contains(&f.as_str()),
                Expr::Field { name, .. } if self.defined.contains(name) => self.fns.contains(name),
                Expr::Field { name, .. } => FRESH_METHODS.contains(&name.as_str()),
                _ => false,
            },
            _ => false,
        }
    }

    /// Is every string this body returns one the caller becomes the owner of?
    fn body_returns_fresh(&self, params: &HashSet<String>, body: &FnBody) -> bool {
        match body {
            FnBody::Expr(e) => self.returned(params, e, &[]),
            FnBody::Block(ss) => {
                let kept = self.retained(ss, Tail::Read);
                self.returned_from(params, ss, &kept)
            }
        }
    }

    /// The tail of a statement list, and the tails of any branches it ends in.
    fn returned_from(&self, params: &HashSet<String>, ss: &[Stmt], kept: &HashSet<String>) -> bool {
        let locals = self.fresh_locals(params, ss, kept);
        match ss.last() {
            Some(Stmt::Expr(e)) => self.returned(params, e, &locals),
            _ => false,
        }
    }

    /// Is `e` in return position fresh, given the locals known to be?
    fn returned(&self, params: &HashSet<String>, e: &Expr, locals: &[String]) -> bool {
        match e {
            Expr::Ident(n) => locals.iter().any(|l| l == n),
            Expr::Ternary { then, els, .. } => {
                self.returned(params, then, locals) && self.returned(params, els, locals)
            }
            Expr::Try(inner) | Expr::Reify(inner) => self.returned(params, inner, locals),
            Expr::If { then, els, .. } => {
                let kept = self.retained(then, Tail::Read);
                self.returned_from(params, then, &kept)
                    && els.as_ref().is_some_and(|e| {
                        let kept = self.retained(e, Tail::Read);
                        self.returned_from(params, e, &kept)
                    })
            }
            Expr::Block(ss) => {
                let kept = self.retained(ss, Tail::Read);
                self.returned_from(params, ss, &kept)
            }
            Expr::Match { arms, .. } => arms.iter().all(|a| self.returned(params, &a.body, locals)),
            _ => self.allocates(e),
        }
    }

    /// Locals in `ss` that hold a string nobody else is holding.
    fn fresh_locals(
        &self,
        params: &HashSet<String>,
        ss: &[Stmt],
        kept: &HashSet<String>,
    ) -> Vec<String> {
        let mut binds: HashMap<String, bool> = HashMap::new();
        each_bind(ss, &mut |name, value| {
            if params.contains(name) {
                return;
            }
            let fresh = self.allocates(value);
            binds
                .entry(name.to_string())
                .and_modify(|ok| *ok &= fresh)
                .or_insert(fresh);
        });
        binds
            .into_iter()
            .filter(|(name, ok)| *ok && !kept.contains(name))
            .map(|(name, _)| name)
            .collect()
    }
}

/// Every `name = value` in the subtree, however deeply nested.
pub fn each_bind(ss: &[Stmt], f: &mut impl FnMut(&str, &Expr)) {
    for s in ss {
        if let Stmt::Bind(b) = s
            && let Expr::Ident(n) = &b.target
        {
            f(n, &b.value);
        }
        walk_stmt(s, &mut |e| {
            if let Expr::Assign { target, value } = e
                && let Expr::Ident(n) = target.as_ref()
            {
                f(n, value);
            }
        });
        stmt_blocks(s, &mut |inner| each_bind(inner, f));
    }
}

/// Apply `f` to each statement list nested inside `s`.
fn stmt_blocks(s: &Stmt, f: &mut impl FnMut(&[Stmt])) {
    let mut visit = |e: &Expr| match e {
        Expr::If { then, els, .. } => {
            f(then);
            if let Some(e) = els {
                f(e);
            }
        }
        Expr::For { body, .. } | Expr::While { body, .. } | Expr::Block(body) => f(body),
        Expr::Match { arms, .. } => {
            for a in arms {
                if let Expr::Block(ss) = &a.body {
                    f(ss);
                }
            }
        }
        _ => {}
    };
    match s {
        Stmt::Expr(e) => walk_expr(e, &mut visit),
        Stmt::Bind(b) => walk_expr(&b.value, &mut visit),
        _ => {}
    }
}

/// How the last statement's value is treated.
#[derive(Clone, Copy, PartialEq)]
pub enum Tail {
    /// It leaves the block, so a name there is handed to the caller.
    Flows,
    /// It is only read.
    Read,
}

impl Fresh {
    /// Names whose string may be kept somewhere this statement list cannot see.
    pub fn retained(&self, stmts: &[Stmt], tail: Tail) -> HashSet<String> {
        let mut out = HashSet::new();
        self.retain_stmts(stmts, tail, &mut out);
        out
    }

    /// Does this callee read what it is given without keeping any of it?
    fn borrowing(&self, callee: &Expr) -> bool {
        match callee {
            Expr::Ident(f) => !self.defined.contains(f) && BORROWING_FNS.contains(&f.as_str()),
            Expr::Field { name, .. } => {
                !self.defined.contains(name) && BORROWING_METHODS.contains(&name.as_str())
            }
            _ => false,
        }
    }

    fn retain_stmts(&self, stmts: &[Stmt], tail: Tail, out: &mut HashSet<String>) {
        let n = stmts.len();
        for (i, s) in stmts.iter().enumerate() {
            let last = i + 1 == n;
            match s {
                Stmt::Bind(b) => {
                    self.flows_out(&b.value, out);
                    if !matches!(&b.target, Expr::Ident(_)) {
                        self.reads(&b.target, out);
                    }
                }
                Stmt::Expr(e) if last && tail == Tail::Flows => self.flows_out(e, out),
                Stmt::Expr(e) => self.reads(e, out),
                Stmt::Fn(f) => match &f.body {
                    Some(FnBody::Expr(e)) => keep_all(e, out),
                    Some(FnBody::Block(ss)) => ss
                        .iter()
                        .for_each(|s| walk_stmt(s, &mut |e| keep_all(e, out))),
                    None => {}
                },
                _ => {}
            }
        }
    }

    /// The value leaves this position, so a bare name here is no longer ours.
    fn flows_out(&self, e: &Expr, out: &mut HashSet<String>) {
        match e {
            Expr::Ident(n) => {
                out.insert(n.clone());
            }
            Expr::Ternary { cond, then, els } => {
                self.reads(cond, out);
                self.flows_out(then, out);
                self.flows_out(els, out);
            }
            Expr::Try(inner) | Expr::Reify(inner) => self.flows_out(inner, out),
            _ => self.reads(e, out),
        }
    }

    /// The expression is evaluated and its parts read.
    fn reads(&self, e: &Expr, out: &mut HashSet<String>) {
        match e {
            Expr::Ident(_) | Expr::Int(_) | Expr::Float(_) | Expr::Bool(_) | Expr::Unit => {}
            Expr::Path(_) | Expr::Break | Expr::Continue => {}
            Expr::Str(parts) => {
                for p in parts {
                    if let StrPart::Interp(e) = p {
                        self.reads(e, out);
                    }
                }
            }
            Expr::List(items) => items.iter().for_each(|i| keep_all(i, out)),
            Expr::Record(fs) | Expr::Ctor { fields: fs, .. } => keep_fields(fs, out),
            Expr::With { base, fields } => {
                keep_all(base, out);
                keep_fields(fields, out);
            }
            Expr::Lambda { body, .. } => keep_all(body, out),
            Expr::Spawn(x) | Expr::Await(x) | Expr::Fail(x) => keep_all(x, out),
            Expr::Assign { target, value } => {
                self.reads(target, out);
                keep_all(value, out);
            }
            Expr::Call { callee, args } => {
                let borrows = self.borrowing(callee);
                match callee.as_ref() {
                    Expr::Field { base, .. } if borrows => self.reads(base, out),
                    Expr::Field { base, .. } => keep_all(base, out),
                    other => self.reads(other, out),
                }
                for a in args {
                    if borrows {
                        self.reads(arg_expr(a), out);
                    } else {
                        keep_all(arg_expr(a), out);
                    }
                }
            }
            Expr::Field { base, .. } => self.reads(base, out),
            Expr::Index { base, index } => {
                self.reads(base, out);
                self.reads(index, out);
            }
            Expr::Range { lo, hi } => {
                self.reads(lo, out);
                self.reads(hi, out);
            }
            Expr::Unary { expr, .. } => self.reads(expr, out),
            Expr::Binary { lhs, rhs, .. } => {
                self.reads(lhs, out);
                self.reads(rhs, out);
            }
            Expr::Ternary { cond, then, els } => {
                self.reads(cond, out);
                self.reads(then, out);
                self.reads(els, out);
            }
            Expr::Try(x) | Expr::Reify(x) => self.reads(x, out),
            Expr::Return(v) => {
                if let Some(x) = v {
                    self.flows_out(x, out);
                }
            }
            Expr::If { cond, then, els } => {
                self.reads(cond, out);
                self.retain_stmts(then, Tail::Flows, out);
                if let Some(e) = els {
                    self.retain_stmts(e, Tail::Flows, out);
                }
            }
            Expr::Match { scrut, arms } => {
                self.reads(scrut, out);
                arms.iter().for_each(|a| self.flows_out(&a.body, out));
            }
            Expr::For { iter, body, .. } => {
                self.reads(iter, out);
                self.retain_stmts(body, Tail::Read, out);
            }
            Expr::While { cond, body } => {
                self.reads(cond, out);
                self.retain_stmts(body, Tail::Read, out);
            }
            Expr::Block(ss) => self.retain_stmts(ss, Tail::Flows, out),
        }
    }
}

/// Names this function may append to in place.
pub fn appendable_names(f: &FnDef, defined: &HashSet<String>) -> HashSet<String> {
    let aliased = aliased_names(f, defined);
    let mut own: HashMap<String, bool> = HashMap::new();
    let mut barred: HashSet<String> = f
        .params
        .iter()
        .filter(|p| !p.variadic)
        .map(|p| p.name.clone())
        .collect();
    let Some(body) = &f.body else {
        return HashSet::new();
    };
    let stmts: Vec<Stmt> = match body {
        FnBody::Block(ss) => ss.clone(),
        FnBody::Expr(e) => vec![Stmt::Expr((**e).clone())],
    };
    each_bind(&stmts, &mut |name, value| {
        let fresh = matches!(value, Expr::List(_)) || is_self_push(name, value);
        own.entry(name.to_string())
            .and_modify(|ok| *ok &= fresh)
            .or_insert(fresh);
    });
    for st in &stmts {
        walk_stmt(st, &mut |e| {
            if let Expr::For { pat, .. } = e {
                pattern_names(pat, &mut barred);
            }
        });
    }
    own.into_iter()
        .filter(|(n, ok)| *ok && !aliased.contains(n) && !barred.contains(n))
        .map(|(n, _)| n)
        .collect()
}

/// `xs = xs.push(v)`, the only shape that hands a list back to itself.
fn is_self_push(name: &str, value: &Expr) -> bool {
    let Expr::Call { callee, .. } = value else {
        return false;
    };
    matches!(callee.as_ref(), Expr::Field { base, name: m }
        if m == "push" && matches!(base.as_ref(), Expr::Ident(b) if b == name))
}

fn pattern_names(p: &Pattern, out: &mut HashSet<String>) {
    match p {
        Pattern::Bind(n) => {
            out.insert(n.clone());
        }
        Pattern::Ctor { args, .. } => args.iter().for_each(|a| pattern_names(a, out)),
        Pattern::Or(ps) => ps.iter().for_each(|a| pattern_names(a, out)),
        Pattern::List { elems, rest } => {
            elems.iter().for_each(|a| pattern_names(a, out));
            if let Some(r) = rest {
                pattern_names(r, out);
            }
        }
        Pattern::Record(fs) => {
            for (f, sub) in fs {
                match sub {
                    Some(p) => pattern_names(p, out),
                    None => {
                        out.insert(f.clone());
                    }
                }
            }
        }
        _ => {}
    }
}

/// Names this function gives a second holder to.
pub fn aliased_names(f: &FnDef, defined: &HashSet<String>) -> HashSet<String> {
    let mut out = HashSet::new();
    let Some(body) = &f.body else {
        return out;
    };
    match body {
        FnBody::Expr(e) => alias_in_expr(e, defined, &mut out),
        FnBody::Block(ss) => alias_in_stmts(ss, defined, &mut out),
    }
    out
}

fn alias_in_stmts(ss: &[Stmt], defined: &HashSet<String>, out: &mut HashSet<String>) {
    for s in ss {
        match s {
            Stmt::Bind(b) => {
                if !self_append(&b.target, &b.value) {
                    keep_all(&b.value, out);
                }
                alias_in_expr(&b.value, defined, out);
            }
            Stmt::Expr(e) => alias_in_expr(e, defined, out),
            Stmt::Fn(inner) => {
                out.extend(aliased_names(inner, defined));
            }
            _ => {}
        }
    }
}

fn self_append(target: &Expr, value: &Expr) -> bool {
    matches!(target, Expr::Ident(name) if is_self_push(name, value))
}

fn alias_in_expr(e: &Expr, defined: &HashSet<String>, out: &mut HashSet<String>) {
    let mut go = |x: &Expr| alias_in_expr(x, defined, out);
    match e {
        Expr::Ident(_) => {}
        Expr::Call { callee, args } => {
            let borrows = borrowing_call(callee, defined);
            match callee.as_ref() {
                Expr::Field { base, .. } => alias_in_expr(base, defined, out),
                other => keep_all(other, out),
            }
            for a in args {
                match arg_expr(a) {
                    x @ Expr::Str(_) => alias_in_expr(x, defined, out),
                    x if borrows => alias_in_expr(x, defined, out),
                    x => keep_all(x, out),
                }
            }
        }
        Expr::For { iter, body, .. } => {
            keep_all(iter, out);
            alias_in_stmts(body, defined, out);
        }
        Expr::If { cond, then, els } => {
            go(cond);
            alias_in_stmts(then, defined, out);
            if let Some(e) = els {
                alias_in_stmts(e, defined, out);
            }
        }
        Expr::While { cond, body } => {
            go(cond);
            alias_in_stmts(body, defined, out);
        }
        Expr::Block(ss) => alias_in_stmts(ss, defined, out),
        Expr::Field { base, .. } => go(base),
        Expr::Index { base, index } => {
            go(base);
            go(index);
        }
        Expr::Binary { lhs, rhs, .. } => {
            go(lhs);
            go(rhs);
        }
        Expr::Unary { expr, .. } => go(expr),
        Expr::Ternary { cond, then, els } => {
            go(cond);
            keep_all(then, out);
            keep_all(els, out);
        }
        Expr::Str(parts) => {
            for p in parts {
                if let StrPart::Interp(x) = p {
                    go(x);
                }
            }
        }
        _ => keep_all(e, out),
    }
}

/// Does this callee only read what it is given?
fn borrowing_call(callee: &Expr, defined: &HashSet<String>) -> bool {
    match callee {
        Expr::Ident(f) => !defined.contains(f) && BORROWING_FNS.contains(&f.as_str()),
        Expr::Field { name, .. } => {
            !defined.contains(name) && BORROWING_METHODS.contains(&name.as_str())
        }
        _ => false,
    }
}

/// A record's fields hold what they are given, however each one is written.
fn keep_fields(fs: &[Field], out: &mut HashSet<String>) {
    for f in fs {
        match f {
            Field::Value { value, .. } | Field::Bare(value) => keep_all(value, out),
            Field::Shorthand(n) => {
                out.insert(n.clone());
            }
            Field::Type { .. } => {}
        }
    }
}

/// Everything named in here may be kept.
fn keep_all(e: &Expr, out: &mut HashSet<String>) {
    walk_names(e, &mut |n| {
        out.insert(n.to_string());
    });
}

/// Every top-level name the module defines.
fn collect_defined(items: &[Stmt], out: &mut HashSet<String>) {
    for s in items {
        match s {
            Stmt::Fn(f) => {
                out.insert(f.name.clone());
            }
            Stmt::Bind(Bind {
                target: Expr::Ident(n),
                ..
            })
            | Stmt::Alias { name: n, .. } => {
                out.insert(n.clone());
            }
            _ => {}
        }
    }
}

/// The module's functions that have a body: name, parameter names, body.
fn fn_bodies(items: &[Stmt]) -> Vec<(String, HashSet<String>, &FnBody)> {
    items
        .iter()
        .filter_map(|s| match s {
            Stmt::Fn(f) => f.body.as_ref().map(|b| {
                let params = f.params.iter().map(|p| p.name.clone()).collect();
                (f.name.clone(), params, b)
            }),
            _ => None,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn module(src: &str) -> Module {
        maca_parser::parse(src).module
    }

    fn fresh(src: &str) -> Fresh {
        Fresh::of(&module(src))
    }

    /// A function that builds its result hands the caller something to own.
    #[test]
    fn a_built_string_is_the_callers() {
        let f = fresh("greet(n: str) -> str => \"hello \" ++ n\n");
        assert!(f.fns.contains("greet"));
    }

    /// A function that hands back its own argument does not, because the caller would be releasing a string it was only lent.
    #[test]
    fn a_returned_argument_is_not() {
        let f = fresh("same(n: str) -> str => n\n");
        assert!(!f.fns.contains("same"));
    }

    /// The accumulator shape: built up a piece at a time, then returned.
    #[test]
    fn an_accumulator_returned_at_the_end_is_fresh() {
        let f = fresh(
            "page(n: int) -> str {\n    \
             out = \"\"\n    \
             for i in 1..n {\n        out = out ++ \"row\"\n    }\n    \
             out\n}\n",
        );
        assert!(f.fns.contains("page"));
    }

    /// …but not once it has been handed somewhere else on the way out.
    #[test]
    fn an_accumulator_stored_elsewhere_is_not() {
        let f = fresh(
            "page(seen: str[]) -> str {\n    \
             out = \"\" ++ \"a\"\n    \
             seen.push(out)\n    \
             out\n}\n",
        );
        assert!(!f.fns.contains("page"));
    }

    /// One function's freshness carries into the next.
    #[test]
    fn freshness_flows_through_a_call() {
        let f = fresh("row() -> str => \"a\" ++ \"b\"\n\npage() -> str => row()\n");
        assert!(f.fns.contains("row") && f.fns.contains("page"));
    }

    /// A recursive function is not talked into a freshness it never earned.
    #[test]
    fn recursion_starts_from_nothing() {
        let f = fresh("walk(s: str) -> str => s.length() > 0 ? walk(s) : s\n");
        assert!(!f.fns.contains("walk"));
    }

    /// The names a block's statements keep, for a module that defines nothing but the function under test.
    fn kept(src: &str) -> HashSet<String> {
        let m = module(src);
        let Stmt::Fn(FnDef {
            body: Some(FnBody::Block(ss)),
            ..
        }) = &m.items[0]
        else {
            panic!("a block body");
        };
        Fresh::of(&m).retained(ss, Tail::Flows)
    }

    /// Reading a name is not keeping it, and that is what lets an accumulator be released one iteration at a time.
    #[test]
    fn concatenation_reads_its_operands() {
        let kept = kept("f() -> int {\n    a = \"x\"\n    b = a ++ \"y\"\n    0\n}\n");
        assert!(!kept.contains("a"), "read, not kept: {kept:?}");
    }

    /// Handing a name to a function the module wrote does keep it.
    #[test]
    fn an_argument_to_an_unknown_call_is_kept() {
        assert!(kept("f() -> int {\n    a = \"x\"\n    remember(a)\n    0\n}\n").contains("a"));
    }

    /// Written UFCS, the receiver is argument zero, so the two spellings of the same call have to agree about it.
    #[test]
    fn a_receiver_is_kept_by_whatever_keeps_an_argument() {
        assert!(
            kept("f() -> int {\n    a = \"x\"\n    remember(a)\n    0\n}\n").contains("a"),
            "written as a call"
        );
        assert!(
            kept("f() -> int {\n    a = \"x\"\n    a.remember()\n    0\n}\n").contains("a"),
            "written UFCS"
        );
        assert!(
            !kept("f() -> int {\n    a = \"x\"\n    n = a.length()\n    0\n}\n").contains("a"),
            "a runtime helper that only measures keeps nothing"
        );
    }

    /// A module's own definition wins over the table.
    #[test]
    fn a_redefined_helper_is_no_longer_borrowing() {
        let src = "length(s: str) -> int => 0\n\n                   f() -> int {\n    a = \"x\"\n    n = a.length()\n    0\n}\n";
        let m = module(src);
        let Stmt::Fn(FnDef {
            body: Some(FnBody::Block(ss)),
            ..
        }) = &m.items[1]
        else {
            panic!("a block body");
        };
        assert!(Fresh::of(&m).retained(ss, Tail::Flows).contains("a"));
    }

    /// A container holds what it is given, whatever shape it is written in, including the shorthand, which names a variable and holds no expression for a walk over expressions to find.
    #[test]
    fn a_container_keeps_its_elements() {
        assert!(kept("f() -> int {\n    a = \"x\"\n    xs = [a]\n    0\n}\n").contains("a"));
        assert!(
            kept("f() -> int {\n    a = \"x\"\n    xs = [Holder { a = a }]\n    0\n}\n")
                .contains("a"),
            "a named field"
        );
        assert!(
            kept("f() -> int {\n    a = \"x\"\n    xs = [Holder { a }]\n    0\n}\n").contains("a"),
            "and the shorthand for it"
        );
    }

    /// A parameter's first value is the caller's, which is not in this body to be looked at.
    #[test]
    fn a_conditionally_rewritten_parameter_is_not_fresh() {
        let f = fresh(
            "norm(s: str, add: bool) -> str {\n                 if add {\n        s = s ++ \"!\"\n    }\n    s\n}\n",
        );
        assert!(!f.fns.contains("norm"));
    }
}
