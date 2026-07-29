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
    for cand in [dir.join(&joined), PathBuf::from(&joined)] {
        if cand.is_file() {
            return Some(cand);
        }
    }
    let by_sibling = dir.join(format!("{last}.maca"));
    by_sibling.is_file().then_some(by_sibling)
}

/// The local module a single import statement names, if any. Foreign imports
/// (`import c "…"`, `nixpkgs`, a raw block) resolve to no file.
pub fn import_target(im: &Import, importer: &Path) -> Option<PathBuf> {
    resolve_module_path(&import_segments(im)?, importer)
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
