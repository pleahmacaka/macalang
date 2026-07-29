//! Local module resolution for the native `build`/`run` path.
//!
//! Two import shapes name a local `.maca` module:
//!
//! - **whole-module** — `import a/b` (`Import::Module`) or a single-word
//!   `import a` (`Import::Bare`): the entire sibling module is inlined.
//! - **selective** — `import { foo, bar } from a/b` (`Import::Names`): only the
//!   named top-level definitions are inlined, together with the transitive
//!   closure of same-module definitions they reference (so the slice still
//!   compiles). A name that the module doesn't define is a clean error rather
//!   than a dangling reference.
//!
//! Everything is inlined into one translation unit (dependency order, each
//! module once), so `maca build a.maca` sees a single source string. Foreign
//! imports (`import c "…"`, `nixpkgs`, stdlib builtins) resolve to no file and
//! are left for the backend.

use crate::ast::*;
use crate::modules::{import_segments, import_target, names_a_file};
use std::collections::{BTreeSet, HashMap, HashSet};
use std::path::{Path, PathBuf};

/// Read a program and inline every local module it imports (transitively). A
/// whole-module import contributes the module's full source text; a selective
/// import contributes only the requested definitions and their same-module
/// dependency closure. A program with no local imports is just its own text.
pub fn load_with_imports(entry: &Path) -> Result<String, String> {
    let mut g = Graph::default();
    collect(entry, &mut g)?;
    let sel = resolve_selection(entry, &g);

    let mut combined = String::new();
    for path in &g.order {
        match sel.get(path).unwrap_or(&Sel::All) {
            Sel::All => {
                // Preserve the module verbatim (comments, formatting) — its own
                // `import` lines are no-ops for codegen.
                combined.push_str(&read(path)?);
            }
            Sel::Names(want) => {
                let module = &g.parsed[path];
                let mut sliced = slice_module(module, want, path)?;
                qualify_private(&mut sliced, want, path);
                combined.push_str(&crate::print_module(&Module { items: sliced }));
            }
        }
        combined.push('\n');
    }
    Ok(combined)
}

/// Qualify a module's private definitions with the module's own name.
///
/// Everything is inlined into one translation unit, so two files that each keep
/// a `helper` to themselves collide — and "two files in a package cannot share
/// a private name" defeats the point of splitting a package into files at all.
/// The failure was a C `redefinition` error naming a function the reader never
/// wrote twice.
///
/// Only *private* names move. A name some importer asked for by hand is the
/// name it was asked for, and renaming it would break the caller; a private
/// name is one nobody asked of this module, so nothing outside can be referring
/// to it. Every one of them is qualified rather than only the ones that happen
/// to clash today, because a collision that appears when an unrelated file
/// gains a helper is a collision nobody can see coming — and because
/// `alpha__helper` in a stack trace says more than `helper` did.
fn qualify_private(items: &mut [Stmt], want: &BTreeSet<String>, path: &Path) {
    let Some(stem) = path.file_stem().map(|s| s.to_string_lossy().into_owned()) else {
        return;
    };
    // A body-less function is a foreign declaration: its name *is* the symbol
    // the engine provides, so it is the module's to expose and not the
    // module's to rename. Qualifying `http_listen` asked the linker for
    // `server__http_listen`, which nothing defines.
    let foreign: HashSet<&str> = items
        .iter()
        .filter_map(|st| match st {
            Stmt::Fn(f) if f.body.is_none() => Some(f.name.as_str()),
            _ => None,
        })
        .collect();

    let private: Vec<String> = items
        .iter()
        .filter_map(defined_name)
        .filter(|n| !want.contains(*n) && !foreign.contains(*n))
        .map(str::to_string)
        .collect();

    for name in private {
        let qualified = format!("{stem}__{name}");
        for st in items.iter_mut() {
            crate::ast::rename_ident(st, &name, &qualified);
        }
    }
}

/// How much of a module to inline.
enum Sel {
    /// The whole module (a `import a/b` / `import a` somewhere wants it, or it
    /// is the entry file).
    All,
    /// Only these top-level names (from `import { … } from a/b`), later grown to
    /// their same-module dependency closure.
    Names(BTreeSet<String>),
}

