//! A parameter's declared type is what its argument is lowered against.
//!
//! The values are asserted in Maca (`tests/programs/selective_generic.maca`);
//! this side runs it and checks the count, and separately builds a selective
//! import of every module in the tree, which is the shape the defect was found
//! in and the one nothing was exercising.

mod common;
use common::*;

use std::path::{Path, PathBuf};
use std::process::Command;

fn repo() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

#[test]
fn a_parameters_declared_type_reaches_its_argument() {
    if !have("cc") {
        eprintln!("skipping: no cc");
        return;
    }
    let program =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/programs/selective_generic.maca");
    let out = Command::new(maca())
        .args(["test", &program.to_string_lossy()])
        .output()
        .expect("spawn maca test");
    let text =
        String::from_utf8_lossy(&out.stdout).to_string() + &String::from_utf8_lossy(&out.stderr);
    assert!(out.status.success(), "{text}");
    assert!(text.contains("3 tests passed"), "{text}");
}

/// A selective import naming one function, which is the shape the defect was
/// found in.
///
/// `flame` calls `chart_rows(t, sc, 0, [])`, whose `acc` is declared `str[]`.
/// Sliced down to that one name the callee is specialized, and the empty list
/// decided its own type: the specialization took an int array and returned it
/// as the `str[]` the signature promised. Naming every export instead makes the
/// slice equal the whole module, which is why the sweep below cannot see this.
#[test]
fn a_selective_import_of_one_name_types_the_helpers_it_pulls() {
    if !have("cc") {
        eprintln!("skipping: no cc");
        return;
    }
    let root = repo();
    let entry = root.join("maca_one_name_probe.maca");
    std::fs::write(
        &entry,
        "import { flame } from profile/flame

main() -> int => 0
",
    )
    .unwrap();
    let out = std::env::temp_dir().join(format!("maca-onename-{}", std::process::id()));
    let o = Command::new(maca())
        .arg("build")
        .arg(&entry)
        .arg("-o")
        .arg(&out)
        .output()
        .expect("spawn maca build");
    let _ = std::fs::remove_file(&entry);
    let _ = std::fs::remove_file(&out);
    assert!(
        o.status.success(),
        "{}",
        String::from_utf8_lossy(&o.stderr)
            .lines()
            .take(6)
            .collect::<Vec<_>>()
            .join("\n")
    );
}

/// Every top-level function a module defines, which is what a selective import
/// of it can name.
///
/// All of them go into one import rather than one import each. There are 607
/// across `modules/`, and a build apiece is minutes; naming them together still
/// pulls the union of their closures, which is what the defect needed. What it
/// gives up is isolation: this says a module has a name that does not slice, not
/// which one.
fn exports(path: &Path) -> Vec<String> {
    let Ok(src) = std::fs::read_to_string(path) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for l in src.lines() {
        let Some(name) = l.split('(').next() else {
            continue;
        };
        let ok = !name.is_empty()
            && name
                .chars()
                .all(|c| c.is_ascii_lowercase() || c == '_' || c.is_ascii_digit())
            && l.starts_with(name)
            && l.contains('(');
        if ok && !out.contains(&name.to_string()) {
            out.push(name.to_string());
        }
    }
    out
}

#[test]
fn every_module_builds_both_selectively_and_whole() {
    // A module that only ever gets imported whole can carry a defect that only
    // the sliced form shows, and the other way round: `cli/help` used `Cmd`
    // without importing `cli/spec`, so neither form worked, and nothing noticed
    // because everything that wanted it also imported the spec.
    if !have("cc") {
        eprintln!("skipping: no cc");
        return;
    }
    let root = repo();
    let scratch = std::env::temp_dir().join(format!("maca-selimp-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&scratch);
    std::fs::create_dir_all(&scratch).unwrap();

    let mut checked = 0;
    let mut failed: Vec<String> = Vec::new();
    let mut pkgs: Vec<PathBuf> = std::fs::read_dir(root.join("modules"))
        .unwrap()
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.is_dir())
        .collect();
    pkgs.sort();
    for pkg in pkgs {
        let name = pkg.file_name().unwrap().to_string_lossy().to_string();
        let mut files: Vec<PathBuf> = std::fs::read_dir(&pkg)
            .unwrap()
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| p.extension().is_some_and(|x| x == "maca"))
            .collect();
        files.sort();
        for f in files {
            let stem = f.file_stem().unwrap().to_string_lossy().to_string();
            let module = format!("{name}/{stem}");
            let names = exports(&f);
            if names.is_empty() {
                continue;
            }
            let named = names.join(", ");
            for src in [
                format!("import {{ {named} }} from {module}\n\nmain() -> int => 0\n"),
                format!("import {module}\n\nmain() -> int => 0\n"),
            ] {
                let entry = root.join("maca_selective_import_probe.maca");
                std::fs::write(&entry, &src).unwrap();
                let o = Command::new(maca())
                    .arg("build")
                    .arg(&entry)
                    .arg("-o")
                    .arg(scratch.join("probe"))
                    .output()
                    .expect("spawn maca build");
                let _ = std::fs::remove_file(&entry);
                checked += 1;
                if !o.status.success() {
                    failed.push(format!(
                        "{}\n{}",
                        src.lines().next().unwrap(),
                        String::from_utf8_lossy(&o.stderr)
                            .lines()
                            .take(3)
                            .collect::<Vec<_>>()
                            .join("\n")
                    ));
                }
            }
        }
    }
    let _ = std::fs::remove_dir_all(&scratch);
    assert!(checked > 40, "only {checked} imports were tried");
    assert!(failed.is_empty(), "{}", failed.join("\n\n"));
}
