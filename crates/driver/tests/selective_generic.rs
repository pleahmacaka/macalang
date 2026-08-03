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

/// A record type reached through a selective import is concrete, not generic.
///
/// `imports::fresh` renames an inlined module's items to `<module>__<name>`,
/// and a module stem is lowercase, so `Tone` declared in `pal.maca` reaches the
/// back end as `pal__Tone` and `is_type_var_name` read the leading `p` and
/// called it a type variable. Every helper taking a list of it was then
/// monomorphized instead of emitted, and `cc` was handed a call to a function
/// that does not exist and an undeclared `pal__ToneArr`.
///
/// Two files, because one file is the case that already worked: nothing is
/// renamed until a module is inlined. And the entry names only `sheet`, whose
/// own signature is `-> str`, because naming the helper or the type puts the
/// record back in the slice's own signature and the defect disappears.
#[test]
fn a_record_type_from_an_inlined_module_is_not_a_type_variable() {
    if !have("cc") {
        eprintln!("skipping: no cc");
        return;
    }
    let root = repo();
    let dir = root.join("maca_rec_probe_src");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("pal.maca"),
        "Tone = { cls: str, decl: str }

tones() -> Tone[] =>
    { cls = \"a\", decl = \"1\" },
    { cls = \"b\", decl = \"2\" }

render(ts: Tone[]) -> str =>
    ts.map(t => t.cls ++ t.decl).join(\"\")

sheet() -> str => render(tones())
",
    )
    .unwrap();
    let entry = root.join("maca_rec_probe.maca");
    std::fs::write(
        &entry,
        "import { sheet } from maca_rec_probe_src/pal

main() -> int {
  info(sheet())
  0
}
",
    )
    .unwrap();

    let out = std::env::temp_dir().join(format!("maca-recprobe-{}", std::process::id()));
    let o = Command::new(maca())
        .arg("run")
        .arg(&entry)
        .arg("-o")
        .arg(&out)
        .output()
        .expect("spawn maca run");
    let _ = std::fs::remove_file(&entry);
    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::remove_file(&out);

    let text = String::from_utf8_lossy(&o.stdout).to_string() + &String::from_utf8_lossy(&o.stderr);
    assert!(o.status.success(), "{text}");
    // The value, not just the exit status: a specialization that compiled but
    // took the wrong element type would still have run.
    assert!(text.contains("a1b2"), "{text}");
}

/// A lambda inside an inlined module captures the module's own parameter, even
/// when the entry defines a function spelling the name the same way.
///
/// `emit_closure` decided what to capture by asking `is_known_global`, which
/// answers for the *whole* inlined program. `render(at: str, …)` therefore
/// stopped capturing `at` the moment anything else in the slice defined an
/// `at`, and the lambda body read that function as a value instead: `++` was
/// handed a `maca_closure2` and the native backend refused the program. Nothing
/// is renamed until a module is inlined and a one-file program rarely has two
/// `at`s, which is why it took a slice this wide to show. `apps/tomo/highlight`
/// is where it was found, against `at` in `site_home.maca`.
///
/// Both lowering paths are here, because a specialization builds its own `env`
/// and would not have been fixed by the same line otherwise: `render` takes the
/// record list concretely, `render_any` takes it as `e[]` and is monomorphized.
/// The value is asserted, not the exit status: the record element type was
/// right in the emitted C either way, and the capture is the only thing the
/// printed string can disagree about.
#[test]
fn a_lambda_in_an_inlined_module_captures_its_own_parameter() {
    if !have("cc") {
        eprintln!("skipping: no cc");
        return;
    }
    let root = repo();
    let dir = root.join("maca_cap_probe_src");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("pal.maca"),
        "Tone = { cls: str, decl: str }

tones() -> Tone[] =>
    { cls = \"a\", decl = \"1\" },
    { cls = \"b\", decl = \"2\" }

render(at: str, ts: Tone[]) -> str =>
    ts.map(t => at ++ t.cls ++ t.decl).join(\"\")

render_any(at: str, ts: e[]) -> str =>
    ts.map(t => at ++ t.cls ++ t.decl).join(\"\")

sheet() -> str =>
    render(\".x \", tones()) ++ render_any(\"|y \", tones())
",
    )
    .unwrap();
    let entry = root.join("maca_cap_probe.maca");
    std::fs::write(
        &entry,
        "import { sheet } from maca_cap_probe_src/pal

at(cs: str[], i: int) -> str => cs.get(i)

main() -> int {
  info(sheet())
  info(at([\"z\"], 0))
  0
}
",
    )
    .unwrap();

    let out = std::env::temp_dir().join(format!("maca-capprobe-{}", std::process::id()));
    let o = Command::new(maca())
        .arg("run")
        .arg(&entry)
        .arg("-o")
        .arg(&out)
        .output()
        .expect("spawn maca run");
    let _ = std::fs::remove_file(&entry);
    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::remove_file(&out);

    let text = String::from_utf8_lossy(&o.stdout).to_string() + &String::from_utf8_lossy(&o.stderr);
    assert!(o.status.success(), "{text}");
    assert!(text.contains(".x a1.x b2|y a1|y b2"), "{text}");
    assert!(
        text.contains("z"),
        "the shadowed function still works: {text}"
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
