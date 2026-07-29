//! Who is holding this string, and may the block that built it let go?
//!
//! A `maca_str` is a `const char*`. Nothing about the value says whether it
//! points at a block this runtime allocated, at a literal in `.rodata`, or at
//! bytes some other value is still using — and until that question has an
//! answer, the only safe thing to do with a string is to keep it forever, which
//! is what the back end used to do. A program that built a page per request
//! grew by every page it had ever built.
//!
//! Two facts make an answer possible.
//!
//! The runtime keeps a promise: every `maca_str` a runtime helper returns is a
//! block it just allocated or a static literal, never one of its arguments.
//! (The shortcuts that used to return an argument unchanged — `replace` with an
//! empty needle, `pad` of an already-wide string — now copy.) So the *producer*
//! of a string is knowable from the expression that produced it.
//!
//! And retention is visible in the source: a string is kept only where the
//! program puts it somewhere — another binding, a list, a record field, an
//! argument to a function that might store it. Reading one does not keep it,
//! because every reader copies what it needs. That distinction is the whole
//! difference between this analysis and the array one next door, which counts
//! any mention at all: `out = out ++ row` *reads* `out`, so an accumulator
//! built up in a loop is still the block's to release, one iteration at a time.
//!
//! Both halves fail safe. A producer this module does not recognize is not
//! fresh, and a position it does not recognize retains — so an unknown shape
//! costs memory that is held too long, which is exactly what the program did
//! before any of this existed.

use maca_parser::ast::*;
use std::collections::{HashMap, HashSet};

