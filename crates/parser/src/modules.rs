use crate::ast::Import;
use std::cell::RefCell;
use std::path::{Path, PathBuf};

thread_local! {
    /// The project that last asked for a module, which is the project the compiler's own copy of a package resolves its own imports against.
    static ASKED_BY: RefCell<Option<PathBuf>> = const { RefCell::new(None) };
}

/// The directory names a project keeps code under.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Layout {
    /// Import search roots under the project root, in order.
    pub roots: Vec<String>,
    /// Where applications live.
    pub apps: String,
}

/// Where `maca add` installs a dependency.
pub const INSTALLED: &str = "maca_modules";

/// The file a project's settings live in.
pub const MANIFEST: &str = "maca.toml";

impl Default for Layout {
    fn default() -> Self {
        Layout {
            roots: vec!["modules".into(), "src".into(), INSTALLED.into()],
            apps: "apps".into(),
        }
    }
}

impl Layout {
    /// Read `[layout]` out of a `maca.toml`.
    pub fn from_manifest(toml: &str) -> Layout {
        Layout::from_chain(&[toml.to_string()])
    }

    /// Read `[layout]` off a manifest chain, the nearest manifest that states a key answering for it.
    pub fn from_chain(texts: &[String]) -> Layout {
        let mut out = Layout::default();
        let nearest = |key: &str| texts.iter().find_map(|t| layout_key(t, key));
        if let Some(v) = nearest("modules") {
            out.roots[0] = v;
        }
        if let Some(v) = nearest("src") {
            out.roots[1] = v;
        }
        if let Some(v) = nearest("apps") {
            out.apps = v;
        }
        out
    }
}

/// `key = "value"` inside `[layout]`, ignoring other sections.
fn layout_key(toml: &str, key: &str) -> Option<String> {
    let mut in_layout = false;
    for line in toml.lines() {
        let t = line.trim();
        if t.starts_with('[') {
            in_layout = t.starts_with("[layout]");
            continue;
        }
        if !in_layout || !t.starts_with(key) {
            continue;
        }
        let rest = t[key.len()..].trim_start();
        if let Some(v) = rest.strip_prefix('=') {
            let v = v.trim().trim_matches('"').trim();
            if !v.is_empty() {
                return Some(v.to_string());
            }
        }
    }
    None
}

/// The nearest directory at or above `from` holding a `maca.toml`.
pub fn project_root(from: &Path) -> Option<PathBuf> {
    from.ancestors()
        .find(|d| d.join(MANIFEST).is_file())
        .map(Path::to_path_buf)
}

/// Does this manifest declare the workspace its neighbours are members of?
pub fn declares_workspace(toml: &str) -> bool {
    toml.lines().any(|l| l.trim() == "[workspace]")
}

/// A directory as the walk above it needs it: rooted and folded, because a relative path has no ancestors above the directory it was written from, and a `..` left in one makes the same directory look like two.
fn rooted(dir: &Path) -> PathBuf {
    let cwd = || std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let full = if dir.is_absolute() {
        dir.to_path_buf()
    } else if dir.as_os_str().is_empty() || dir == Path::new(".") {
        cwd()
    } else {
        cwd().join(dir)
    };
    let mut out = PathBuf::new();
    for part in full.components() {
        match part {
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                out.pop();
            }
            other => out.push(other),
        }
    }
    out
}

/// Every directory at or above `from` holding a `maca.toml`, nearest first.
fn manifest_dirs(from: &Path) -> Vec<PathBuf> {
    rooted(from)
        .ancestors()
        .filter(|d| d.join(MANIFEST).is_file())
        .map(Path::to_path_buf)
        .collect()
}

/// The directory a manifest chain ends at: the outermost `[workspace]`, or the nearest manifest when no workspace holds it.
pub fn workspace_root(from: &Path) -> Option<PathBuf> {
    let dirs = manifest_dirs(from);
    dirs.iter()
        .rev()
        .find(|d| {
            std::fs::read_to_string(d.join(MANIFEST))
                .map(|t| declares_workspace(&t))
                .unwrap_or(false)
        })
        .or_else(|| dirs.first())
        .cloned()
}

/// The manifests that answer for `from`, nearest first, ending at the workspace root.
pub fn manifest_chain(from: &Path) -> Vec<PathBuf> {
    let Some(stop) = workspace_root(from) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for dir in manifest_dirs(from) {
        let last = dir == stop;
        out.push(dir);
        if last {
            break;
        }
    }
    out
}

/// The layout in force for a file: its manifest chain's `[layout]`, or the defaults.
pub fn layout_for(importer: &Path) -> Layout {
    let dir = importer.parent().unwrap_or_else(|| Path::new("."));
    let mut chain = manifest_chain(dir);
    if chain.is_empty()
        && let Ok(cwd) = std::env::current_dir()
    {
        chain = manifest_chain(&cwd);
    }
    let texts: Vec<String> = chain
        .iter()
        .filter_map(|d| std::fs::read_to_string(d.join(MANIFEST)).ok())
        .collect();
    Layout::from_chain(&texts)
}

