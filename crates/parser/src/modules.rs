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
//! the packages, the code meant to be imported and what this repository
//! publishes, and `apps/*` are the programs built out of them. A single-package
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
//! `modules`, `src` and `maca_modules` are search roots. `apps` is not: an
//! application is imported by its written path (`import apps/tomo/conf`),
//! because two apps may both have a `conf` and neither should silently answer
//! for the other.
//!
//! # A path is a path
//!
//! There is no entry file and no index. `modules/http/server.maca` is
//! `http/server`, and that is the only thing it is:
//!
//! ```text
//! import { listen } from http/server
//! ```
//!
//! An installed dependency is the same: `maca_modules/toml/parse.maca` is
//! `toml/parse`, not `maca_modules/toml/parse`. Where a package physically
//! lives is the project's business; what you write is the name.
//!
//! The alternative was a per-directory entry module re-exporting its
//! neighbours, and it cost more than it bought: two names for every file, a
//! second place to update when one moved, and an import whose meaning depended
//! on a file the reader never opened. A path says where a thing is, and reading
//! it is the whole point.

use crate::ast::Import;
use std::path::{Path, PathBuf};

/// The directory names a project keeps code under.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Layout {
    /// Import search roots under the project root, in order.
    pub roots: Vec<String>,
    /// Where applications live. Not a search root; see the module docs.
    pub apps: String,
}

/// Where `maca add` installs a dependency. A search root like the others, so an
/// installed package is written by its own name and the directory it happens to
/// sit in never appears in anybody's source.
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
    /// Read `[layout]` out of a `maca.toml`. Unset keys keep their defaults, so
    /// a project that never mentions layout gets `modules`, `src` and `apps`.
    pub fn from_manifest(toml: &str) -> Layout {
        let mut out = Layout::default();
        if let Some(v) = layout_key(toml, "modules") {
            out.roots[0] = v;
        }
        if let Some(v) = layout_key(toml, "src") {
            // A polyrepo naming its own source root replaces `src`, not
            // `modules`; a repository may legitimately have both.
            out.roots[1] = v;
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
    /// A second, different file the same written import also names, one found
    /// as a written path and one under a search root. Nothing in the source
    /// says which of the two won, so this is an ambiguity to report rather than
    /// a precedence to rely on; see `candidates`.
    pub shadowed: Option<PathBuf>,
}

/// The file `import a/b` names, relative to the importing file.
///
/// The written path wins, from the importer's directory upward and then from
/// the working directory. That is what reaches `apps/tomo/conf.maca` from
/// `tools/`. Each of those directories is also tried as a project root, so
/// `import std/text` finds `modules/std/text.maca` from anywhere in the tree.
pub fn resolve_module_path(segs: &[String], importer: &Path) -> Option<PathBuf> {
    resolve_module(segs, importer).map(|r| r.chosen)
}

/// `resolve_module_path`, and what else the same import names.
///
/// The bare-sibling rule comes last on purpose: it is a convenience for
/// `import selfhost/token` next to its siblings, and taking it first meant any
/// `list.maca` beside a program silently shadowed `std/list` with no
/// diagnostic at all.
pub fn resolve_module(segs: &[String], importer: &Path) -> Option<Resolved> {
    let last = segs.last()?;
    let dir = importer.parent().unwrap_or_else(|| Path::new("."));
    let joined = segs.join("/");
    let layout = layout_for(importer);
    let mut found: Vec<(PathBuf, Found)> = Vec::new();

    // The importer's own directory, then every directory above it up to the
    // project root, then the working directory. Walking up is what lets a
    // program deep in the tree say `import std/text` and mean the project's
    // own. Resolving only against the working directory made a build depend on
    // where it was started from, and `cargo test`, which runs in a crate
    // directory, could not resolve an example's imports at all.
    for base in dir.ancestors() {
        candidates(base, &joined, &layout, &mut found);
        // Stop at the project root. Without this the walk continues into `$HOME`
        // and `/`, where a stray `std/` becomes the standard library for every
        // project beneath it, and the language server, whose own search is
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
    // started, and the language server, bounded by its workspace, could never
    // agree with it.
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
///
/// A written path against a source root, either way round: nothing the author
/// wrote distinguishes `bench/stat` the directory from `bench/stat` the
/// package, so whichever wins, the loser is invisible.
///
/// An installed dependency losing to the project's own source is *not* that.
/// `maca_modules` is a directory `maca add` created, not one anybody wrote, and
/// a project's own copy outranking a vendored one is the documented
/// precedence. See `an_installed_dependency_does_not_outrank_the_projects_own_source`.
fn hides(chosen: Found, other: Found) -> bool {
    matches!(
        (chosen, other),
        (Found::Written, Found::Root) | (Found::Root, Found::Written)
    )
}

/// Every file `<base>` offers for one written import, best first: `<base>/<path>`,
/// then `<base>/<root>/<path>` for each search root in order.
///
/// The written path comes first, and that has a cost worth knowing: a
/// directory that shares a package's name shadows it. The tree had exactly that
/// shape, a top-level `bench/` beside `modules/bench/`, whose files the
/// benchmark subsystem imports as `bench/…`, and it was closed by moving the
/// directory, not by reordering here. What is new is that the shadowing is no
/// longer silent: both candidates are collected, and an import that names two
/// files is refused by `imports::collect` naming both.
///
/// Reordering was tried and reverted. It does not fix the general case: the
/// ancestor walk visits each directory in turn, so `apps/bench/report.maca`
/// still answers for `bench/report` from anywhere under `apps/`, whatever the
/// order within one directory. And it makes things worse elsewhere:
/// `maca_modules` is a search root, so roots-first lets an installed
/// dependency outrank the project's own source. Across the 229 imports in this
/// repository the two orderings agree on every one, which is also why refusing
/// an ambiguous import costs those imports nothing.
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

/// The same file reached two ways. `modules/std/text.maca` is both the written
/// path from inside `modules/` and the `modules` root's answer from the project
/// root, and that is one file, not an ambiguity.
fn same_file(a: &Path, b: &Path) -> bool {
    let real = |p: &Path| std::fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf());
    real(a) == real(b)
}

/// The module at `stem`: the file `stem.maca`, and nothing else.
fn module_file(stem: &Path) -> Option<PathBuf> {
    let flat = stem.with_extension("maca");
    flat.is_file().then_some(flat)
}

/// The local module a single import statement names, if any. Foreign imports
/// (`import c "…"`, `nixpkgs`, a raw block) resolve to no file.
pub fn import_target(im: &Import, importer: &Path) -> Option<PathBuf> {
    import_resolution(im, importer).map(|r| r.chosen)
}

/// `import_target`, and what else that import statement names.
pub fn import_resolution(im: &Import, importer: &Path) -> Option<Resolved> {
    resolve_module(&import_segments(im)?, importer)
}

/// Does this import name a file it is an error not to find?
///
/// A slash path (`import std/text`, `import { lines } from std/text`) names a
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
        // A selective import can only mean a local module, since there is
        // nothing to select from a builtin, so a single word is as much a
        // promise as a slash path. `import { greet } from lib` with no `lib`
        // used to resolve to nothing, silently, and the program failed at the
        // linker instead.
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
