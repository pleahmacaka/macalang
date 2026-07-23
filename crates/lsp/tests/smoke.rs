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
    assert!(bad.iter().any(|d| d.contains("TypeMismatch")), "diag: {bad:?}");
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
    assert!(maca_lsp::is_config_source("import nixpkgs\nsystem.stateVersion = \"24.11\"\n"));
    assert!(maca_lsp::is_config_source("dev.name = \"x\"\n"));
    assert!(!maca_lsp::is_config_source("main() -> int { 0 }\n"));
}

#[test]
fn program_completions_offers_user_functions() {
    let src = "helper() -> int => 1\nhelium() -> int => 2\nmain() -> int => 0\n";
    let got = maca_lsp::program_completions(src, "hel");
    assert!(got.contains(&"helper".to_string()) && got.contains(&"helium".to_string()), "{got:?}");
    assert!(!got.contains(&"main".to_string()), "prefix filter failed: {got:?}");
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
        assert!(src.is_char_boundary(off), "col {col} -> non-boundary byte {off}");
    }
}