/// Which rule turned up a candidate file for one written import.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Found {
    /// `<base>/<the path exactly as written>`.
    Written,
    /// `<base>/<one of the project's own source roots>/<the path>`.
    Root,
    /// `<base>/maca_modules/<the path>`: an installed dependency.
    Installed,
}

/// The file an import resolves to, and the file that resolution hid.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Resolved {
    /// The file the import means.
    pub chosen: PathBuf,
    /// A second, different file the same written import also names, one found as a written path and one under a search root.
    pub shadowed: Option<PathBuf>,
}

/// The file `import a/b` names, relative to the importing file.
pub fn resolve_module_path(segs: &[String], importer: &Path) -> Option<PathBuf> {
    resolve_module(segs, importer).map(|r| r.chosen)
}

/// `resolve_module_path`, and what else the same import names.
pub fn resolve_module(segs: &[String], importer: &Path) -> Option<Resolved> {
    let last = segs.last()?;
    let own = importer.parent().unwrap_or_else(|| Path::new("."));
    let joined = segs.join("/");
    let asked = asking(own);
    let dir = asked.as_path();
    let layout = layout_for(&dir.join("_m.maca"));
    let mut found: Vec<(PathBuf, Found)> = Vec::new();

    let stop = workspace_root(dir);
    for base in dir.ancestors() {
        candidates(base, &joined, &layout, &mut found);
        if stop.as_deref().is_some_and(|s| same_dir(base, s)) {
            break;
        }
    }
    if found.is_empty() && stop.is_none() {
        candidates(Path::new("."), &joined, &layout, &mut found);
    }
    if found.is_empty() {
        let chosen = module_file(&own.join(last)).or_else(|| builtin(&joined))?;
        return Some(Resolved {
            chosen,
            shadowed: None,
        });
    }
    let (chosen, rule) = found[0].clone();
    let shadowed = found[1..]
        .iter()
        .find(|(p, other)| hides(rule, *other) && !same_file(p, &chosen))
        .map(|(p, _)| p.clone());
    Some(Resolved { chosen, shadowed })
}

/// Do these two rules finding two different files make the import ambiguous?
fn hides(chosen: Found, other: Found) -> bool {
    matches!(
        (chosen, other),
        (Found::Written, Found::Root) | (Found::Root, Found::Written)
    )
}

/// Every file `<base>` offers for one written import, best first.
fn candidates(base: &Path, path: &str, layout: &Layout, out: &mut Vec<(PathBuf, Found)>) {
    if let Some(hit) = module_file(&base.join(path)) {
        out.push((hit, Found::Written));
    }
    for r in &layout.roots {
        if let Some(hit) = module_file(&base.join(r).join(path)) {
            let rule = if r == INSTALLED {
                Found::Installed
            } else {
                Found::Root
            };
            out.push((hit, rule));
        }
    }
}

/// The standard library the compiler carries, which answers for an import only once the project has been asked and had nothing.
fn builtin(path: &str) -> Option<PathBuf> {
    module_file(&maca_stdlib::root()?.join(path))
}

/// The directory whose project answers an import: the importing file's own, unless that file is the compiler's copy of a package, whose imports belong to the project that asked for it.
fn asking(dir: &Path) -> PathBuf {
    if in_builtin(dir) {
        if let Some(project) = ASKED_BY.with(|p| p.borrow().clone()) {
            return project;
        }
        return dir.to_path_buf();
    }
    let here = rooted(dir);
    ASKED_BY.with(|p| *p.borrow_mut() = Some(here));
    dir.to_path_buf()
}

/// Is this file the compiler's own copy of a package rather than one of the project's?
fn in_builtin(dir: &Path) -> bool {
    rooted(dir).starts_with(rooted(&maca_stdlib::holder()))
}

/// The directory `maca add` installed `name` into, found by the walk an `import` takes.
pub fn installed_dir(name: &str, importer: &Path) -> Option<PathBuf> {
    let dir = importer.parent().unwrap_or_else(|| Path::new("."));
    let stop = workspace_root(dir);
    for base in dir.ancestors() {
        let cand = base.join(INSTALLED).join(name);
        if cand.is_dir() {
            return Some(cand);
        }
        if stop.as_deref().is_some_and(|s| same_dir(base, s)) {
            break;
        }
    }
    None
}

/// The directory a package spec names and the file inside it the spec asked for, with a leading `@scope/` dropped the way `maca add` drops it.
pub fn split_package(spec: &str) -> (String, Option<String>) {
    let spec = spec.trim().trim_start_matches('/');
    let rest = match spec.strip_prefix('@').and_then(|s| s.split_once('/')) {
        Some((_, rest)) => rest,
        None => spec,
    };
    match rest.split_once('/') {
        Some((name, sub)) if !sub.is_empty() => (name.to_string(), Some(sub.to_string())),
        _ => (rest.trim_end_matches('/').to_string(), None),
    }
}

