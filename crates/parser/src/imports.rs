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
use crate::modules::{import_resolution, import_segments, import_target, names_a_file};
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::path::{Path, PathBuf};

/// Read a program and inline every local module it imports (transitively). A
/// whole-module import contributes the module's full source text; a selective
/// import contributes only the requested definitions and their same-module
/// dependency closure. A program with no local imports is just its own text.
pub fn load_with_imports(entry: &Path) -> Result<String, String> {
    let mut g = Graph::default();
    collect(entry, &mut g)?;
    let sel = resolve_selection(entry, &g);

    let mut units: Vec<Unit> = Vec::new();
    for path in &g.order {
        let src = read(path)?;
        match sel.get(path).unwrap_or(&Sel::All) {
            Sel::All => units.push(Unit {
                items: g.parsed[path].items.clone(),
                verbatim: true,
                path: path.clone(),
                src,
            }),
            Sel::Names(want) => {
                let mut sliced = slice_module(&g.parsed[path], want, path)?;
                qualify_private(&mut sliced, want, path);
                units.push(Unit {
                    items: sliced,
                    verbatim: false,
                    path: path.clone(),
                    src,
                });
            }
        }
    }
    separate_scopes(&mut units, &g, entry)?;

    let mut combined = String::new();
    for u in &units {
        if u.verbatim {
            // Preserve the module verbatim (comments, formatting). Its own
            // `import` lines are no-ops for codegen.
            combined.push_str(&u.src);
        } else {
            combined.push_str(&crate::print_module(&Module {
                items: u.items.clone(),
            }));
        }
        combined.push('\n');
    }
    Ok(combined)
}

/// One module's contribution to the flat translation unit.
struct Unit {
    path: PathBuf,
    /// The module's own text. Emitted as it was written while nothing has
    /// rewritten the module, which keeps its comments and its formatting in the
    /// program the back end reads.
    src: String,
    /// What the module contributes: every item for a whole-module import, the
    /// requested slice for a selective one.
    items: Vec<Stmt>,
    /// Whether `src` still says what `items` do. A rename clears this, and the
    /// module is printed from its tree instead.
    verbatim: bool,
}

impl Unit {
    fn stem(&self) -> String {
        self.path
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| "module".to_string())
    }

    fn defines(&self, name: &str) -> bool {
        self.items
            .iter()
            .filter_map(defined_name)
            .any(|n| n == name)
    }

    /// Every top-level name the module contributes.
    fn defined(&self) -> Vec<String> {
        self.items
            .iter()
            .filter_map(defined_name)
            .map(str::to_string)
            .collect()
    }
}

/// What each module can reach: the modules it imports, transitively.
type Reach = HashMap<PathBuf, HashSet<PathBuf>>;

/// Per module, the names some importer asked that module for by hand.
type Asked = HashMap<PathBuf, BTreeSet<String>>;

/// Keep two modules' names apart once they share one namespace.
///
/// Inlining flattens every module into a single translation unit, so a name
/// written in one file can be answered by a definition in another that the
/// first never imported. Two shapes of that, and both used to fail in a file
/// the author was not looking at:
///
/// * two modules define the same name, and the checker types a call in one of
///   them against the other's signature (`profile/flame` and `profile/report`
///   each keeping a `helper`);
/// * one module's top level answers for a name another module binds as a
///   parameter, which the C back end turns into a function value where a
///   lambda captured it (`cli/style`'s `pad` against the `pad` parameter of
///   `std/text`'s `indent`).
///
/// Both are repaired by moving a name. `rename_ident` rewrites a definition
/// together with its references and declines to touch a function that gives the
/// name a meaning of its own, which is exactly the distinction both shapes turn
/// on, so it is asked rather than reimplemented here.
fn separate_scopes(units: &mut [Unit], g: &Graph, entry: &Path) -> Result<(), String> {
    let scope = Scope::of(units, g, entry);
    separate_definitions(units, &scope)?;
    separate_bindings(units, &scope);
    Ok(())
}

