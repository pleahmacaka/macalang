mod common;
use common::*;

use std::process::Command;

fn run(name: &str, src: &str) -> (bool, String) {
    let dir = std::env::temp_dir().join("maca-method-sets");
    let _ = std::fs::create_dir_all(&dir);
    let f = dir.join(format!("{name}.maca"));
    std::fs::write(&f, src).unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_maca"))
        .args(["run", &f.to_string_lossy()])
        .output()
        .expect("spawn maca run");
    (
        out.status.success(),
        String::from_utf8_lossy(&out.stdout).to_string() + &String::from_utf8_lossy(&out.stderr),
    )
}

/// One call per documented `str` method, in one program.
#[test]
fn every_str_method_the_checker_allows_actually_works() {
    if have_wsl() || !have("cc") {
        eprintln!("skipping: needs a host cc and no wsl");
        return;
    }
    let calls = [
        "s.length()",
        "s.split(\",\").length()",
        "s.trim().length()",
        "s.upper().length()",
        "s.lower().length()",
        "s.contains(\"a\") ? 1 : 0",
        "s.starts_with(\"a\") ? 1 : 0",
        "s.ends_with(\"a\") ? 1 : 0",
        "s.replace(\"a\", \"b\").length()",
        "s.substr(0, 2).length()",
        "s.slice(1, 3).length()",
        "s.index_of(\"b\")",
        "s.repeat(2).length()",
        "s.pad_start(9, \" \").length()",
        "s.pad_end(9, \" \").length()",
        "s.pad_center(9, \" \").length()",
        "s.chars().length()",
        "s.at(0).length()",
        "s.at(0).is_whitespace() ? 1 : 0",
        "s.at(0).is_ascii_digit() ? 1 : 0",
        "s.at(0).is_alpha() ? 1 : 0",
    ];
    assert_eq!(
        calls.len(),
        maca_core::STR_METHODS.len(),
        "STR_METHODS changed but this test didn't"
    );
    let body: String = calls.iter().map(|c| format!("    n = n + {c}\n")).collect();
    let src = format!(
        "main() -> int {{\n    s = \"abcdef\"\n    n = 0\n{body}    info(\"{{n}}\")\n    0\n}}\n"
    );
    let (ok, out) = run("str_methods", &src);
    assert!(ok, "a documented str method doesn't work:\n{out}");
}

#[test]
fn every_list_method_the_checker_allows_actually_works() {
    if have_wsl() || !have("cc") {
        eprintln!("skipping: needs a host cc and no wsl");
        return;
    }
    let int_calls = [
        "xs.map(v => v * 2).length()",
        "xs.filter(v => v > 1).length()",
        "xs.reduce(0, (a, b) => a + b)",
        "xs.fold(0, (a, b) => a + b)",
        "xs.sort().length()",
        "xs.reverse().length()",
        "xs.push(9).length()",
        "xs.pop().length()",
        "xs.slice(0, 2).length()",
        "xs.contains(1) ? 1 : 0",
        "xs.index_of(2)",
        "xs.sum()",
        "xs.min()",
        "xs.max()",
        "xs.first()",
        "xs.last()",
        "xs.get(0)",
        "xs.length()",
        "xs.parallel(v => v * 2).length()",
        "xs.set(1, 9).length()",
        "xs.insert(0, 9).length()",
        "xs.remove(0).length()",
        "xs.index_of_by(v => v > 1)",
        "xs.enumerate().length()",
        "xs.sort_by(v => v * -1).length()",
    ];
    let str_calls = ["ss.join(\"-\").length()"];
    assert_eq!(
        int_calls.len() + str_calls.len(),
        maca_core::LIST_METHODS.len(),
        "LIST_METHODS changed but this test didn't"
    );
    let body: String = int_calls
        .iter()
        .chain(str_calls.iter())
        .map(|c| format!("    n = n + {c}\n"))
        .collect();
    let src = format!(
        "main() -> int {{\n    xs = [1, 2, 3]\n    ss = [\"a\", \"b\"]\n    n = 0\n{body}    info(\"{{n}}\")\n    0\n}}\n"
    );
    let (ok, out) = run("list_methods", &src);
    assert!(ok, "a documented list method doesn't work:\n{out}");
}

/// The same executed check for `Map str V`.
#[test]
fn every_map_method_the_checker_allows_actually_works() {
    if have_wsl() || !have("cc") {
        eprintln!("skipping: needs a host cc and no wsl");
        return;
    }
    let calls = [
        "m.set(\"c\", 3).length()",
        "m.get(\"a\", 0)",
        "m.has(\"a\") ? 1 : 0",
        "m.remove(\"a\").length()",
        "m.keys().length()",
        "m.length()",
    ];
    assert_eq!(
        calls.len(),
        maca_core::MAP_METHODS.len(),
        "MAP_METHODS changed but this test didn't"
    );
    let body: String = calls.iter().map(|c| format!("    n = n + {c}\n")).collect();
    let src = format!(
        "main() -> int {{\n    m: Map str int = map()\n    m = m.set(\"a\", 1).set(\"b\", 2)\n    n = 0\n{body}    info(\"{{n}}\")\n    0\n}}\n"
    );
    let (ok, out) = run("map_methods", &src);
    assert!(ok, "a documented map method doesn't work:\n{out}");
}

/// The point of the whole exercise.
#[test]
fn a_misspelt_method_is_a_diagnostic_with_a_suggestion() {
    if have_wsl() || !have("cc") {
        eprintln!("skipping: needs a host cc and no wsl");
        return;
    }
    for (src, want) in [
        (
            "main() -> int {\n    info(\"{\"ab\".lenght()}\")\n    0\n}\n",
            "did you mean `length`",
        ),
        (
            "main() -> int {\n    info(\"{[1, 2].mapp(v => v)}\")\n    0\n}\n",
            "did you mean `map`",
        ),
        (
            "main() -> int {\n    m: Map str int = map()\n    info(\"{m.lenght()}\")\n    0\n}\n",
            "did you mean `length`",
        ),
    ] {
        let (ok, out) = run("misspelt", src);
        assert!(!ok, "a misspelt method should not compile:\n{out}");
        assert!(out.contains(want), "expected {want:?}, got:\n{out}");
        assert!(
            !out.contains("undefined reference"),
            "it reached the linker:\n{out}"
        );
    }
}

/// Gradual typing still has to work.
#[test]
fn gradual_and_user_defined_methods_are_not_flagged() {
    if have_wsl() || !have("cc") {
        eprintln!("skipping: needs a host cc and no wsl");
        return;
    }
    let (ok, out) = run(
        "user_method",
        "shout(s: str) -> str => s.upper() ++ \"!\"\n\n\
         main() -> int {\n    info(\"{\"hi\".shout()}\")\n    0\n}\n",
    );
    assert!(ok, "a user-defined UFCS method was rejected:\n{out}");
    assert!(out.contains("HI!"), "wrong output:\n{out}");
}
