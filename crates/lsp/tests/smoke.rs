use maca_lsp::{config_completions, diagnostics, hover};

#[test]
fn hover_returns_signature() {
    let src = "add(s: Store, title: str) -> Store {\n    s\n}\n";
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
    assert_eq!(maca_lsp::position_to_offset(src, 1, 0), 4);
    assert_eq!(maca_lsp::position_to_offset(src, 2, 2), 10);
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
    let src = "한글 = 1\n// 주석 comment\nmsg = \"안녕 world\"\n";
    for off in 0..=src.len() {
        let _ = maca_lsp::prefix_at(src, off);
    }
    let _ = maca_lsp::prefix_at(src, src.len() + 50);
}

#[test]
fn position_to_offset_maps_utf16_columns() {
    let src = "한글 = 1\n";
    assert_eq!(maca_lsp::position_to_offset(src, 0, 0), 0);
    assert_eq!(maca_lsp::position_to_offset(src, 0, 2), 6);
    assert_eq!(maca_lsp::position_to_offset(src, 0, 3), 7);
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
    assert_eq!(refs[0].0, off);
}

#[test]
fn references_skip_comments_and_strings() {
    let src = "// name here\nname() -> int => 1\nmain() -> int { info(\"name\") name() }\n";
    let off = src.find("name() -> int").unwrap();
    let refs = maca_lsp::references(src, off);
    assert_eq!(refs.len(), 2, "comment/string occurrences leaked: {refs:?}");
}

/// The program that showed what the old whole-word search did.
const SHADOWED: &str = "P = { x: int }\n\
                        \n\
                        f() -> int {\n\
                        \x20   x = 1\n\
                        \x20   x + 1\n\
                        }\n\
                        \n\
                        g(x: int) -> int => x * 2\n\
                        \n\
                        h() -> int {\n\
                        \x20   p = P { x = 9 }\n\
                        \x20   p.x\n\
                        }\n";

fn refs_at(src: &str, needle: &str) -> Vec<(usize, usize)> {
    maca_lsp::references(src, src.find(needle).expect("needle"))
}

#[test]
fn a_local_is_scoped_to_its_function() {
    let refs = refs_at(SHADOWED, "x = 1");
    assert_eq!(refs.len(), 2, "expected only `f`'s two uses: {refs:?}");
    let g = SHADOWED.find("g(x: int)").unwrap();
    assert!(refs.iter().all(|(s, _)| *s < g), "escaped `f`: {refs:?}");
}

#[test]
fn a_parameter_is_scoped_to_its_function() {
    let refs = refs_at(SHADOWED, "x: int) -> int => x");
    assert_eq!(
        refs.len(),
        2,
        "expected `g`'s parameter and its use: {refs:?}"
    );
}

/// A field is a different thing from a variable that happens to share its name.
#[test]
fn a_field_is_not_a_variable() {
    let decl = refs_at(SHADOWED, "x: int }");
    assert_eq!(decl.len(), 3, "declaration, key, access: {decl:?}");
    assert_eq!(decl, refs_at(SHADOWED, "x = 9"), "from the literal's key");
    let f_body = SHADOWED.find("x = 1").unwrap();
    assert!(
        decl.iter().all(|(s, _)| *s != f_body),
        "a field rename reached a local: {decl:?}"
    );
}

/// A `{` is a record or a block depending on the line, which is why the parser has a `no_brace` mode.
#[test]
fn a_function_body_is_not_a_record() {
    let src = "Point = { x: int }\n\
               at(n: int) -> Point => Point { x = n }\n\
               go() -> int {\n\
               \x20   count = 1\n\
               \x20   count + 1\n\
               }\n";
    let refs = refs_at(src, "count = 1");
    assert_eq!(refs.len(), 2, "`count` is a local, not a field: {refs:?}");
}

#[test]
fn references_on_empty_position_is_empty() {
    let src = "main() -> int => 0\n";
    let off = src.find(' ').unwrap();
    assert!(maca_lsp::references(src, off).is_empty());
}