/// What the program as a whole says about a name, gathered once.
struct Scope {
    /// Which modules each module can reach.
    reach: Reach,
    /// Names an importer asked a module for by hand.
    asked: Asked,
    /// The program's own file, whose names nothing here renames.
    entry: PathBuf,
    /// Names written as a field or inside a pattern anywhere in the program,
    /// which is where a rename cannot follow them.
    members: HashSet<String>,
    /// Per module, in `units` order, the names that module binds itself.
    bound: Vec<BTreeSet<String>>,
}

impl Scope {
    fn of(units: &[Unit], g: &Graph, entry: &Path) -> Scope {
        let defined: BTreeSet<String> = units.iter().flat_map(Unit::defined).collect();
        Scope {
            reach: import_closure(g),
            asked: requested_names(g),
            entry: canon(entry),
            members: units.iter().flat_map(|u| members(&u.items)).collect(),
            bound: units.iter().map(|u| own_bindings(u, &defined)).collect(),
        }
    }
}

/// Two modules defining one name: move the private one out of the way.
///
/// A collision has two possible repairs, so the private side moves and the API
/// side keeps the name a caller and a crash trace both show. Where neither side
/// can move, the clash is reported naming both files, because the alternative
/// is a diagnostic about the file the author was not editing.
fn separate_definitions(units: &mut [Unit], scope: &Scope) -> Result<(), String> {
    let mut owners: BTreeMap<String, Vec<usize>> = BTreeMap::new();
    for (i, u) in units.iter().enumerate() {
        for n in u.defined() {
            let os = owners.entry(n).or_default();
            if !os.contains(&i) {
                os.push(i);
            }
        }
    }
    let mut taken: HashSet<String> = owners.keys().cloned().collect();

    for (name, os) in &owners {
        if os.len() < 2 {
            continue;
        }
        let meant = meanings(units, os, name, scope)?;
        let mut kept: Vec<usize> = Vec::new();
        for &i in os {
            if is_api(&units[i], name, &scope.asked) || !can_move(units, i, name, scope, &meant) {
                kept.push(i);
                continue;
            }
            let to = fresh(&units[i].stem(), name, &mut taken);
            move_name(units, i, name, &to, scope, &meant);
        }
        if kept.len() > 1 {
            return Err(clashing_definitions(units, &kept, name));
        }
    }
    Ok(())
}

/// Which of several same-named definitions a third module means.
///
/// A module that neither defines the name nor binds it, and can reach two of the
/// definitions, is written against exactly one of them — and moving the other
/// must not take its reference along. Left to `reaches` alone the reference went
/// to whichever definition moved, so a call written against a documented
/// function silently ran a private helper of a module the author never opened.
///
/// What says which is the same rule the rest of this pass uses: a private
/// definition is not a name another module can be referring to, so where exactly
/// one of the definitions in reach is API, that is the one. Where none is, or
/// more than one is, nothing in the module's source distinguishes them, and the
/// reference is refused rather than bound by the order the moves happened in.
fn meanings(
    units: &[Unit],
    os: &[usize],
    name: &str,
    scope: &Scope,
) -> Result<HashMap<usize, usize>, String> {
    let mut out = HashMap::new();
    for i in 0..units.len() {
        if os.contains(&i) || scope.bound[i].contains(name) || !refers(&units[i], name) {
            continue;
        }
        let in_reach: Vec<usize> = os
            .iter()
            .copied()
            .filter(|&o| reaches(scope, &units[i].path, &units[o].path))
            .collect();
        if in_reach.len() < 2 {
            continue;
        }
        let api: Vec<usize> = in_reach
            .iter()
            .copied()
            .filter(|&o| is_api(&units[o], name, &scope.asked))
            .collect();
        match api.as_slice() {
            [only] => {
                out.insert(i, *only);
            }
            _ => return Err(ambiguous_reference(units, i, &in_reach, name)),
        }
    }
    Ok(out)
}

/// Does this module write the name anywhere at all?
fn refers(unit: &Unit, name: &str) -> bool {
    let mut refs = BTreeSet::new();
    unit.items.iter().for_each(|st| refs_in_stmt(st, &mut refs));
    refs.contains(name)
}

