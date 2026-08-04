use std::io::Write;
use std::process::Command;

/// Emit `src`, then evaluate `calls` against it in Node and return the output.
fn run(src: &str, calls: &[&str]) -> Vec<String> {
    let p = maca_parser::parse(src);
    assert!(p.errors.is_empty(), "parse: {:?}", p.errors);
    let js = maca_backend_js::emit(&p.module).js;

    let mut h = std::collections::hash_map::DefaultHasher::new();
    std::hash::Hash::hash(&(src, calls), &mut h);
    let key = std::hash::Hasher::finish(&h);
    let dir = std::env::temp_dir().join(format!("maca-js-match-{}-{key:x}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let module = dir.join("app.js");
    std::fs::File::create(&module)
        .unwrap()
        .write_all(js.as_bytes())
        .unwrap();

    let driver = dir.join("run.js");
    let mut d = String::from("const m = require(\"./app.js\");\nwith (m) {\n");
    for c in calls {
        d.push_str(&format!("  console.log(String({c}));\n"));
    }
    d.push_str("}\n");
    std::fs::File::create(&driver)
        .unwrap()
        .write_all(d.as_bytes())
        .unwrap();

    let out = Command::new("node")
        .arg(&driver)
        .output()
        .expect("node is required for the JS backend tests");
    let stdout = String::from_utf8_lossy(&out.stdout).to_string();
    assert!(
        out.status.success(),
        "node failed\n--- stderr ---\n{}\n--- js ---\n{js}",
        String::from_utf8_lossy(&out.stderr)
    );
    stdout.lines().map(|l| l.to_string()).collect()
}

const COLOR: &str = "\
Color = Red | Green | Blue

name(c: Color) -> str {
    match c {
        Red => \"red\"
        Green => \"green\"
        Blue => \"blue\"
    }
}

mk_green() -> Color => Green
mk_blue() -> Color => Blue
main() -> int => 0
";

#[test]
fn a_nullary_variant_selects_its_own_arm() {
    let out = run(COLOR, &["name(mk_green())", "name(mk_blue())", "name(Red)"]);
    assert_eq!(out, vec!["green", "blue", "red"]);
}

#[test]
fn a_nullary_variant_is_a_value_not_a_bare_identifier() {
    let out = run(COLOR, &["mk_green().$"]);
    assert_eq!(out, vec!["Green"]);
}

const SHAPE: &str = "\
Shape = Circle(float) | Rect(float, float)

area(s: Shape) -> float {
    match s {
        Circle(r) => 3.0 * r * r
        Rect(w, h) => w * h
    }
}

main() -> int => 0
";

#[test]
fn a_constructor_pattern_binds_its_payload() {
    let out = run(SHAPE, &["area(Rect(3.0, 4.0))", "area(Circle(2.0))"]);
    assert_eq!(out, vec!["12", "12"]);
}

#[test]
fn a_constructor_call_carries_its_arguments() {
    let out = run(SHAPE, &["Rect(3.0, 4.0)._1", "Circle(2.0).$"]);
    assert_eq!(out, vec!["4", "Circle"]);
}

const LISTS: &str = "\
head_or(xs: int[], d: int) -> int {
    match xs {
        [] => d
        x, ..rest => x
    }
}

second_or(xs: int[], d: int) -> int {
    match xs {
        [a, b] => b
        _ => d
    }
}

rest_head(xs: int[], d: int) -> int {
    match xs {
        x, ..rest => head_or(rest, d)
        _ => d
    }
}

main() -> int => 0
";

#[test]
fn a_list_pattern_tests_length_and_binds_elements() {
    let out = run(
        LISTS,
        &[
            "head_or([], 9)",
            "head_or([7, 8], 9)",
            "second_or([1, 2], 9)",
            "second_or([1, 2, 3], 9)",
            "rest_head([1, 2, 3], 9)",
        ],
    );
    assert_eq!(out, vec!["9", "7", "2", "9", "2"]);
}

const RECORDS: &str = "\
Task = { title: str, done: bool }

title_of(t: Task) -> str {
    match t {
        { title } => title
    }
}

first_title(ts: Task[]) -> str {
    match ts {
        t, ..rest => t.title
        _ => \"empty\"
    }
}

main() -> int => 0
";

#[test]
fn a_record_pattern_binds_its_fields() {
    let out = run(
        RECORDS,
        &[
            "title_of({ title: \"write\", done: false })",
            "first_title([])",
            "first_title([{ title: \"ship\", done: false }])",
        ],
    );
    assert_eq!(out, vec!["write", "empty", "ship"]);
}

const GUARD: &str = "\
Shape = Circle(float) | Rect(float, float)

kind(s: Shape) -> str {
    match s {
        Circle(r) if r > 10.0 => \"big circle\"
        Circle(r) => \"circle\"
        Rect(w, h) => \"rect\"
    }
}

main() -> int => 0
";

#[test]
fn a_guard_reads_the_arms_own_bindings() {
    let out = run(
        GUARD,
        &[
            "kind(Circle(20.0))",
            "kind(Circle(1.0))",
            "kind(Rect(1.0, 2.0))",
        ],
    );
    assert_eq!(out, vec!["big circle", "circle", "rect"]);
}

#[test]
fn an_uncovered_scrutinee_throws_rather_than_answering_undefined() {
    let src = "\
pick(n: int) -> str {
    match n {
        1 => \"one\"
    }
}

main() -> int => 0
";
    let out = run(
        src,
        &["(() => { try { return pick(2); } catch (e) { return \"threw\"; } })()"],
    );
    assert_eq!(out, vec!["threw"]);
}

#[test]
fn use_strict_stays_the_first_statement() {
    let p = maca_parser::parse(COLOR);
    let js = maca_backend_js::emit(&p.module).js;
    assert!(
        js.trim_start().starts_with("\"use strict\""),
        "directive is no longer first:\n{}",
        &js[..js.len().min(200)]
    );
}