#[derive(Default)]
struct Graph {
    /// Post-order (dependencies before dependents), each module once.
    order: Vec<PathBuf>,
    /// Parsed module per canonical path.
    parsed: HashMap<PathBuf, Module>,
}

fn read(path: &Path) -> Result<String, String> {
    std::fs::read_to_string(path).map_err(|e| format!("cannot read {}: {e}", path.display()))
}

fn canon(path: &Path) -> PathBuf {
    std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

/// Depth-first post-order over local imports; parse each module once.
///
/// A slash-path import that resolves to nothing is an error here rather than a
/// line that does nothing. `import std/str` — a module that has never existed —
/// sat in four files, and each of them then hand-wrote the helpers it believed
/// it was importing.
fn collect(path: &Path, g: &mut Graph) -> Result<(), String> {
    let key = canon(path);
    if g.parsed.contains_key(&key) {
        return Ok(());
    }
    let src = read(path)?;
    let parsed = crate::parse(&src);
    g.parsed.insert(key.clone(), parsed.module.clone());
    for item in &parsed.module.items {
        let Stmt::Import(im) = item else { continue };
        match import_target(im, path) {
            Some(dep) => collect(&dep, g)?,
            None if names_a_file(im) => {
                let written = import_segments(im).unwrap_or_default().join("/");
                return Err(format!(
                    "{}: no module `{written}` — `{written}.maca` is not beside \
                     this file or in the working directory",
                    path.display()
                ));
            }
            None => {}
        }
    }
    g.order.push(key);
    Ok(())
}

/// Effective selection per module: the entry file and any whole-module import
/// target are `All`; a module reached only through `import { … } from` is
/// `Names(union of requested)`. `All` always wins over `Names`. Propagated to a
/// fixpoint so a diamond (one importer wants all, another a slice) is `All`.
fn resolve_selection(entry: &Path, g: &Graph) -> HashMap<PathBuf, Sel> {
    let mut sel: HashMap<PathBuf, Sel> = HashMap::new();
    sel.insert(canon(entry), Sel::All);

    // Iterate to a fixpoint: seeding a module can turn a later one `All`.
    let mut changed = true;
    while changed {
        changed = false;
        for path in &g.order {
            let module = &g.parsed[path];
            for item in &module.items {
                let Stmt::Import(im) = item else { continue };
                let Some(dep) = import_target(im, path) else {
                    continue;
                };
                let dep = canon(&dep);
                let want: Option<Vec<String>> = match im {
                    Import::Module(_) | Import::Bare(_) => None, // → All
                    Import::Names { names, .. } => Some(names.clone()),
                    _ => continue,
                };
                changed |= merge(&mut sel, dep, want);
            }
        }
    }
    sel
}

/// Fold one import edge's request into `sel[dep]`. Returns whether it changed.
fn merge(sel: &mut HashMap<PathBuf, Sel>, dep: PathBuf, want: Option<Vec<String>>) -> bool {
    match (sel.get_mut(&dep), want) {
        (Some(Sel::All), _) => false,
        (_, None) => {
            // Whole-module import: promote to All.
            sel.insert(dep, Sel::All);
            true
        }
        (Some(Sel::Names(have)), Some(names)) => {
            let mut changed = false;
            for n in names {
                changed |= have.insert(n);
            }
            changed
        }
        (None, Some(names)) => {
            sel.insert(dep, Sel::Names(names.into_iter().collect()));
            true
        }
    }
}

/// The top-level name a statement defines, if any.
fn defined_name(st: &Stmt) -> Option<&str> {
    match st {
        Stmt::Fn(f) => Some(&f.name),
        Stmt::Bind(b) => match &b.target {
            Expr::Ident(n) => Some(n),
            _ => None,
        },
        Stmt::Alias { name, .. } => Some(name),
        _ => None,
    }
}

/// Slice a module down to `want` plus the transitive closure of the same-module
/// definitions those items reference. Errors if a requested name isn't defined
/// in the module (a typo, or a name that lives elsewhere).
fn slice_module(
    module: &Module,
    want: &BTreeSet<String>,
    path: &Path,
) -> Result<Vec<Stmt>, String> {
    // name → item index, and variant → owning type (so referencing a sum
    // variant pulls in its type definition).
    let mut by_name: HashMap<&str, usize> = HashMap::new();
    let mut variant_owner: HashMap<String, String> = HashMap::new();
    for (i, st) in module.items.iter().enumerate() {
        if let Some(n) = defined_name(st) {
            by_name.insert(n, i);
            if let Stmt::Bind(b) = st
                && let Expr::Ident(ty_name) = &b.target
            {
                for v in sum_variants(&b.value) {
                    variant_owner.insert(v, ty_name.clone());
                }
            }
        }
    }

    let modname = path
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.display().to_string());

    // Seed the worklist from the requested names (mapping a requested variant
    // to its owning type).
    let mut queue: Vec<usize> = Vec::new();
    let mut visited: HashSet<usize> = HashSet::new();
    for name in want {
        let idx = by_name
            .get(name.as_str())
            .copied()
            .or_else(|| {
                variant_owner
                    .get(name)
                    .and_then(|t| by_name.get(t.as_str()).copied())
            })
            .ok_or_else(|| {
                format!(
                    "import {{ {name} }} from {modname}: '{name}' is not defined \
                     in that module"
                )
            })?;
        if visited.insert(idx) {
            queue.push(idx);
        }
    }

    // Grow the closure: every referenced name that this module defines (or a
    // referenced variant's owning type) is pulled in too. Over-inclusion is
    // harmless — a spuriously matched local name just compiles unused — so no
    // scope analysis is needed.
    while let Some(idx) = queue.pop() {
        let mut refs = BTreeSet::new();
        refs_in_stmt(&module.items[idx], &mut refs);
        for r in refs {
            let target = by_name.get(r.as_str()).copied().or_else(|| {
                variant_owner
                    .get(&r)
                    .and_then(|t| by_name.get(t.as_str()).copied())
            });
            if let Some(t) = target
                && visited.insert(t)
            {
                queue.push(t);
            }
        }
    }

    // Emit in original source order for stable, readable output.
    //
    // A foreign import comes along whatever was selected. `import c "http.h"`
    // is not a definition anybody can ask for by name — it is the module saying
    // which engine its functions are compiled against, and dropping it left the
    // slice referring to symbols the link step was never told to provide.
    Ok(module
        .items
        .iter()
        .enumerate()
        .filter(|(i, st)| {
            visited.contains(i) || matches!(st, Stmt::Import(Import::Foreign { .. }))
        })
        .map(|(_, st)| st.clone())
        .collect())
}

