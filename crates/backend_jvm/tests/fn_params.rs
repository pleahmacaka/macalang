use std::process::Command;

fn emit(src: &str, class: &str) -> Result<String, Vec<String>> {
    let p = maca_parser::parse(src);
    assert!(p.errors.is_empty(), "parse: {:?}", p.errors);
    maca_backend_jvm::emit_checked(&p.module, class, None)
}

fn ok(src: &str, class: &str) -> String {
    match emit(src, class) {
        Ok(s) => s,
        Err(e) => panic!("unexpected refusal: {e:?}"),
    }
}

fn refused(src: &str, class: &str) -> String {
    match emit(src, class) {
        Ok(s) => panic!("expected a refusal, got:\n{s}"),
        Err(e) => e.join("\n"),
    }
}

fn have_jdk() -> bool {
    Command::new("javac")
        .arg("-version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Compile the emitted Java and run its `main`, returning stdout lines.
fn run(src: &str, class: &str) -> Option<Vec<String>> {
    if !have_jdk() {
        eprintln!("skipping: no JDK");
        return None;
    }
    let java = ok(src, class);
    let mut h = std::collections::hash_map::DefaultHasher::new();
    std::hash::Hash::hash(&src, &mut h);
    let key = std::hash::Hasher::finish(&h);
    let dir = std::env::temp_dir().join(format!("maca-jvm-fnp-{}-{key:x}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join(format!("{class}.java")), &java).unwrap();

    let c = Command::new("javac")
        .arg(format!("{class}.java"))
        .current_dir(&dir)
        .output()
        .unwrap();
    assert!(
        c.status.success(),
        "javac failed\n{}\n--- java ---\n{java}",
        String::from_utf8_lossy(&c.stderr)
    );
    let o = Command::new("java")
        .arg(class)
        .current_dir(&dir)
        .output()
        .unwrap();
    Some(
        String::from_utf8_lossy(&o.stdout)
            .lines()
            .map(|l| l.to_string())
            .collect(),
    )
}

#[test]
fn a_callback_whose_result_is_used_becomes_a_function() {
    let src = "\
twice(f, x: int) -> int => f(f(x))

main() -> int {
    info(str(twice(v => v + 1, 5)))
    0
}
";
    let java = ok(src, "A");
    assert!(
        java.contains("_Fn1 f"),
        "parameter is not an interface:\n{java}"
    );
    assert!(java.contains("f.apply("), "call site not lowered:\n{java}");
    assert!(!java.contains("Object f"), "still Object:\n{java}");
    let Some(out) = run(src, "A") else { return };
    assert_eq!(out, vec!["7"]);
}

#[test]
fn a_callback_whose_result_is_discarded_becomes_a_consumer() {
    let src = "\
register(cb) {
    cb(1)
}

main() -> int {
    register(v => info(str(v)))
    0
}
";
    let java = ok(src, "B");
    assert!(java.contains("_Act1 cb"), "not a consumer:\n{java}");
    assert!(
        java.contains("cb.accept("),
        "call site not lowered:\n{java}"
    );
    let Some(out) = run(src, "B") else { return };
    assert_eq!(out, vec!["1"]);
}

#[test]
fn arity_zero_and_two_are_both_lowered() {
    let src = "\
combine(f, a: int, b: int) -> int => f(a, b)
produce(mk) -> int => mk()
tick(go) {
    go()
}

main() -> int {
    info(str(combine((a, b) => a * b, 6, 7)))
    info(str(produce(() => 5)))
    tick(() => info(\"ticked\"))
    0
}
";
    let java = ok(src, "C");
    for want in ["_Fn2 f", "_Fn0 mk", "_Act0 go"] {
        assert!(java.contains(want), "missing {want}:\n{java}");
    }
    let Some(out) = run(src, "C") else { return };
    assert_eq!(out, vec!["42", "5", "ticked"]);
}

#[test]
fn an_interface_is_declared_once_per_shape_and_only_when_used() {
    let two = ok(
        "f(a, x: int) -> int => a(x)\ng(b, y: int) -> int => b(y)\nmain() -> int => 0\n",
        "D",
    );
    assert_eq!(
        two.matches("interface _Fn1").count(),
        1,
        "shape declared more than once:\n{two}"
    );

    let none = ok("f(x: int) -> int => x + 1\nmain() -> int => 0\n", "E");
    assert!(
        !none.contains("interface _Fn"),
        "unused interface emitted:\n{none}"
    );
}

#[test]
fn a_parameter_taking_more_arguments_than_java_can_express_is_refused() {
    let msg = refused(
        "quad(f, x: int) -> int => f(x, x, x, x)\nmain() -> int => 0\n",
        "F",
    );
    assert!(
        msg.contains('f'),
        "message does not name the parameter: {msg}"
    );
    assert!(msg.contains('4'), "message does not say the arity: {msg}");
    assert!(
        !msg.contains("Object") && !msg.contains("_Fn"),
        "refusal talks about generated Java: {msg}"
    );
}

#[test]
fn an_annotated_parameter_keeps_its_declared_type() {
    let java = ok("f(n: int) -> int => n + 1\nmain() -> int => 0\n", "G");
    assert!(java.contains("long n"), "lost the declared type:\n{java}");
}

#[test]
fn a_parameter_that_is_never_called_stays_a_plain_value() {
    let java = ok("f(x) -> int => 1\nmain() -> int => 0\n", "H");
    assert!(java.contains("Object x"), "wrongly inferred:\n{java}");
}

#[test]
fn a_lambda_handed_straight_to_a_java_api_still_works() {
    let java = ok(
        "import java \"java.util.ArrayList\"\n\nmain() -> int {\n    xs = ArrayList()\n    xs.forEach(v => info(str(v)))\n    0\n}\n",
        "I",
    );
    assert!(java.contains("import java.util.ArrayList;"), "{java}");
    assert!(java.contains("v) ->"), "lambda not emitted:\n{java}");
}
