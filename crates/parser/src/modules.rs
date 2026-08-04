use crate::ast::Import;
use std::path::{Path, PathBuf};

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
        let mut out = Layout::default();
        if let Some(v) = layout_key(toml, "modules") {
            out.roots[0] = v;
        }
        if let Some(v) = layout_key(toml, "src") {
            out.roots[1] = v;
        }
        if let Some(v) = layout_key(toml, "apps") {
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
        .find(|d| d.join("maca.toml").is_file())
        .map(Path::to_path_buf)
}

/// The layout in force for a file: its project's `[layout]`, or the defaults.
pub fn layout_for(importer: &Path) -> Layout {
    let dir = importer.parent().unwrap_or_else(|| Path::new("."));
    project_root(dir)
        .or_else(|| project_root(&std::env::current_dir().ok()?))
        .and_then(|r| std::fs::read_to_string(r.join("maca.toml")).ok())
        .map_or_else(Layout::default, |t| Layout::from_manifest(&t))
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
    let dir = importer.parent().unwrap_or_else(|| Path::new("."));
    let joined = segs.join("/");
    let layout = layout_for(importer);
    let mut found: Vec<(PathBuf, Found)> = Vec::new();

    for base in dir.ancestors() {
        candidates(base, &joined, &layout, &mut found);
        if base.join("maca.toml").is_file() {
            break;
        }
    }
    if found.is_empty() && project_root(dir).is_none() {
        candidates(Path::new("."), &joined, &layout, &mut found);
    }
    if found.is_empty() {
        return module_file(&dir.join(last)).map(|chosen| Resolved {
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

/// The same file reached two ways.
fn same_file(a: &Path, b: &Path) -> bool {
    let real = |p: &Path| std::fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf());
    real(a) == real(b)
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