#[test]
fn signature_help_reports_active_parameter() {
    let src = "add(a: int, b: int) -> int => a + b\nmain() -> int => add(1, 2)\n";
    let off = src.rfind("add(").unwrap() + 4;
    let (sig, params, active) = maca_lsp::signature_help(src, off).expect("signature help");
    assert_eq!(sig, "add(a: int, b: int) -> int");
    assert_eq!(params, vec!["a: int".to_string(), "b: int".to_string()]);
    assert_eq!(active, 0);

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
    let off = src.rfind("inner(").unwrap() + 6;
    let (sig, _, _) = maca_lsp::signature_help(src, off).expect("signature help");
    assert_eq!(sig, "inner(x: int) -> int");
    let off2 = src.rfind("), 2").unwrap() + 3;
    let (sig2, _, active2) = maca_lsp::signature_help(src, off2).expect("signature help");
    assert_eq!(sig2, "outer(a: int, b: int) -> int");
    assert_eq!(active2, 1);
}

fn spans_of(src: &str, at: &str) -> Vec<(usize, usize)> {
    let off = src.find(at).expect("the cursor anchor");
    let b = maca_lsp::binding::resolve(src, off).expect("a binding");
    maca_lsp::binding::spans(src, &b)
}

fn scope_of(src: &str, at: &str) -> maca_lsp::Scope {
    let off = src.find(at).expect("the cursor anchor");
    maca_lsp::binding::resolve(src, off)
        .expect("a binding")
        .scope
}

/// A constant, a record and a sum type are definitions, not assignments.
#[test]
fn a_definition_renames_from_its_own_declaration() {
    let konst = "NTASKS = 6 as const\n\nmain() -> int {\n    n = NTASKS\n    n + NTASKS\n}\n";
    assert_eq!(scope_of(konst, "NTASKS"), maca_lsp::Scope::TopLevel);
    assert_eq!(spans_of(konst, "NTASKS").len(), 3, "declaration + two uses");

    let rec = "Expr = {\n    children: Expr[]\n}\n\nmk() -> Expr =>\n    Expr { children = [] }\n";
    assert_eq!(scope_of(rec, "Expr"), maca_lsp::Scope::TopLevel);
    assert_eq!(
        spans_of(rec, "Expr").len(),
        4,
        "decl, field, return, literal"
    );

    let sum = "Color = Red | Green\n\npick() -> Color => Red\n";
    assert_eq!(spans_of(sum, "Color").len(), 2);
}

/// `c with { port = p }` and `=> Point { x = n }` are record literals whose line also carries a `->`.
#[test]
fn a_field_rename_reaches_literals_on_an_arrow_line() {
    let src = "Config = {\n    port: int\n}\n\nset(c: Config, p: int) -> Config => c with { port = p }\n\nat(n: int) -> Config => Config { port = n }\n";
    assert_eq!(scope_of(src, "port"), maca_lsp::Scope::Field);
    assert_eq!(
        spans_of(src, "port").len(),
        3,
        "declaration, the `with` update, and the literal"
    );
}

/// A block opened inside a still-open call, `xs.map(v => { y = 1 … })`, used to keep the file's paren counter above zero.
#[test]
fn a_local_in_a_lambda_block_is_not_a_field() {
    let src = "Row = {\n    y: int\n}\n\nmain() -> int {\n    zs = [1].map(v => {\n        y = v + 1\n        y * 2\n    })\n    zs.length()\n}\n";
    assert!(matches!(
        scope_of(src, "y = v + 1"),
        maca_lsp::Scope::Local(_)
    ));
    assert_eq!(
        spans_of(src, "y = v + 1").len(),
        2,
        "just the two in the block"
    );
}

/// A function passed by name is an argument, not a parameter.
#[test]
fn a_function_passed_by_name_is_still_top_level() {
    let src = "quote(s: str) -> str => s\n\nmain() -> str => [\"a\"].map(quote).join(\",\")\n";
    assert_eq!(scope_of(src, "quote)"), maca_lsp::Scope::TopLevel);
    assert_eq!(spans_of(src, "quote)").len(), 2, "definition and use");
}

/// The other direction: a bracketless list pattern's head *is* a binder, and its `prev` is the brace rather than a comma.
#[test]
fn a_bracketless_list_pattern_binds_its_head() {
    let src = "head_of(xs: int[]) -> int {\n    match xs {\n        [] => 0\n        first, ..rest => first + rest.length()\n    }\n}\n";
    assert!(matches!(
        scope_of(src, "first, ..rest"),
        maca_lsp::Scope::Local(_)
    ));
    assert_eq!(spans_of(src, "first, ..rest").len(), 2);
}