/// The variant names of a sum-type value `A | B(x) | C`. Non-union values yield
/// nothing.
fn sum_variants(value: &Expr) -> Vec<String> {
    fn go(e: &Expr, out: &mut Vec<String>) {
        match e {
            Expr::Binary {
                op: BinOp::Union,
                lhs,
                rhs,
            } => {
                go(lhs, out);
                go(rhs, out);
            }
            Expr::Ident(n) => out.push(n.clone()),
            Expr::Ctor { name, .. } => out.push(name.clone()),
            // `Circle(float)` — a variant with payload parses as a call.
            Expr::Call { callee, .. } => {
                if let Expr::Ident(n) = &**callee {
                    out.push(n.clone());
                }
            }
            _ => {}
        }
    }
    let mut out = Vec::new();
    // Only treat a top-level union as a sum type; a lone ident value is an alias.
    if matches!(
        value,
        Expr::Binary {
            op: BinOp::Union,
            ..
        }
    ) {
        go(value, &mut out);
    }
    out
}

// --- reference collection -------------------------------------------------
// Collect *every* referenced name (identifiers, constructors, type heads).
// Deliberately ignores scoping: pulling in a spurious same-named top-level
// definition is harmless, whereas missing one breaks the compile.

fn refs_in_stmt(st: &Stmt, out: &mut BTreeSet<String>) {
    match st {
        Stmt::Fn(f) => {
            for p in &f.params {
                if let Some(t) = &p.ty {
                    refs_in_type(t, out);
                }
            }
            if let Some(t) = &f.ret {
                refs_in_type(t, out);
            }
            match &f.body {
                Some(FnBody::Block(stmts)) => stmts.iter().for_each(|s| refs_in_stmt(s, out)),
                Some(FnBody::Expr(e)) => refs_in_expr(e, out),
                None => {}
            }
        }
        Stmt::Bind(b) => {
            for t in &b.tys {
                refs_in_type(t, out);
            }
            refs_in_expr(&b.value, out);
        }
        Stmt::Alias { value, .. } => refs_in_expr(value, out),
        Stmt::Expr(e) => refs_in_expr(e, out),
        Stmt::Import(_) => {}
    }
}

