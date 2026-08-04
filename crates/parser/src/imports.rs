use crate::ast::*;
use crate::modules::{import_resolution, import_segments, import_target, names_a_file};
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::path::{Path, PathBuf};

/// Read a program and inline every local module it imports (transitively).
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

/// Every module a program inlines that carries an `import <lang> "…"` block, in dependency order.
pub fn modules_importing(entry: &Path, lang: &str) -> Result<Vec<PathBuf>, String> {
    let mut g = Graph::default();
    collect(entry, &mut g)?;
    Ok(g.order
        .iter()
        .filter(|p| {
            g.parsed[*p]
                .items
                .iter()
                .any(|it| matches!(it, Stmt::Import(Import::Foreign { lang: l, .. }) if l == lang))
        })
        .cloned()
        .collect())
}

/// One module's contribution to the flat translation unit.
struct Unit {
    path: PathBuf,
    /// The module's own text.
    src: String,
    /// What the module contributes: every item for a whole-module import, the requested slice for a selective one.
    items: Vec<Stmt>,
    /// Whether `src` still says what `items` do.
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
    /// Names written as a field or inside a pattern anywhere in the program, which is where a rename cannot follow them.
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

/// A top level answering for a name another module binds locally.
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

/// Which of `names` this module gives a meaning of its own to.
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
fn is_api(unit: &Unit, name: &str, asked: &Asked) -> bool {
    documented(&unit.src).contains(name)
        || asked.get(&unit.path).is_some_and(|ns| ns.contains(name))
}

/// The names carrying a `///` block directly above them.
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

/// The name a top-level declaration line introduces.
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
fn qualify_private(items: &mut [Stmt], want: &BTreeSet<String>, path: &Path) {
    let Some(stem) = path.file_stem().map(|s| s.to_string_lossy().into_owned()) else {
        return;
    };
    let foreign: HashSet<&str> = items
        .iter()
        .filter_map(|st| match st {
            Stmt::Fn(f) if f.body.is_none() => Some(f.name.as_str()),
            _ => None,
        })
        .collect();

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

/// The names an importer can be expected to write.
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

/// The type names a definition writes where a caller can see them.
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
        Stmt::Bind(b) => {
            b.tys.iter().for_each(|t| refs_in_type(t, out));
            refs_in_expr(&b.value, out);
        }
        _ => {}
    }
}

/// How much of a module to inline.
enum Sel {
    /// The whole module (a `import a/b` / `import a` somewhere wants it, or it is the entry file).
    All,
    /// Only these top-level names (from `import { … } from a/b`), later grown to their same-module dependency closure.
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
fn collect(path: &Path, g: &mut Graph) -> Result<(), String> {
    let key = canon(path);
    if g.parsed.contains_key(&key) {
        return Ok(());
    }
    let src = read(path)?;
    let parsed = crate::parse(&src);
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
                    "{}: no module `{written}`: `{written}.maca` is not beside \
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
fn ambiguous(importer: &Path, im: &Import, chosen: &Path, other: &Path) -> String {
    let written = import_segments(im).unwrap_or_default().join("/");
    format!(
        "{}: ambiguous import `{written}`: it names two files:\n  \
         {} (as written, the one this build would use)\n  \
         {} (under a search root)\n  \
         A directory sharing a package's name hides the package, and the import \
         line cannot say which was meant. Rename the directory, or move the \
         module so that one path names it.",
        importer.display(),
        chosen.display(),
        other.display()
    )
}

/// Effective selection per module.
fn resolve_selection(entry: &Path, g: &Graph) -> HashMap<PathBuf, Sel> {
    let mut sel: HashMap<PathBuf, Sel> = HashMap::new();
    sel.insert(canon(entry), Sel::All);

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
                    Import::Module(_) | Import::Bare(_) => None,
                    Import::Names { names, .. } => Some(names.clone()),
                    _ => continue,
                };
                changed |= merge(&mut sel, dep, want);
            }
        }
    }
    sel
}

/// Fold one import edge's request into `sel[dep]`.
fn merge(sel: &mut HashMap<PathBuf, Sel>, dep: PathBuf, want: Option<Vec<String>>) -> bool {
    match (sel.get_mut(&dep), want) {
        (Some(Sel::All), _) => false,
        (_, None) => {
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

/// Slice a module down to `want` plus the transitive closure of the same-module definitions those items reference.
fn slice_module(
    module: &Module,
    want: &BTreeSet<String>,
    path: &Path,
) -> Result<Vec<Stmt>, String> {
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

    Ok(module
        .items
        .iter()
        .enumerate()
        .filter(|(i, st)| visited.contains(i) || matches!(st, Stmt::Import(Import::Foreign { .. })))
        .map(|(_, st)| st.clone())
        .collect())
}

/// The variant names of a sum-type value `A | B(x) | C`.
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
            Expr::Call { callee, .. } => {
                if let Expr::Ident(n) = &**callee {
                    out.push(n.clone());
                }
            }
            _ => {}
        }
    }
    let mut out = Vec::new();
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
        Expr::Return(v) => {
            if let Some(x) = v {
                refs_in_expr(x, out);
            }
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