/// A top-level name is not visible inside a function that binds the same name itself.
#[test]
fn a_top_level_rename_skips_a_function_that_shadows_it() {
    let src = "helper(n: int) -> int => n * 2\n\nuses() -> int => helper(1)\n\nshadows() -> int {\n    helper = 5\n    helper + helper\n}\n";
    assert_eq!(
        spans_of(src, "helper(n").len(),
        2,
        "the definition and the one real call"
    );
}

/// `import std/text` names a directory.
#[test]
fn an_import_path_is_not_a_binding() {
    let src = "import std/text\n\nmain() -> int {\n    text = \"hello\"\n    text.length()\n}\n";
    assert_eq!(spans_of(src, "text = ").len(), 2, "the local only");

    let sel = "import { lines } from std/text\n\nmain() -> int => lines(\"a\").length()\n";
    assert_eq!(
        spans_of(sel, "lines").len(),
        2,
        "the imported name and its use"
    );
}

/// `int` and `info` are identifiers to the lexer.
#[test]
fn primitives_and_builtins_are_not_renameable() {
    let src = "add(a: int, b: int) -> int => a + b\n";
    let off = src.find("int").unwrap();
    assert!(maca_lsp::binding::resolve(src, off).is_none());
    assert!(!maca_lsp::binding::is_renameable_to("int"));
    assert!(!maca_lsp::binding::is_renameable_to("if"));
    assert!(!maca_lsp::binding::is_renameable_to("1x"));
    assert!(!maca_lsp::binding::is_renameable_to(""));
    assert!(maca_lsp::binding::is_renameable_to("twice"));
}

/// An argument that happens to start a line is not a new top-level item.
#[test]
fn a_wrapped_argument_does_not_end_the_item() {
    let src = "pick(a: int, b: int) -> int => a + b\n\nmain() -> int {\n    n = pick(\n1,\n2)\n    n + n\n}\n";
    assert_eq!(
        spans_of(src, "n = pick").len(),
        3,
        "the binding and both uses"
    );
}

/// A function whose body binds its own name, `f() { f = 1 f }`, still has a head that is the definition.
#[test]
fn an_item_head_is_the_definition_even_when_its_body_shadows_it() {
    let src = "f(n: int) -> int {\n    f = n + 1\n    f\n}\n\nmain() -> int => f(1)\n";
    assert_eq!(scope_of(src, "f(n"), maca_lsp::Scope::TopLevel);
    let call = src.rfind("f(1)").unwrap();
    assert_eq!(
        spans_of(src, "f(n"),
        vec![(0, 1), (call, call + 1)],
        "the head and the call, not the local"
    );
    assert!(matches!(scope_of(src, "f = n"), maca_lsp::Scope::Local(_)));
}

/// `mk(n: int) -> Point => { x = n, y = n }` is a record literal with no type name in front of it, and the token before the brace cannot say so.
#[test]
fn a_field_rename_reaches_an_anonymous_literal_after_an_arrow() {
    let src = "Point = {\n    x: int,\n    y: int\n}\n\nmk(n: int) -> Point => { x = n, y = n }\n";
    assert_eq!(scope_of(src, "x: int"), maca_lsp::Scope::Field);
    assert_eq!(
        spans_of(src, "x: int").len(),
        2,
        "the declaration and the literal's key"
    );
}

/// The block form of the same shape stays a block.
#[test]
fn a_block_after_an_arrow_is_still_a_block() {
    let src = "Row = {\n    acc: int\n}\n\nf() -> int => {\n    acc = 1\n    acc + 1\n}\n";
    assert!(matches!(
        scope_of(src, "acc = 1"),
        maca_lsp::Scope::Local(_)
    ));
    assert_eq!(
        spans_of(src, "acc = 1").len(),
        2,
        "just the two in the block"
    );
}

/// A misspelt method's diagnostic has to point at the typo.
#[test]
fn a_method_typo_is_marked_on_the_method() {
    let src = "count(s: str) -> int {\n    s.lenght()\n}\n";
    let d = maca_lsp::diagnostics_located(src, false);
    assert_eq!(
        d.len(),
        1,
        "{:?}",
        d.iter().map(|d| &d.message).collect::<Vec<_>>()
    );
    assert_eq!(&src[d[0].start..d[0].end], "lenght", "anchored elsewhere");
}
