//! Which file an `import` names, and where a project keeps its code.
//!
//! This lives beside the `Import` AST rather than in the driver because two
//! things need it and they must agree: the build inlines the module it
//! resolves to, and the language server renames across the same edge. A
//! second copy of these rules would let go-to-definition and the compiler
//! disagree about which `list.maca` a program means.
//!
//! # Layout
//!
//! The recommended monorepo shape is `modules/` beside `apps/`: `modules/*` are
//! the packages — the code meant to be imported, and what this repository
//! publishes — and `apps/*` are the programs built out of them. A single-package
//! repository can use `src/*` instead and needs no manifest entry for it. Both
//! are searched by default, so neither is a decision anyone has to make up
//! front, and `maca.toml` can rename either:
//!
//! ```toml
//! [layout]
//! modules = "packages"
//! apps    = "services"
//! ```
//!
//! `modules` and `src` are search roots — `import http` looks under them.
//! `apps` is not: an application is imported by its written path
//! (`import apps/tomo/conf`), because two apps may both have a `conf` and
//! neither should silently answer for the other.
//!
//! # `_init.maca`
//!
//! A package may be one file (`modules/http.maca`) or a directory with an
//! entry module (`modules/http/_init.maca`). The two are interchangeable to an
//! importer — `import http` finds either — so a module can grow from a file
//! into a directory without touching a line at any call site. The entry module
//! is where a package says what its name means, which is what makes
//! `maca -m http.serve` read the way it does.

use crate::ast::Import;
use std::path::{Path, PathBuf};

/// The directory names a project keeps code under.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Layout {
    /// Import search roots under the project root, in order.
    pub roots: Vec<String>,
    /// Where applications live. Not a search root — see the module docs.
    pub apps: String,
}

impl Default for Layout {
    fn default() -> Self {
        Layout {
            roots: vec!["modules".into(), "src".into()],
            apps: "apps".into(),
        }
    }
}

impl Layout {
    /// Read `[layout]` out of a `maca.toml`. Unset keys keep their defaults, so
    /// a project that never mentions layout gets `modules`, `src` and `apps`.
    pub fn from_manifest(toml: &str) -> Layout {
        let mut out = Layout::default();
        if let Some(v) = layout_key(toml, "modules") {
            out.roots = vec![v, "src".into()];
        }
        if let Some(v) = layout_key(toml, "src") {
            // A polyrepo naming its own source root replaces `src`, not
            // `modules` — a repository may legitimately have both.
            let n = out.roots.len();
            out.roots[n - 1] = v;
        }
        if let Some(v) = layout_key(toml, "apps") {
            out.apps = v;
        }
        out
    }
}

/// `key = "value"` inside `[layout]`, ignoring other sections.
///
/// Scanned line by line rather than searched, so a commented-out key is a
/// comment. (`apps/tomo/conf.maca` learned that the hard way about `book.toml`.)
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

/// The file `import a/b` names, relative to the importing file.
///
/// The written path wins, from the importer's directory upward and then from
/// the working directory — that is what reaches `apps/tomo/conf.maca` from
/// `tools/`. Each of those directories is also tried as a project root, so
/// `import std/text` finds `modules/std/text.maca` from anywhere in the tree.
/// Either form may be a file or a directory with an `_init.maca`.
///
/// The bare-sibling rule comes last on purpose: it is a convenience for
/// `import selfhost/token` next to its siblings, and taking it first meant any
/// `list.maca` beside a program silently shadowed `std/list` with no
/// diagnostic at all.
pub fn resolve_module_path(segs: &[String], importer: &Path) -> Option<PathBuf> {
    let last = segs.last()?;
    let dir = importer.parent().unwrap_or_else(|| Path::new("."));
    let joined = segs.join("/");
    let layout = layout_for(importer);

    // The importer's own directory, then every directory above it up to the
    // project root, then the working directory. Walking up is what lets a
    // program deep in the tree say `import std/text` and mean the project's
    // own — resolving only against the working directory made a build depend on
    // where it was started from, and `cargo test`, which runs in a crate
    // directory, could not resolve an example's imports at all.
    for base in dir.ancestors() {
        if let Some(hit) = in_base(base, &joined, &layout) {
            return Some(hit);
        }
        // Stop at the project root. Without this the walk continues into `$HOME`
        // and `/`, where a stray `std/` becomes the standard library for every
        // project beneath it — and the language server, whose own search is
        // bounded by the workspace, would then disagree with the compiler about
        // which file a name is defined in.
        if base.join("maca.toml").is_file() {
            break;
        }
    }
    // The working directory, but only when the importer has no project of its
    // own. A file inside a project resolves against *that* project or not at
    // all: building `projA/app.maca` from inside `projB` used to find projB's
    // `modules/`, which is a build whose meaning depends on where it was
    // started — and the language server, bounded by its workspace, could never
    // agree with it.
    if project_root(dir).is_none()
        && let Some(hit) = in_base(Path::new("."), &joined, &layout)
    {
        return Some(hit);
    }
    module_file(&dir.join(last))
}

/// `<base>/<path>`, then each search root under `base`. A hit is either
/// `<path>.maca` or `<path>/_init.maca`.
fn in_base(base: &Path, path: &str, layout: &Layout) -> Option<PathBuf> {
    module_file(&base.join(path)).or_else(|| {
        layout
            .roots
            .iter()
            .find_map(|r| module_file(&base.join(r).join(path)))
    })
}

/// The module at `stem`: the file `stem.maca`, or the directory `stem/` entered
/// through its `_init.maca`.
fn module_file(stem: &Path) -> Option<PathBuf> {
    let flat = stem.with_extension("maca");
    if flat.is_file() {
        return Some(flat);
    }
    let init = stem.join("_init.maca");
    init.is_file().then_some(init)
}

/// The local module a single import statement names, if any. Foreign imports
/// (`import c "…"`, `nixpkgs`, a raw block) resolve to no file.
pub fn import_target(im: &Import, importer: &Path) -> Option<PathBuf> {
    resolve_module_path(&import_segments(im)?, importer)
}

/// Does this import name a file it is an error not to find?
///
/// A slash path — `import std/text`, `import { lines } from std/text` — names a
/// module and nothing else, so failing to resolve one is a typo. A bare
/// `import nixpkgs` is not: it may be a sibling module or the builtin, and the
/// two are told apart only by whether the file is there.
///
/// `import std/str` sat in four files for months. Nothing resolved it and
/// nothing said so, and each of those files then hand-wrote the helpers it
/// thought it was importing.
pub fn names_a_file(im: &Import) -> bool {
    match im {
        Import::Module(segs) => segs.len() > 1,
        // A selective import can only mean a local module — there is nothing to
        // select from a builtin — so a single word is as much a promise as a
        // slash path. `import { greet } from lib` with no `lib` used to resolve
        // to nothing, silently, and the program failed at the linker instead.
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
        assert_eq!(l.roots, vec!["modules".to_string(), "src".to_string()]);
        assert_eq!(l.apps, "apps");
    }

    #[test]
    fn layout_keys_are_renameable() {
        let l = Layout::from_manifest("[layout]\nmodules = \"packages\"\napps = \"services\"\n");
        assert_eq!(l.roots, vec!["packages".to_string(), "src".to_string()]);
        assert_eq!(l.apps, "services");
    }

    #[test]
    fn a_polyrepo_renames_only_its_source_root() {
        let l = Layout::from_manifest("[layout]\nsrc = \"lib\"\n");
        assert_eq!(l.roots, vec!["modules".to_string(), "lib".to_string()]);
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