fn refs_in_type(t: &Type, out: &mut BTreeSet<String>) {
    match t {
        Type::Name(segs) => {
            if let Some(head) = segs.first() {
                out.insert(head.clone());
            }
        }
        Type::Apply(h, args) => {
            refs_in_type(h, out);
            args.iter().for_each(|a| refs_in_type(a, out));
        }
        Type::Array(t) | Type::Opt(t) | Type::Paren(t) => refs_in_type(t, out),
    }
}

fn refs_in_expr(e: &Expr, out: &mut BTreeSet<String>) {
    match e {
        Expr::Ident(n) => {
            out.insert(n.clone());
        }
        Expr::Ctor { name, fields } => {
            out.insert(name.clone());
            fields.iter().for_each(|f| refs_in_field(f, out));
        }
        Expr::Int(_)
        | Expr::Float(_)
        | Expr::Bool(_)
        | Expr::Unit
        | Expr::Path(_)
        | Expr::Break
        | Expr::Continue => {}
        Expr::Str(parts) => parts.iter().for_each(|p| {
            if let StrPart::Interp(e) = p {
                refs_in_expr(e, out)
            }
        }),
        Expr::List(es) => es.iter().for_each(|e| refs_in_expr(e, out)),
        Expr::Record(fields) => fields.iter().for_each(|f| refs_in_field(f, out)),
        Expr::Call { callee, args } => {
            refs_in_expr(callee, out);
            args.iter().for_each(|a| refs_in_arg(a, out));
        }
        Expr::Field { base, .. } => refs_in_expr(base, out),
        Expr::Index { base, index } => {
            refs_in_expr(base, out);
            refs_in_expr(index, out);
        }
        Expr::Range { lo, hi } => {
            refs_in_expr(lo, out);
            refs_in_expr(hi, out);
        }
        Expr::Unary { expr, .. } => refs_in_expr(expr, out),
        Expr::Binary { lhs, rhs, .. } => {
            refs_in_expr(lhs, out);
            refs_in_expr(rhs, out);
        }
        Expr::Ternary { cond, then, els } => {
            refs_in_expr(cond, out);
            refs_in_expr(then, out);
            refs_in_expr(els, out);
        }
        Expr::If { cond, then, els } => {
            refs_in_expr(cond, out);
            then.iter().for_each(|s| refs_in_stmt(s, out));
            if let Some(e) = els {
                e.iter().for_each(|s| refs_in_stmt(s, out));
            }
        }
        Expr::Match { scrut, arms } => {
            refs_in_expr(scrut, out);
            for a in arms {
                refs_in_pattern(&a.pat, out);
                if let Some(g) = &a.guard {
                    refs_in_expr(g, out);
                }
                refs_in_expr(&a.body, out);
            }
        }
        Expr::For { pat, iter, body } => {
            refs_in_pattern(pat, out);
            refs_in_expr(iter, out);
            body.iter().for_each(|s| refs_in_stmt(s, out));
        }
        Expr::While { cond, body } => {
            refs_in_expr(cond, out);
            body.iter().for_each(|s| refs_in_stmt(s, out));
        }
        Expr::Lambda {
            params,
            ret: _,
            body,
        } => {
            for p in params {
                if let Some(t) = &p.ty {
                    refs_in_type(t, out);
                }
            }
            refs_in_expr(body, out);
        }
        Expr::With { base, fields } => {
            refs_in_expr(base, out);
            fields.iter().for_each(|f| refs_in_field(f, out));
        }
        Expr::Try(e) | Expr::Fail(e) | Expr::Reify(e) | Expr::Await(e) | Expr::Spawn(e) => {
            refs_in_expr(e, out)
        }
        Expr::Assign { target, value } => {
            refs_in_expr(target, out);
            refs_in_expr(value, out);
        }
        Expr::Block(stmts) => stmts.iter().for_each(|s| refs_in_stmt(s, out)),
    }
}