/// One name, several modules answering for it, and a third module writing it.
fn ambiguous_reference(units: &[Unit], at: usize, in_reach: &[usize], name: &str) -> String {
    let files: Vec<String> = in_reach
        .iter()
        .map(|&o| format!("  {}", units[o].path.display()))
        .collect();
    format!(
        "{}: `{name}` is defined by more than one module this file reaches, and \
         every module is inlined into one:\n{}\n  Nothing here says which one \
         `{name}` means. Ask for the one you mean with `import {{ {name} }} from \
         …`, or rename the others.",
        units[at].path.display(),
        files.join("\n")
    )
}

/// A top level answering for a name another module binds locally: move the top
/// level, so the binding means what it says.
///
/// Every module that binds the name is evidence, including one that imports the
/// definition and means both. `can_move` is what settles those: a module that
/// binds the name *and* can reach the definition would keep a reference the
/// rename cannot follow, so nothing moves on its account. Filtering here as
/// well was tried, and no program can tell the difference: the case it would
/// have suppressed is exactly the case `can_move` refuses.
fn separate_bindings(units: &mut [Unit], scope: &Scope) {
    let mut owner: HashMap<String, usize> = HashMap::new();
    for (i, u) in units.iter().enumerate() {
        for n in u.defined() {
            owner.insert(n, i);
        }
    }
    let mut moves: BTreeSet<(String, usize)> = BTreeSet::new();
    for v in 0..units.len() {
        for (name, &o) in &owner {
            if o != v && scope.bound[v].contains(name) {
                moves.insert((name.clone(), o));
            }
        }
    }
    let mut taken: HashSet<String> = owner.keys().cloned().collect();
    // `separate_definitions` ran first, so every top-level name is answered by
    // one module and there is nothing left for a third module to be ambiguous
    // about.
    let settled = HashMap::new();
    for (name, u) in moves {
        if !can_move(units, u, &name, scope, &settled) {
            continue;
        }
        let to = fresh(&units[u].stem(), &name, &mut taken);
        move_name(units, u, &name, &to, scope, &settled);
    }
}

/// May this module's `name` be renamed without losing a reference to it?
///
/// Everything the flat translation unit holds is rewritten together, so the bar
/// is that every reference follows. Four kinds do not.
///
/// A name read from outside the source: `main` is the entry symbol, `maca test`
/// finds a suite by looking for `test_…`, a body-less declaration *is* the
/// symbol some library provides, and the entry file's own names are what the
/// person running the program typed.
///
/// A name written as a field or a pattern, which `rename_ident` deliberately
/// leaves alone. `r.start("build")` is a UFCS call the C back end lowers to
/// `start(r, "build")`, so renaming the definition and not the call left the
/// call naming a function nothing defines.
///
/// A name this module binds itself as well as defines, where the rename would
/// skip the function holding the binding and leave its reference behind.
///
/// And a name some module that reaches this one binds, for the same reason.
fn can_move(
    units: &[Unit],
    owner: usize,
    name: &str,
    scope: &Scope,
    meant: &HashMap<usize, usize>,
) -> bool {
    if name == "main" || name.starts_with("test_") || units[owner].path == scope.entry {
        return false;
    }
    if scope.members.contains(name) || is_foreign(&units[owner].items, name) {
        return false;
    }
    (0..units.len())
        .filter(|&i| rewritten_by(units, i, owner, name, scope, meant))
        .all(|i| !scope.bound[i].contains(name))
}

/// Does moving `owner`'s `name` rewrite this module?
///
/// The module that defines it, and every module that can reach the definition
/// and has no definition of its own to mean instead — except one `meanings` has
/// already settled on a different definition for, whose reference this move is
/// not about.
fn rewritten_by(
    units: &[Unit],
    i: usize,
    owner: usize,
    name: &str,
    scope: &Scope,
    meant: &HashMap<usize, usize>,
) -> bool {
    if i == owner {
        return true;
    }
    !units[i].defines(name)
        && reaches(scope, &units[i].path, &units[owner].path)
        && meant.get(&i).is_none_or(|&o| o == owner)
}