/// Runtime helpers that hand back a string of their own making.
///
/// Called by name, so a module that defines its own `trim` shadows the entry
/// and gets nothing — the caller checks that before asking.
const FRESH_METHODS: &[&str] = &[
    // `str` — every one of these builds its result
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
    // `T[]` — `join` builds a string; the accessors return an element, which
    // belongs to the list, so they are deliberately absent
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
///
/// A name that reaches only these is still held by exactly one binding. Each
/// entry also never returns one of its arguments, so the caller cannot end up
/// with a second name for the same bytes — `max(a, b)` is missing for that
/// reason, not by oversight.
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
///
/// Every `str` method qualifies. The list methods here are the ones that
/// answer a question or build something new; `push`, `set` and the
/// closure-taking ones are absent because they store what they are given.
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
///
/// A call is the one producer whose freshness cannot be read off the call site:
/// `page(20)` builds a string, `first_line(text)` might hand back a slice of
/// its argument, and both are spelled the same. So it is answered once for the
/// whole module, by looking at what each function returns.
pub struct Fresh {
    fns: HashSet<String>,
    /// Names the module defines. A definition shadows a runtime helper, so a
    /// module with its own `trim` gets the module's answer, not the table's.
    defined: HashSet<String>,
}

impl Fresh {
    /// Work out which functions return a string the caller becomes the only
    /// owner of.
    ///
    /// Least fixed point from "nothing is fresh": each round marks a function
    /// whose every returned expression is fresh *given what is known so far*,
    /// and the rounds stop when a pass adds nothing. Starting pessimistic is
    /// what makes recursion safe — `walk(dir)` calling itself never talks
    /// itself into a freshness it has not earned.
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
            for (name, body) in &bodies {
                if fresh.fns.contains(name) {
                    continue;
                }
                if fresh.body_returns_fresh(body) {
                    fresh.fns.insert(name.clone());
                    grew = true;
                }
            }
            if !grew {
                return fresh;
            }
        }
    }

    /// Does `e`, evaluated here, produce a string nothing else is holding?
    pub fn allocates(&self, e: &Expr) -> bool {
        match e {
            // a literal is in `.rodata` and an interpolation is a concat chain;
            // releasing either is right, since a release only reaches a block
            // this allocator handed out
            Expr::Str(_) | Expr::Path(_) => true,
            // `++` concatenates and `/` joins paths — both build
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
    fn body_returns_fresh(&self, body: &FnBody) -> bool {
        match body {
            FnBody::Expr(e) => self.returned(e, &[]),
            FnBody::Block(ss) => {
                // A local named as the result is fresh when the local itself is
                // — `page` builds `out` a piece at a time and hands it back, and
                // refusing that would leave the common shape unowned. What it
                // must not do is hand the same string somewhere else on the way
                // out, so retention is measured with the tail left out.
                let kept = retained(ss, Tail::Read);
                self.returned_from(ss, &kept)
            }
        }
    }

    /// The tail of a statement list, and the tails of any branches it ends in.
    fn returned_from(&self, ss: &[Stmt], kept: &HashSet<String>) -> bool {
        let locals = fresh_locals(self, ss, kept);
        match ss.last() {
            Some(Stmt::Expr(e)) => self.returned(e, &locals),
            // a body that ends in a binding has no value to return
            _ => false,
        }
    }

    /// Is `e` in return position fresh, given the locals known to be?
    fn returned(&self, e: &Expr, locals: &[String]) -> bool {
        match e {
            Expr::Ident(n) => locals.iter().any(|l| l == n),
            Expr::Ternary { then, els, .. } => {
                self.returned(then, locals) && self.returned(els, locals)
            }
            Expr::Try(inner) | Expr::Reify(inner) => self.returned(inner, locals),
            Expr::If { then, els, .. } => {
                let kept = retained(then, Tail::Read);
                self.returned_from(then, &kept)
                    && els.as_ref().is_some_and(|e| {
                        let kept = retained(e, Tail::Read);
                        self.returned_from(e, &kept)
                    })
            }
            Expr::Block(ss) => {
                let kept = retained(ss, Tail::Read);
                self.returned_from(ss, &kept)
            }
            Expr::Match { arms, .. } => arms.iter().all(|a| self.returned(&a.body, locals)),
            _ => self.allocates(e),
        }
    }
}

/// Locals in `ss` that hold a string nobody else is holding.
///
/// A name qualifies when every value it is ever given is fresh — the first one
/// and every reassignment anywhere below, including inside a loop — and nothing
/// keeps it.
fn fresh_locals(f: &Fresh, ss: &[Stmt], kept: &HashSet<String>) -> Vec<String> {
    let mut binds: HashMap<String, bool> = HashMap::new();
    each_bind(ss, &mut |name, value| {
        let fresh = f.allocates(value);
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

/// Every `name = value` in the subtree, however deeply nested.
///
/// A loop body reassigns the accumulator declared above it, so a walk that
/// stopped at this statement list would call an accumulator fresh on the
/// strength of its first value alone.
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
    /// It leaves the block — a name there is handed to the caller.
    Flows,
    /// It is only read — used when asking what a body keeps *apart from* what
    /// it returns.
    Read,
}

/// Names whose string may be kept somewhere this statement list cannot see.
///
/// The default is retention: a call goes on the list unless the callee is one
/// of the borrowing helpers above. Everything a container holds goes on it, and
/// so does anything a lambda could capture.
pub fn retained(stmts: &[Stmt], tail: Tail) -> HashSet<String> {
    let mut out = HashSet::new();
    retain_stmts(stmts, tail, &mut out);
    out
}

fn retain_stmts(stmts: &[Stmt], tail: Tail, out: &mut HashSet<String>) {
    let n = stmts.len();
    for (i, s) in stmts.iter().enumerate() {
        let last = i + 1 == n;
        match s {
            Stmt::Bind(b) => {
                flows_out(&b.value, out);
                // `xs[i] = s` and `p.f = s` put the value in the container; the
                // container expression itself is only read.
                if !matches!(&b.target, Expr::Ident(_)) {
                    reads(&b.target, out);
                }
            }
            Stmt::Expr(e) if last && tail == Tail::Flows => flows_out(e, out),
            Stmt::Expr(e) => reads(e, out),
            // a nested function can capture anything it names
            Stmt::Fn(f) => match &f.body {
                Some(FnBody::Expr(e)) => keep_all(e, out),
                Some(FnBody::Block(ss)) => ss.iter().for_each(|s| {
                    walk_stmt(s, &mut |e| {
                        if let Expr::Ident(n) = e {
                            out.insert(n.clone());
                        }
                    })
                }),
                None => {}
            },
            _ => {}
        }
    }
}

/// The value leaves this position, so a bare name here is no longer ours.
fn flows_out(e: &Expr, out: &mut HashSet<String>) {
    match e {
        Expr::Ident(n) => {
            out.insert(n.clone());
        }
        Expr::Ternary { cond, then, els } => {
            reads(cond, out);
            flows_out(then, out);
            flows_out(els, out);
        }
        Expr::Try(inner) | Expr::Reify(inner) => flows_out(inner, out),
        _ => reads(e, out),
    }
}

/// The expression is evaluated and its parts read. Sub-positions that do keep a
/// value are marked as they are reached.
fn reads(e: &Expr, out: &mut HashSet<String>) {
    match e {
        Expr::Ident(_) | Expr::Int(_) | Expr::Float(_) | Expr::Bool(_) | Expr::Unit => {}
        Expr::Path(_) | Expr::Break | Expr::Continue => {}
        Expr::Str(parts) => {
            for p in parts {
                if let StrPart::Interp(e) = p {
                    reads(e, out);
                }
            }
        }
        // a container holds what it is given
        Expr::List(items) => items.iter().for_each(|i| keep_all(i, out)),
        Expr::Record(fs) | Expr::Ctor { fields: fs, .. } => keep_fields(fs, out),
        Expr::With { base, fields } => {
            keep_all(base, out);
            keep_fields(fields, out);
        }
        // a lambda outlives the expression that wrote it, and so does anything
        // it captured; a task the same
        Expr::Lambda { body, .. } => keep_all(body, out),
        Expr::Spawn(x) | Expr::Await(x) | Expr::Fail(x) => keep_all(x, out),
        Expr::Assign { target, value } => {
            reads(target, out);
            keep_all(value, out);
        }
        Expr::Call { callee, args } => {
            let borrows = borrowing(callee);
            if let Expr::Field { base, .. } = callee.as_ref() {
                reads(base, out);
            } else {
                reads(callee, out);
            }
            for a in args {
                if borrows {
                    reads(arg_expr(a), out);
                } else {
                    keep_all(arg_expr(a), out);
                }
            }
        }
        Expr::Field { base, .. } => reads(base, out),
        Expr::Index { base, index } => {
            reads(base, out);
            reads(index, out);
        }
        Expr::Range { lo, hi } => {
            reads(lo, out);
            reads(hi, out);
        }
        Expr::Unary { expr, .. } => reads(expr, out),
        Expr::Binary { lhs, rhs, .. } => {
            reads(lhs, out);
            reads(rhs, out);
        }
        Expr::Ternary { cond, then, els } => {
            reads(cond, out);
            reads(then, out);
            reads(els, out);
        }
        Expr::Try(x) | Expr::Reify(x) => reads(x, out),
        Expr::If { cond, then, els } => {
            reads(cond, out);
            retain_stmts(then, Tail::Flows, out);
            if let Some(e) = els {
                retain_stmts(e, Tail::Flows, out);
            }
        }
        Expr::Match { scrut, arms } => {
            reads(scrut, out);
            arms.iter().for_each(|a| flows_out(&a.body, out));
        }
        Expr::For { iter, body, .. } => {
            reads(iter, out);
            retain_stmts(body, Tail::Read, out);
        }
        Expr::While { cond, body } => {
            reads(cond, out);
            retain_stmts(body, Tail::Read, out);
        }
        Expr::Block(ss) => retain_stmts(ss, Tail::Flows, out),
    }
}

/// A record's fields hold what they are given, however each one is written —
/// `{ name = s }`, the shorthand `{ s }`, and a bare element alike.
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
    walk_expr(e, &mut |c| {
        if let Expr::Ident(n) = c {
            out.insert(n.clone());
        }
    });
}

/// Does this callee read its arguments without keeping any of them?
fn borrowing(callee: &Expr) -> bool {
    match callee {
        Expr::Ident(f) => BORROWING_FNS.contains(&f.as_str()),
        Expr::Field { name, .. } => BORROWING_METHODS.contains(&name.as_str()),
        _ => false,
    }
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

/// The module's functions that have a body, by name.
fn fn_bodies(items: &[Stmt]) -> Vec<(String, &FnBody)> {
    items
        .iter()
        .filter_map(|s| match s {
            Stmt::Fn(f) => f.body.as_ref().map(|b| (f.name.clone(), b)),
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

    /// A function that hands back its own argument does not — the caller would
    /// be releasing a string it was only lent.
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

    /// Reading a name is not keeping it — that is what lets an accumulator be
    /// released one iteration at a time.
    #[test]
    fn concatenation_reads_its_operands() {
        let m = module("f() -> int {\n    a = \"x\"\n    b = a ++ \"y\"\n    0\n}\n");
        let Stmt::Fn(FnDef {
            body: Some(FnBody::Block(ss)),
            ..
        }) = &m.items[0]
        else {
            panic!("a block body");
        };
        let kept = retained(ss, Tail::Flows);
        assert!(!kept.contains("a"), "read, not kept: {kept:?}");
    }

    /// Handing a name to a function the module wrote does keep it: what that
    /// function does with the string is not visible from here.
    #[test]
    fn an_argument_to_an_unknown_call_is_kept() {
        let m = module("f() -> int {\n    a = \"x\"\n    remember(a)\n    0\n}\n");
        let Stmt::Fn(FnDef {
            body: Some(FnBody::Block(ss)),
            ..
        }) = &m.items[0]
        else {
            panic!("a block body");
        };
        assert!(retained(ss, Tail::Flows).contains("a"));
    }

    /// A container holds what it is given, whatever shape it is written in.
    #[test]
    fn a_container_keeps_its_elements() {
        let m = module("f() -> int {\n    a = \"x\"\n    xs = [a]\n    0\n}\n");
        let Stmt::Fn(FnDef {
            body: Some(FnBody::Block(ss)),
            ..
        }) = &m.items[0]
        else {
            panic!("a block body");
        };
        assert!(retained(ss, Tail::Flows).contains("a"));
    }
}