fn refs_in_field(f: &Field, out: &mut BTreeSet<String>) {
    match f {
        Field::Value { value, .. } => refs_in_expr(value, out),
        Field::Type { ty, .. } => refs_in_type(ty, out),
        Field::Shorthand(n) => {
            out.insert(n.clone());
        }
        Field::Bare(e) => refs_in_expr(e, out),
    }
}

fn refs_in_arg(a: &Arg, out: &mut BTreeSet<String>) {
    match a {
        Arg::Pos(e) => refs_in_expr(e, out),
        Arg::Named { value, .. } => refs_in_expr(value, out),
        Arg::Directive { value, .. } => refs_in_expr(value, out),
    }
}

fn refs_in_pattern(p: &Pattern, out: &mut BTreeSet<String>) {
    match p {
        Pattern::Ctor { name, args } => {
            out.insert(name.clone());
            args.iter().for_each(|a| refs_in_pattern(a, out));
        }
        Pattern::Record(fields) => {
            for (name, sub) in fields {
                out.insert(name.clone());
                if let Some(sp) = sub {
                    refs_in_pattern(sp, out);
                }
            }
        }
        Pattern::List { elems, rest } => {
            elems.iter().for_each(|e| refs_in_pattern(e, out));
            if let Some(r) = rest {
                refs_in_pattern(r, out);
            }
        }
        Pattern::Or(alts) => alts.iter().for_each(|a| refs_in_pattern(a, out)),
        // Wild / literals / plain binds reference nothing top-level. A
        // `Pattern::Bind` shadows, but we don't remove it from the ref set of
        // the enclosing item — over-inclusion is harmless.
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(dir: &Path, name: &str, src: &str) -> PathBuf {
        let p = dir.join(name);
        std::fs::write(&p, src).unwrap();
        p
    }

    fn tmp(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("maca-imports-{tag}"));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn selective_import_pulls_only_named_plus_deps() {
        let d = tmp("sel");
        write(
            &d,
            "util.maca",
            "helper(x: int) -> int => x + 1\n\
             wanted(x: int) -> int => helper(x) * 2\n\
             unused(x: int) -> int => x - 99\n",
        );
        let entry = write(
            &d,
            "main.maca",
            "import { wanted } from util\nmain() -> int => wanted(10)\n",
        );
        let combined = load_with_imports(&entry).unwrap();
        assert!(combined.contains("wanted"), "keeps the named fn");
        assert!(combined.contains("helper"), "keeps its dependency");
        assert!(
            !combined.contains("unused"),
            "drops the unreferenced fn:\n{combined}"
        );
    }

    #[test]
    fn selective_import_pulls_referenced_sum_type() {
        let d = tmp("sum");
        write(
            &d,
            "colors.maca",
            "Color = Red | Green | Blue\n\
             warmth(c: Color) -> int =>\n    match c {\n        Red => 2\n        Green => 1\n        Blue => 0\n    }\n\
             Unrelated = A | B\n",
        );
        let entry = write(
            &d,
            "main.maca",
            "import { warmth } from colors\nmain() -> int => warmth(Green)\n",
        );
        let combined = load_with_imports(&entry).unwrap();
        assert!(combined.contains("warmth"));
        assert!(combined.contains("Color ="), "pulls the type the fn uses");
        assert!(
            !combined.contains("Unrelated"),
            "drops the unused sum type:\n{combined}"
        );
    }

    #[test]
    fn importing_a_missing_name_is_an_error() {
        let d = tmp("missing");
        write(&d, "m.maca", "foo() -> int => 1\n");
        let entry = write(
            &d,
            "main.maca",
            "import { nope } from m\nmain() -> int => 0\n",
        );
        let err = load_with_imports(&entry).unwrap_err();
        assert!(err.contains("nope"), "error names the symbol: {err}");
        assert!(err.contains("not defined"), "error is explanatory: {err}");
    }

    #[test]
    fn whole_module_import_still_inlines_everything() {
        let d = tmp("whole");
        write(&d, "lib.maca", "a() -> int => 1\nb() -> int => 2\n");
        let entry = write(&d, "main.maca", "import lib\nmain() -> int => a() + b()\n");
        let combined = load_with_imports(&entry).unwrap();
        assert!(combined.contains("a()") && combined.contains("b()"));
    }
}
