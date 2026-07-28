//! The checker's method lists must match what the back end can actually lower.
//!
//! `maca-core` rejects a method a `str` or `T[]` doesn't have, so a typo is a
//! diagnostic instead of `undefined reference to 'slice'` from the linker. That
//! check is only as good as its lists, and a hand-maintained list rots: the
//! first version of it was missing `parallel` and `join` and invented `count`,
//! `get` and `fixed` on `str`.
//!
//! So the lists are not trusted — they are executed. Every name in them is
//! compiled and run against a real receiver here. A name the back end can't
//! lower fails to build; a name the back end *can* lower but that is missing
//! from the list gets rejected by the checker as a typo, which the second half
//! of this file catches.

use std::process::Command;

fn have(cmd: &str) -> bool {
    Command::new(cmd)
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}
fn wsl() -> bool {
    Command::new("wsl")
        .arg("true")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

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

/// One call per documented `str` method, in one program. It has to compile and
/// run, which means the checker accepted every name and the back end lowered it.
#[test]
fn every_str_method_the_checker_allows_actually_works() {
    if wsl() || !have("cc") {
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
    // one is exercised per documented method
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
    if wsl() || !have("cc") {
        eprintln!("skipping: needs a host cc and no wsl");
        return;
    }
    // `join` needs a str[] and the rest an int[], so they run in one program
    // over two receivers.
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

/// The point of the whole exercise: a misspelt method is a diagnostic naming
/// the closest real one, not a linker error about a C symbol.
#[test]
fn a_misspelt_method_is_a_diagnostic_with_a_suggestion() {
    if wsl() || !have("cc") {
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
        // `str` has substr, not slice — the case that sent people to the linker
        (
            "main() -> int {\n    info(\"{\"abc\".slice(0, 2)}\")\n    0\n}\n",
            "`str` has no method `slice`",
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

/// Gradual typing still has to work: a method on an `any` receiver, or one the
/// program defines itself, must not be flagged.
#[test]
fn gradual_and_user_defined_methods_are_not_flagged() {
    if wsl() || !have("cc") {
        eprintln!("skipping: needs a host cc and no wsl");
        return;
    }
    // a user function used UFCS-style on a str receiver
    let (ok, out) = run(
        "user_method",
        "shout(s: str) -> str => s.upper() ++ \"!\"\n\n\
         main() -> int {\n    info(\"{\"hi\".shout()}\")\n    0\n}\n",
    );
    assert!(ok, "a user-defined UFCS method was rejected:\n{out}");
    assert!(out.contains("HI!"), "wrong output:\n{out}");
}