/// Rename `from` to `to` at its definition and everywhere it is referred to.
fn move_name(
    units: &mut [Unit],
    owner: usize,
    from: &str,
    to: &str,
    scope: &Scope,
    meant: &HashMap<usize, usize>,
) {
    let targets: Vec<usize> = (0..units.len())
        .filter(|&i| rewritten_by(units, i, owner, from, scope, meant))
        .collect();
    for i in targets {
        let before = units[i].items.clone();
        for st in units[i].items.iter_mut() {
            crate::ast::rename_ident(st, from, to);
        }
        if units[i].items != before {
            units[i].verbatim = false;
        }
    }
}

/// Which of `names` this module gives a meaning of its own to: a parameter, a
/// local, a loop variable, a lambda's argument.
///
/// Answered by renaming all of them away at once and seeing which are still
/// referred to. `rename_ident` declines to rewrite a function that binds the
/// name, so whatever survives is bound by this module. Asking the rename itself
/// instead of keeping a second copy of its scope rules here is what makes this
/// answer and the repair agree.
///
/// Only names the module refers to at all are asked about: one it never writes
/// cannot survive the probe, and the probe costs a walk per name.
fn own_bindings(unit: &Unit, names: &BTreeSet<String>) -> BTreeSet<String> {
    let mut refs = BTreeSet::new();
    unit.items.iter().for_each(|st| refs_in_stmt(st, &mut refs));
    let asking: BTreeSet<String> = refs.intersection(names).cloned().collect();
    if asking.is_empty() {
        return BTreeSet::new();
    }
    let mut probe: Vec<Stmt> = unit.items.clone();
    for n in &asking {
        for st in probe.iter_mut() {
            crate::ast::rename_ident(st, n, &format!("\u{1}{n}"));
        }
    }
    let mut left = BTreeSet::new();
    probe.iter().for_each(|st| refs_in_stmt(st, &mut left));
    asking.intersection(&left).cloned().collect()
}

/// Every name this module writes as a field or inside a pattern.
///
/// These are the positions `rename_ident` holds still: a field name is a field,
/// not a binding. The C back end reads two of them as calls, though, since
/// `x.f(y)` is `f(x, y)`, so a name appearing here is a name this pass leaves
/// alone.
fn members(items: &[Stmt]) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    for st in items {
        crate::ast::walk_stmt(st, &mut |e| match e {
            Expr::Field { name, .. } => {
                out.insert(name.clone());
            }
            Expr::Match { arms, .. } => arms.iter().for_each(|a| refs_in_pattern(&a.pat, &mut out)),
            Expr::For { pat, .. } => refs_in_pattern(pat, &mut out),
            _ => {}
        });
    }
    out
}

/// Is this name part of what the module offers, rather than one of its workings?
///
/// Maca has no export keyword. What it has is the `///` marker: a doc block
/// directly above an item is what makes the item API, and an ordinary `//`
/// comment explains a helper to the next reader of the source. That is the rule
/// `tools/macadoc.maca` publishes the reference from and the one the tooling
/// chapter documents, so it is the rule used here.
///
/// A name an importer asked for by hand is API too, whatever comment sits above
/// it: `import { pad } from cli/style` is that module being asked for `pad`, and
/// `maca -m http.serve` generates exactly such an import.
fn is_api(unit: &Unit, name: &str, asked: &Asked) -> bool {
    documented(&unit.src).contains(name)
        || asked.get(&unit.path).is_some_and(|ns| ns.contains(name))
}

/// The names carrying a `///` block directly above them.
///
/// A fourth slash means it is a rule somebody drew rather than a doc block, and
/// a blank line or an ordinary comment in between breaks the attachment. Both
/// follow `tools/macadoc.maca`, so what counts as API here is what gets
/// published as API there.
fn documented(src: &str) -> HashSet<String> {
    let mut out = HashSet::new();
    let mut docs = false;
    for line in src.lines() {
        if line.starts_with("///") && !line.starts_with("////") {
            docs = true;
            continue;
        }
        if docs {
            if let Some(name) = declared_name(line) {
                out.insert(name);
            }
            docs = false;
        }
    }
    out
}

