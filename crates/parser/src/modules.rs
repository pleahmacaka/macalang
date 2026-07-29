//! Which file an `import` names.
//!
//! This lives beside the `Import` AST rather than in the driver because two
//! things need it and they must agree: the build inlines the module it
//! resolves to, and the language server renames across the same edge. A
//! second copy of these rules would let go-to-definition and the compiler
//! disagree about which `list.maca` a program means.

use crate::ast::Import;
use std::path::{Path, PathBuf};

/// The file `import a/b` names, relative to the importing file.
///
/// The written path wins, from the importer's directory and then from the
/// working directory — that is what reaches `std/list.maca` in the repository.
/// The bare-sibling rule comes last on purpose: it is a convenience for
/// `import selfhost/token` next to its siblings, and taking it first meant any
/// `list.maca` beside a program silently shadowed `std/list` with no
/// diagnostic at all.
pub fn resolve_module_path(segs: &[String], importer: &Path) -> Option<PathBuf> {
    let last = segs.last()?;
    let dir = importer.parent().unwrap_or_else(|| Path::new("."));
    let joined = format!("{}.maca", segs.join("/"));

    // The importer's own directory, then every directory above it, then the
    // working directory. Walking up is what lets a program deep in the tree say
    // `import std/text` and mean the project's `std/` — resolving only against
    // the working directory made a build depend on where it was started from,
    // and `cargo test`, which runs in a crate directory, could not resolve an
    // example's imports at all.
    for base in dir.ancestors() {
        let cand = base.join(&joined);
        if cand.is_file() {
            return Some(cand);
        }
    }
    let cwd = PathBuf::from(&joined);
    if cwd.is_file() {
        return Some(cwd);
    }
    let by_sibling = dir.join(format!("{last}.maca"));
    by_sibling.is_file().then_some(by_sibling)
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
        Import::Names { module, .. } => module.len() > 1,
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
