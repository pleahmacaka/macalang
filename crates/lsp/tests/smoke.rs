use maca_lsp::{config_completions, diagnostics, hover};

#[test]
fn hover_returns_signature() {
    let src = "add(s: Store, title: str) -> Store {\n    s\n}\n";
    // offset within `add`
    let h = hover(src, 1).expect("hover");
    assert!(h.contains("add(") && h.contains("-> Store"), "hover: {h}");
}

#[test]
fn config_option_completion() {
    let c = config_completions("net");
    assert!(c.contains(&"networking".to_string()), "completions: {c:?}");
    let s = config_completions("s");
    assert!(s.contains(&"system".to_string()) && s.contains(&"services".to_string()));
}

#[test]
fn diagnostics_surface() {
    let bad = diagnostics("bad() -> int => \"x\"\n", false);
    assert!(
        bad.iter().any(|d| d.contains("TypeMismatch")),
        "diag: {bad:?}"
    );
    let good = diagnostics("main() -> int {\n    0\n}\n", false);
    assert!(good.is_empty(), "good: {good:?}");
}

#[test]
fn position_to_offset_maps_line_col() {
    let src = "abc\ndef\nghi";
    assert_eq!(maca_lsp::position_to_offset(src, 0, 0), 0);
    assert_eq!(maca_lsp::position_to_offset(src, 1, 0), 4); // after "abc\n"
    assert_eq!(maca_lsp::position_to_offset(src, 2, 2), 10); // 'i'
    // clamps past end of a line to the line's content length
    assert_eq!(maca_lsp::position_to_offset(src, 0, 99), 3);
}

#[test]
fn is_config_source_detects_nix_mode() {
    assert!(maca_lsp::is_config_source(
        "import nixpkgs\nsystem.stateVersion = \"24.11\"\n"
    ));
    assert!(maca_lsp::is_config_source("dev.name = \"x\"\n"));
    assert!(!maca_lsp::is_config_source("main() -> int { 0 }\n"));
}

#[test]
fn program_completions_offers_user_functions() {
    let src = "helper() -> int => 1\nhelium() -> int => 2\nmain() -> int => 0\n";
    let got = maca_lsp::program_completions(src, "hel");
    assert!(
        got.contains(&"helper".to_string()) && got.contains(&"helium".to_string()),
        "{got:?}"
    );
    assert!(
        !got.contains(&"main".to_string()),
        "prefix filter failed: {got:?}"
    );
}

#[test]
fn prefix_at_reads_dotted_identifier() {
    let src = "system.pa";
    assert_eq!(maca_lsp::prefix_at(src, src.len()), "system.pa");
}

#[test]
fn prefix_at_never_panics_on_multibyte_offsets() {
    // Every byte offset (incl. mid-char) must be safe — the running LSP feeds
    // arbitrary completion offsets here. Regression for the mid-char slice panic.
    let src = "한글 = 1\n// 주석 comment\nmsg = \"안녕 world\"\n";
    for off in 0..=src.len() {
        let _ = maca_lsp::prefix_at(src, off); // must not panic
    }
    // off past the end is clamped, not a crash
    let _ = maca_lsp::prefix_at(src, src.len() + 50);
}

#[test]
fn position_to_offset_maps_utf16_columns() {
    // 한글 is two 3-byte chars (one UTF-16 unit each); `=` is column 3 → byte 7.
    let src = "한글 = 1\n";
    assert_eq!(maca_lsp::position_to_offset(src, 0, 0), 0); // before 한
    assert_eq!(maca_lsp::position_to_offset(src, 0, 2), 6); // before the space
    assert_eq!(maca_lsp::position_to_offset(src, 0, 3), 7); // at '=' — a char boundary
    // the returned offset is always a valid char boundary (never mid-char)
    for col in 0..10 {
        let off = maca_lsp::position_to_offset(src, 0, col);
        assert!(
            src.is_char_boundary(off),
            "col {col} -> non-boundary byte {off}"
        );
    }
}

#[test]
fn references_finds_definition_and_uses() {
    // `twice` is defined once and used twice — three spans, and the mention
    // inside a comment and a string literal must NOT be counted.
    let src = "twice(n: int) -> int => n * 2\n\
               // twice is a helper\n\
               main() -> int {\n\
                   info(\"twice\")\n\
                   twice(1) + twice(2)\n\
               }\n";
    let off = src.find("twice").unwrap();
    let refs = maca_lsp::references(src, off);
    assert_eq!(refs.len(), 3, "expected def + 2 uses, got {refs:?}");
    for (s, e) in &refs {
        assert_eq!(&src[*s..*e], "twice");
    }
    // the first span is the definition
    assert_eq!(refs[0].0, off);
}

#[test]
fn references_skip_comments_and_strings() {
    let src = "// name here\nname() -> int => 1\nmain() -> int { info(\"name\") name() }\n";
    let off = src.find("name() -> int").unwrap();
    let refs = maca_lsp::references(src, off);
    // only the definition and the real call site
    assert_eq!(refs.len(), 2, "comment/string occurrences leaked: {refs:?}");
}

#[test]
fn references_on_empty_position_is_empty() {
    let src = "main() -> int => 0\n";
    // an offset sitting on whitespace yields no word, hence no references
    let off = src.find(' ').unwrap();
    assert!(maca_lsp::references(src, off).is_empty());
}

#[test]
fn signature_help_reports_active_parameter() {
    let src = "add(a: int, b: int) -> int => a + b\nmain() -> int => add(1, 2)\n";
    // cursor just after `add(` → first parameter
    let off = src.rfind("add(").unwrap() + 4;
    let (sig, params, active) = maca_lsp::signature_help(src, off).expect("signature help");
    assert_eq!(sig, "add(a: int, b: int) -> int");
    assert_eq!(params, vec!["a: int".to_string(), "b: int".to_string()]);
    assert_eq!(active, 0);

    // cursor after the comma → second parameter
    let off2 = src.rfind(", 2").unwrap() + 2;
    let (_, _, active2) = maca_lsp::signature_help(src, off2).expect("signature help");
    assert_eq!(active2, 1);
}

#[test]
fn signature_help_outside_a_call_is_none() {
    let src = "add(a: int, b: int) -> int => a + b\nmain() -> int => 0\n";
    let off = src.rfind("0").unwrap();
    assert!(maca_lsp::signature_help(src, off).is_none());
}

#[test]
fn signature_help_handles_nested_calls() {
    let src = "inner(x: int) -> int => x\nouter(a: int, b: int) -> int => a + b\nmain() -> int => outer(inner(1), 2)\n";
    // inside `inner(` — the innermost call wins
    let off = src.rfind("inner(").unwrap() + 6;
    let (sig, _, _) = maca_lsp::signature_help(src, off).expect("signature help");
    assert_eq!(sig, "inner(x: int) -> int");
    // after the nested call closes, we're back on `outer`'s second parameter
    let off2 = src.rfind("), 2").unwrap() + 3;
    let (sig2, _, active2) = maca_lsp::signature_help(src, off2).expect("signature help");
    assert_eq!(sig2, "outer(a: int, b: int) -> int");
    assert_eq!(active2, 1);
}