/// The name a top-level declaration line introduces: `name(` for a function,
/// `Name =` for a type or a constant. An indented line is inside something else
/// and declares nothing at this level.
///
/// A line this cannot read counts as undocumented, which is the safe direction:
/// a name is only ever moved when every reference to it moves too.
fn declared_name(line: &str) -> Option<String> {
    if line.starts_with(char::is_whitespace) {
        return None;
    }
    let head = match (line.find('('), line.find('=')) {
        (Some(p), eq) if eq.is_none_or(|e| p < e) => &line[..p],
        (_, Some(e)) => {
            let head = &line[..e];
            let head = head.strip_prefix("const ").unwrap_or(head);
            head.split(':').next().unwrap_or(head)
        }
        _ => return None,
    };
    let head = head.trim();
    let named = !head.is_empty()
        && head.starts_with(|c: char| c.is_alphabetic() || c == '_')
        && head.chars().all(|c| c.is_alphanumeric() || c == '_');
    named.then(|| head.to_string())
}

/// `<module>__<name>`, and a numbered `<module>2__<name>` if that is taken.
fn fresh(stem: &str, name: &str, taken: &mut HashSet<String>) -> String {
    let mut candidate = format!("{stem}__{name}");
    let mut n = 2;
    while taken.contains(&candidate) {
        candidate = format!("{stem}{n}__{name}");
        n += 1;
    }
    taken.insert(candidate.clone());
    candidate
}

/// Is this name a body-less declaration, whose symbol some library provides?
fn is_foreign(items: &[Stmt], name: &str) -> bool {
    items
        .iter()
        .any(|st| matches!(st, Stmt::Fn(f) if f.name == name && f.body.is_none()))
}

fn reaches(scope: &Scope, from: &Path, to: &Path) -> bool {
    scope.reach.get(from).is_some_and(|s| s.contains(to))
}

/// Every module each module imports, transitively.
fn import_closure(g: &Graph) -> Reach {
    let mut closure: Reach = HashMap::new();
    for (path, module) in &g.parsed {
        let direct = module
            .items
            .iter()
            .filter_map(|item| match item {
                Stmt::Import(im) => import_target(im, path).map(|d| canon(&d)),
                _ => None,
            })
            .collect();
        closure.insert(path.clone(), direct);
    }
    // To a fixpoint rather than by recursion: two modules may import each other,
    // and `g.order` is not a topological order when they do.
    let paths: Vec<PathBuf> = closure.keys().cloned().collect();
    let mut changed = true;
    while changed {
        changed = false;
        for path in &paths {
            let grown: HashSet<PathBuf> = closure[path]
                .iter()
                .filter_map(|d| closure.get(d))
                .flatten()
                .cloned()
                .collect();
            let here = closure.get_mut(path).expect("seeded above");
            for p in grown {
                changed |= here.insert(p);
            }
        }
    }
    closure
}

/// Per module, every name a selective import asked that module for.
fn requested_names(g: &Graph) -> Asked {
    let mut out: Asked = HashMap::new();
    for (path, module) in &g.parsed {
        for item in &module.items {
            let Stmt::Import(im) = item else { continue };
            let Import::Names { names, .. } = im else {
                continue;
            };
            if let Some(dep) = import_target(im, path) {
                out.entry(canon(&dep)).or_default().extend(names.clone());
            }
        }
    }
    out
}

/// Two files, one name, and no way to tell which a third file meant.
fn clashing_definitions(units: &[Unit], kept: &[usize], name: &str) -> String {
    let files: Vec<String> = kept
        .iter()
        .map(|&i| format!("  {}", units[i].path.display()))
        .collect();
    format!(
        "`{name}` is defined by more than one module of this program, and every \
         module is inlined into one:\n{}\n  Both are API, so neither can be \
         moved out of the way. Rename one of them, or ask for the one you mean \
         with `import {{ … }} from …` and keep the other out of the program.",
        files.join("\n")
    )
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

    // A type named in a requested definition's signature is part of that
    // definition however the module feels about it: nothing can call
    // `parse(c: Cmd, …)` without writing `Cmd`, and a `Cmd` renamed to
    // `spec__Cmd` left every caller naming a type that no longer exists. So the
    // public surface grows from the requested names through their signatures,
    // and through the field types of the types those reach.
    let public = surface(items, want);

    let private: Vec<String> = items
        .iter()
        .filter_map(defined_name)
        .filter(|n| !public.contains(*n) && !foreign.contains(*n))
        .map(str::to_string)
        .collect();

    for name in private {
        let qualified = format!("{stem}__{name}");
        for st in items.iter_mut() {
            crate::ast::rename_ident(st, &name, &qualified);
        }
    }
}

