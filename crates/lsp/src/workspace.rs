//! Renaming across files.
//!
//! A top-level definition is visible to every module that imports it, so a
//! rename that edits only the open file leaves those importers calling a name
//! that no longer exists. The editor reports success and the build breaks — the
//! one outcome a rename must not have.
//!
//! Locals and fields stay single-file: a local cannot be seen from another
//! module, and a field would need the record's type followed across the import
//! graph, which the checker knows and this does not.

use crate::binding::{Binding, Scope};
use maca_parser::modules::import_target;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// Every file the rename has to touch, with the spans to replace in each.
///
/// The edited file's own text is passed in rather than read, because the
/// editor's buffer is the truth and may not be saved yet.
pub fn rename_edits(
    root: &Path,
    file: &Path,
    text: &str,
    binding: &Binding,
) -> BTreeMap<PathBuf, Vec<(usize, usize)>> {
    let mut out = BTreeMap::new();
    let here = crate::binding::spans(text, binding);
    if !here.is_empty() {
        out.insert(file.to_path_buf(), here);
    }
    if binding.scope != Scope::TopLevel {
        return out;
    }

    let owner = defining_module(root, file, text, &binding.name).unwrap_or_else(|| canon(file));
    for other in maca_sources(root) {
        if canon(&other) == canon(file) {
            continue;
        }
        let Ok(src) = std::fs::read_to_string(&other) else {
            continue;
        };
        // Only a module that can actually see the definition — one that
        // imports the module defining it, or defines it itself.
        if canon(&other) != owner && !imports(&other, &src, &owner) {
            continue;
        }
        let spans = crate::binding::spans(&src, binding);
        if !spans.is_empty() {
            out.insert(other, spans);
        }
    }
    out
}

/// The module that defines `name` at top level: this file if it does, else the
/// first module it imports that does.
///
/// Renaming from a *call site* has to reach the definition and every other
/// caller, not just the file the cursor is in.
fn defining_module(root: &Path, file: &Path, text: &str, name: &str) -> Option<PathBuf> {
    if defines(text, name) {
        return Some(canon(file));
    }
    for im in imported_modules(file, text) {
        if std::fs::read_to_string(&im).is_ok_and(|s| defines(&s, name)) {
            return Some(canon(&im));
        }
    }
    let _ = root;
    None
}

fn defines(src: &str, name: &str) -> bool {
    crate::document_symbols(src).iter().any(|s| s.name == name)
}

fn imports(file: &Path, src: &str, target: &Path) -> bool {
    imported_modules(file, src)
        .iter()
        .any(|m| canon(m) == *target)
}

fn imported_modules(file: &Path, src: &str) -> Vec<PathBuf> {
    maca_parser::parse(src)
        .module
        .items
        .iter()
        .filter_map(|item| match item {
            maca_parser::Stmt::Import(im) => import_target(im, file),
            _ => None,
        })
        .collect()
}

/// Every `.maca` file under `root`, skipping the directories a build fills.
pub fn maca_sources(root: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    walk(root, &mut out, 0);
    out.sort();
    out
}

const SKIP: &[&str] = &["target", "node_modules", ".git", "_site", "out", ".maca"];

fn walk(dir: &Path, out: &mut Vec<PathBuf>, depth: usize) {
    // A workspace is a tree, not a maze; this bound is a guard against a
    // symlink loop rather than a real limit.
    if depth > 16 {
        return;
    }
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for e in entries.flatten() {
        let p = e.path();
        let name = e.file_name();
        let name = name.to_string_lossy();
        if p.is_dir() {
            if !name.starts_with('.') && !SKIP.contains(&name.as_ref()) {
                walk(&p, out, depth + 1);
            }
        } else if p.extension().is_some_and(|x| x == "maca") {
            out.push(p);
        }
    }
}

fn canon(p: &Path) -> PathBuf {
    std::fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf())
}
