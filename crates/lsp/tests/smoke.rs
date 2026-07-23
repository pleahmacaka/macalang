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