/// The same file reached two ways.
fn same_file(a: &Path, b: &Path) -> bool {
    let real = |p: &Path| std::fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf());
    real(a) == real(b)
}

/// The same directory named relatively and absolutely, which is how a walk over written paths meets a rooted one.
fn same_dir(a: &Path, b: &Path) -> bool {
    rooted(a) == rooted(b) || same_file(&rooted(a), b)
}

/// The module at `stem`: the file `stem.maca`, and nothing else.
fn module_file(stem: &Path) -> Option<PathBuf> {
    let flat = stem.with_extension("maca");
    flat.is_file().then_some(flat)
}

/// The local module a single import statement names, if any.
pub fn import_target(im: &Import, importer: &Path) -> Option<PathBuf> {
    import_resolution(im, importer).map(|r| r.chosen)
}

/// `import_target`, and what else that import statement names.
pub fn import_resolution(im: &Import, importer: &Path) -> Option<Resolved> {
    resolve_module(&import_segments(im)?, importer)
}

/// Does this import name a file it is an error not to find?
pub fn names_a_file(im: &Import) -> bool {
    match im {
        Import::Module(segs) => segs.len() > 1,
        Import::Names { .. } => true,
        _ => false,
    }
}

/// The path segments an import writes, for the shapes that name a local module.
pub fn import_segments(im: &Import) -> Option<Vec<String>> {
    match im {
        Import::Module(segs) => Some(segs.clone()),
        Import::Bare(name) => Some(vec![name.clone()]),
        Import::Names { module, .. } => Some(module.clone()),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layout_defaults_need_no_manifest_entry() {
        let l = Layout::from_manifest("[package]\nname = \"x\"\n");
        assert_eq!(l.roots[0], "modules");
        assert_eq!(l.roots[1], "src");
        assert_eq!(l.apps, "apps");
    }

    #[test]
    fn layout_keys_are_renameable() {
        let l = Layout::from_manifest("[layout]\nmodules = \"packages\"\napps = \"services\"\n");
        assert_eq!(l.roots[0], "packages");
        assert_eq!(l.apps, "services");
    }

    #[test]
    fn a_polyrepo_renames_only_its_source_root() {
        let l = Layout::from_manifest("[layout]\nsrc = \"lib\"\n");
        assert_eq!(l.roots[0], "modules");
        assert_eq!(l.roots[1], "lib");
    }

    #[test]
    fn the_nearest_manifest_that_states_a_key_answers_for_it() {
        let root = "[workspace]\n[layout]\nmodules = \"packages\"\napps = \"services\"\n";
        let member = "[layout]\nsrc = \"lib\"\n".to_string();
        let l = Layout::from_chain(&[member, root.to_string()]);
        assert_eq!(l.roots[0], "packages", "inherited from the root");
        assert_eq!(l.roots[1], "lib", "stated by the member");
        assert_eq!(l.apps, "services", "inherited from the root");
    }

    #[test]
    fn a_member_key_beats_the_root_key_it_repeats() {
        let root = "[layout]\nmodules = \"packages\"\n".to_string();
        let member = "[layout]\nmodules = \"mine\"\n".to_string();
        assert_eq!(Layout::from_chain(&[member, root]).roots[0], "mine");
    }

    #[test]
    fn a_workspace_is_the_table_that_says_so() {
        assert!(declares_workspace(
            "[package]\nname = \"x\"\n\n[workspace]\n"
        ));
        assert!(!declares_workspace("[package]\nname = \"x\"\n"));
        assert!(
            !declares_workspace("members = [\"a\"]\n"),
            "the key without the table is not the table"
        );
    }

    #[test]
    fn a_package_spec_splits_into_a_directory_and_a_file() {
        assert_eq!(split_package("daisyui"), ("daisyui".into(), None));
        assert_eq!(
            split_package("daisyui/dist/full.css"),
            ("daisyui".into(), Some("dist/full.css".into()))
        );
    }

    #[test]
    fn a_scoped_name_installs_under_its_bare_half() {
        assert_eq!(split_package("@scope/pkg"), ("pkg".into(), None));
        assert_eq!(
            split_package("@scope/pkg/dist/x.js"),
            ("pkg".into(), Some("dist/x.js".into()))
        );
    }

    /// A key outside `[layout]`, and a commented-out one, are not the layout.
    #[test]
    fn only_the_layout_section_counts() {
        let l = Layout::from_manifest(
            "[layout]\n# modules = \"old\"\nmodules = \"pkg\"\n\n[build]\napps = \"nope\"\n",
        );
        assert_eq!(l.roots[0], "pkg");
        assert_eq!(l.apps, "apps");
    }
}
