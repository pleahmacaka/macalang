mod common;
use common::*;

use std::process::Command;

/// Run a Maca suite under `maca test` and insist every assertion in it held.
fn suite(name: &str) {
    if have_wsl() || !have("cc") {
        eprintln!("skipping: needs a native cc and no wsl");
        return;
    }
    let out = Command::new(maca())
        .args(["test", &program(name).to_string_lossy()])
        .output()
        .expect("spawn maca test");
    assert!(
        out.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr),
    );
}

/// A source that must not compile, and the words the diagnostic owes the reader.
fn rejected(src: &str, expect: &str) {
    let dir = std::env::temp_dir().join("maca-return-nesting");
    std::fs::create_dir_all(&dir).expect("scratch dir");
    let path = dir.join(format!("r{:x}.maca", hash(src)));
    std::fs::write(&path, src).expect("write source");
    let out = Command::new(maca())
        .args([
            "build",
            &path.to_string_lossy(),
            "-o",
            &path.with_extension("bin").to_string_lossy(),
        ])
        .output()
        .expect("spawn maca build");
    let said = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        !out.status.success(),
        "this compiled, and should not:\n{src}"
    );
    assert!(
        said.contains(expect),
        "expected {expect:?} in the diagnostic for:\n{src}\ngot:\n{said}"
    );
}

fn hash(s: &str) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    s.hash(&mut h);
    h.finish()
}

#[test]
fn return_leaves_early_and_the_tail_is_still_the_value() {
    suite("early_return");
}

#[test]
fn a_nested_definition_reads_and_writes_the_scope_that_holds_it() {
    suite("nested_fns");
}

/// `return` is a statement. Every place it cannot be lowered is named before any backend sees it.
#[test]
fn a_return_that_is_not_a_statement_is_refused_by_name() {
    for src in [
        "pick(b: bool) -> int => b ? return 1 : 2\n\nmain() -> int => pick(true)\n",
        "main() -> int {\n    f = v => return v\n    f(1)\n}\n",
        "main() -> int => return 1\n",
        "main() -> int {\n    n = 1 + (return 2)\n    n\n}\n",
    ] {
        rejected(src, "this `return` stands inside an expression");
    }
}

/// What `return` carries has to match what the function said it gives back.
#[test]
fn return_is_checked_against_the_declared_result() {
    rejected(
        "tally() {\n    return 5\n}\n\nmain() -> int {\n    tally()\n    0\n}\n",
        "declares no result",
    );
    rejected(
        "tally() -> int {\n    return\n}\n\nmain() -> int => tally()\n",
        "a bare `return` leaves without one",
    );
    rejected(
        "tally() -> int {\n    return \"no\"\n}\n\nmain() -> int => tally()\n",
        "expected int, found str",
    );
}

/// A nested definition is a value bound where it is written, so it is in scope from there and no earlier.
#[test]
fn a_nested_definition_is_in_scope_from_where_it_is_written() {
    rejected(
        "main() -> int {\n    go() -> int {\n        return go()\n    }\n\n    go()\n}\n",
        "it cannot name itself",
    );
    rejected(
        "main() -> int {\n    first() -> int => second()\n\n    second() -> int => 1\n\n    first()\n}\n",
        "is defined further down this block",
    );
}

/// The JS backend computes what the native one computes, for the shape the feature was asked for.
#[test]
fn the_js_backend_agrees_with_the_native_one() {
    if !have("node") {
        eprintln!("skipping: needs node");
        return;
    }
    let dir = std::env::temp_dir().join("maca-return-js");
    std::fs::create_dir_all(&dir).expect("scratch dir");
    let src = dir.join("board.maca");
    std::fs::write(
        &src,
        "board() -> str {\n\
         \x20   held = 0\n\
         \x20   moves = 0\n\n\
         \x20   grab(section: int) {\n\
         \x20       if section < 0 {\n\
         \x20           return\n\
         \x20       }\n\n\
         \x20       held = section\n\
         \x20       moves = moves + 1\n\
         \x20   }\n\n\
         \x20   grab(-1)\n\
         \x20   grab(4)\n\n\
         \x20   \"{held}/{moves}\"\n\
         }\n\n\
         main() -> Element => div(p(board()))\n",
    )
    .expect("write source");

    let out_dir = dir.join("out");
    let build = Command::new(maca())
        .args([
            "build",
            "--target",
            "js",
            &src.to_string_lossy(),
            "-o",
            &out_dir.to_string_lossy(),
        ])
        .output()
        .expect("spawn maca build --target js");
    assert!(
        build.status.success(),
        "js build failed:\n{}",
        String::from_utf8_lossy(&build.stderr)
    );

    let probe = out_dir.join("probe.js");
    std::fs::write(
        &probe,
        "console.log(require(\"./app.js\").board());\n".to_string(),
    )
    .expect("write probe");

    let ran = Command::new("node").arg(&probe).output().expect("run node");
    let said = String::from_utf8_lossy(&ran.stdout);
    assert_eq!(
        said.trim(),
        "4/1",
        "the JS backend disagreed with the native one:\n{}",
        String::from_utf8_lossy(&ran.stderr)
    );
}

/// The backends that cannot hold a nested definition say so by name, rather than dropping it and computing something plausible.
#[test]
fn a_backend_that_cannot_lower_a_nested_definition_names_it() {
    let dir = std::env::temp_dir().join("maca-nested-targets");
    std::fs::create_dir_all(&dir).expect("scratch dir");
    let src = dir.join("nested.maca");
    std::fs::write(
        &src,
        "main() -> int {\n\
         \x20   total = 0\n\n\
         \x20   add(n: int) {\n\
         \x20       total = total + n\n\
         \x20   }\n\n\
         \x20   add(42)\n\n\
         \x20   total\n\
         }\n",
    )
    .expect("write source");

    for target in ["rust", "jvm"] {
        let out = Command::new(maca())
            .args([
                "build",
                "--target",
                target,
                &src.to_string_lossy(),
                "-o",
                &dir.join(target).to_string_lossy(),
            ])
            .output()
            .expect("spawn maca build");
        let said = format!(
            "{}{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        );
        assert!(
            !out.status.success(),
            "the {target} backend accepted a nested definition:\n{said}"
        );
        assert!(
            said.contains("`add` is defined inside another function"),
            "the {target} backend did not name what it refused:\n{said}"
        );
    }
}