/// The names an importer can be expected to write: the ones it asked for, plus
/// every type reachable from their signatures.
///
/// Bodies are deliberately not followed. A helper a public function *calls* is
/// still private — the caller never names it — and following calls would make
/// the whole module public the moment one function was exported.
fn surface<'a>(items: &'a [Stmt], want: &BTreeSet<String>) -> BTreeSet<&'a str> {
    let mut out: BTreeSet<&str> = items
        .iter()
        .filter_map(defined_name)
        .filter(|n| want.contains(*n))
        .collect();
    loop {
        let mut named = BTreeSet::new();
        for st in items {
            if defined_name(st).is_some_and(|n| out.contains(n)) {
                sig_refs(st, &mut named);
            }
        }
        let before = out.len();
        for st in items {
            if let Some(n) = defined_name(st)
                && named.contains(n)
            {
                out.insert(n);
            }
        }
        if out.len() == before {
            return out;
        }
    }
}

/// The type names a definition writes where a caller can see them: a function's
/// parameters and return, a record's field types, a sum's payload types.
fn sig_refs(st: &Stmt, out: &mut BTreeSet<String>) {
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
        }
        // `P = { x: T }` and `S = A(T) | B` are both a binding whose value
        // describes a type, so the declaration is where the field and payload
        // types are written.
        Stmt::Bind(b) => {
            b.tys.iter().for_each(|t| refs_in_type(t, out));
            refs_in_expr(&b.value, out);
        }
        _ => {}
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
    // An imported module's syntax errors are the program's syntax errors.
    // Dropping them left the parser's partial tree to be sliced and inlined, so
    // a file that would not compile on its own compiled to something else once
    // something imported it — `a + b` continued onto the next line is a clean
    // refusal in the entry file and was a silently different expression in a
    // module beside it.
    if !parsed.errors.is_empty() {
        return Err(format!(
            "{}: parse errors:\n  {}",
            path.display(),
            parsed.errors.join("\n  ")
        ));
    }
    g.parsed.insert(key.clone(), parsed.module.clone());
    for item in &parsed.module.items {
        let Stmt::Import(im) = item else { continue };
        match import_resolution(im, path) {
            Some(r) if r.shadowed.is_some() => {
                let other = r.shadowed.expect("just matched");
                return Err(ambiguous(path, im, &r.chosen, &other));
            }
            Some(r) => collect(&r.chosen, g)?,
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

/// One written import, two files.
///
/// Refused rather than warned about. The two candidates are different files
/// with, by construction, the same name, so one of them is being compiled and
/// the other silently is not — and which is which depends on a directory layout
/// the import line does not mention. That is not a preference the author
/// expressed, so there is nothing here to honour; the repair is to rename the
/// directory or to write a path that names one file, and both are the author's
/// to make. It costs correct programs nothing: across the 229 imports in this
/// repository no written path and no search root ever name two different files.
fn ambiguous(importer: &Path, im: &Import, chosen: &Path, other: &Path) -> String {
    let written = import_segments(im).unwrap_or_default().join("/");
    format!(
        "{}: ambiguous import `{written}` — it names two files:\n  \
         {} (as written — the one this build would use)\n  \
         {} (under a search root)\n  \
         A directory sharing a package's name hides the package, and the import \
         line cannot say which was meant. Rename the directory, or move the \
         module so that one path names it.",
        importer.display(),
        chosen.display(),
        other.display()
    )
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
        .filter(|(i, st)| visited.contains(i) || matches!(st, Stmt::Import(Import::Foreign { .. })))
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
        Type::Fn(ps, r) => {
            ps.iter().for_each(|p| refs_in_type(p, out));
            refs_in_type(r, out);
        }
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
