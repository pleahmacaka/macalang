use crate::binding::{Binding, Scope};
use maca_parser::modules::import_target;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// Every file the rename has to touch, with the spans to replace in each.
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

    let owner = defining_module(file, text, &binding.name).unwrap_or_else(|| canon(file));
    let mut seen: Vec<PathBuf> = vec![canon(file)];
    for other in maca_sources(root) {
        let real = canon(&other);
        if seen.contains(&real) {
            continue;
        }
        let Ok(src) = std::fs::read_to_string(&other) else {
            continue;
        };
        if real != owner && !can_see(&other, &src, &owner, &binding.name) {
            continue;
        }
        if real != owner && defines(&src, &binding.name) {
            continue;
        }
        seen.push(real);
        let spans = crate::binding::spans(&src, binding);
        if !spans.is_empty() {
            out.insert(other, spans);
        }
    }
    out
}

/// The module that defines `name` at top level.
fn defining_module(file: &Path, text: &str, name: &str) -> Option<PathBuf> {
    if defines(text, name) {
        return Some(canon(file));
    }
    let mut queue: Vec<PathBuf> = imported_modules(file, text);
    let mut seen: Vec<PathBuf> = vec![canon(file)];
    while let Some(m) = queue.pop() {
        let real = canon(&m);
        if seen.contains(&real) {
            continue;
        }
        seen.push(real.clone());
        let Ok(src) = std::fs::read_to_string(&m) else {
            continue;
        };
        if defines(&src, name) {
            return Some(real);
        }
        queue.extend(imported_modules(&m, &src));
    }
    None
}

fn defines(src: &str, name: &str) -> bool {
    crate::document_symbols(src).iter().any(|s| s.name == name)
}

/// Can this module see `name` as defined by `target`?
fn can_see(file: &Path, src: &str, target: &Path, name: &str) -> bool {
    let mut queue: Vec<(PathBuf, bool)> = imports_of(file, src, name);
    let mut seen: Vec<PathBuf> = vec![canon(file)];
    while let Some((m, selective)) = queue.pop() {
        if selective {
            continue;
        }
        let real = canon(&m);
        if seen.contains(&real) {
            continue;
        }
        seen.push(real.clone());
        if real == *target {
            return true;
        }
        if let Ok(s) = std::fs::read_to_string(&m) {
            queue.extend(imports_of(&m, &s, name));
        }
    }
    false
}

/// Each module this file imports, paired with "this import excludes `name`".
fn imports_of(file: &Path, src: &str, name: &str) -> Vec<(PathBuf, bool)> {
    maca_parser::parse(src)
        .module
        .items
        .iter()
        .filter_map(|item| match item {
            maca_parser::Stmt::Import(im) => {
                let excludes = match im {
                    maca_parser::Import::Names { names, .. } => !names.iter().any(|n| n == name),
                    _ => false,
                };
                import_target(im, file).map(|p| (p, excludes))
            }
            _ => None,
        })
        .collect()
}

fn imported_modules(file: &Path, src: &str) -> Vec<PathBuf> {
    imports_of(file, src, "")
        .into_iter()
        .map(|(p, _)| p)
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
