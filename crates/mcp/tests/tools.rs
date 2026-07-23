use maca_mcp::{check, fmt, options, spec, stdlib};

#[test]
fn check_flags_bad_passes_good() {
    // bad: return type is int, body is a string → a diagnostic
    let bad = check("bad() -> int => \"nope\"\n", false);
    assert!(bad.iter().any(|d| d.contains("type-mismatch")), "expected type-mismatch: {bad:?}");

    // good: clean program → no diagnostics
    let good = check("main() -> int {\n    info(\"hi\")\n    0\n}\n", false);
    assert!(good.is_empty(), "good program should have no diagnostics: {good:?}");

    // parse error surfaces too
    let broken = check("main( ) -> int { let = }\n", false);
    assert!(!broken.is_empty(), "broken source should report something");
}

#[test]
fn check_config_effect() {
    let d = check("system.motd = info(\"x\")\n", true);
    assert!(d.iter().any(|x| x.contains("effect-in-config")), "expected effect-in-config: {d:?}");
}

#[test]
fn fmt_roundtrips() {
    let src = "main() -> int {\n    info(\"hi\")\n    0\n}\n";
    let out = fmt(src).expect("fmt");
    // formatting a good program re-parses cleanly
    assert!(check(&out, false).is_empty(), "formatted output should still check: {out}");
}

#[test]
fn reference_tools() {
    assert!(stdlib("json").iter().any(|s| s.contains("json.")));
    assert!(options("net").contains(&"networking".to_string()));
    assert!(spec("syntax").contains("ctor"));
}
