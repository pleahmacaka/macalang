use maca_lexer::{Tok, dump_tokens, lex};
use std::fs;
use std::path::PathBuf;

/// Token kinds for a clean source (asserts no lex errors).
fn toks(src: &str) -> Vec<Tok> {
    let l = lex(src);
    assert!(l.errors.is_empty(), "unexpected lex errors: {:?}", l.errors);
    l.tokens.into_iter().map(|t| t.tok).collect()
}

fn examples_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples")
}

fn golden_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/golden")
}

fn norm(s: &str) -> String {
    s.replace("\r\n", "\n")
}

/// Golden snapshot: lex an example, compare its dump to a committed file.
/// Run with `UPDATE_GOLDEN=1 cargo test -p maca-lexer` to (re)generate.
fn check_golden(name: &str) {
    let src = fs::read_to_string(examples_dir().join(name)).unwrap();
    let l = lex(&src);
    assert!(
        l.errors.is_empty(),
        "{name} lexed with errors: {:?}",
        l.errors
    );
    let dump = dump_tokens(&l.tokens);
    let stem = name.trim_end_matches(".maca");
    let path = golden_dir().join(format!("{stem}.tokens"));
    if std::env::var("UPDATE_GOLDEN").is_ok() {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, &dump).unwrap();
    } else {
        let want = fs::read_to_string(&path)
            .unwrap_or_else(|_| panic!("missing golden {path:?}; run with UPDATE_GOLDEN=1"));
        assert_eq!(norm(&dump), norm(&want), "token dump mismatch for {name}");
    }
}

#[test]
fn golden_hello() {
    check_golden("hello.maca");
}

#[test]
fn golden_taskr() {
    check_golden("taskr.maca");
}

#[test]
fn golden_system() {
    check_golden("system.maca");
}

// ---- targeted rules ------------------------------------------------------

#[test]
fn path_literal_vs_divide() {
    // operand position -> path literal
    assert_eq!(
        toks("x = /tmp"),
        vec![
            Tok::Ident("x".into()),
            Tok::Eq,
            Tok::Path("/tmp".into()),
            Tok::Eof,
        ]
    );
    // between operands -> divide/join operator
    assert_eq!(
        toks("a / b"),
        vec![
            Tok::Ident("a".into()),
            Tok::Slash,
            Tok::Ident("b".into()),
            Tok::Eof,
        ]
    );
    // relative and home paths
    assert_eq!(
        toks("import ./x.maca"),
        vec![Tok::Import, Tok::Path("./x.maca".into()), Tok::Eof,]
    );
    assert!(matches!(toks("y = ~/cfg")[2], Tok::Path(ref p) if p == "~/cfg"));
}

#[test]
fn attached_vs_spaced_question() {
    assert_eq!(
        toks("x?"),
        vec![Tok::Ident("x".into()), Tok::QuestionPost, Tok::Eof]
    );
    assert_eq!(
        toks("c ? x : y"),
        vec![
            Tok::Ident("c".into()),
            Tok::Question,
            Tok::Ident("x".into()),
            Tok::Colon,
            Tok::Ident("y".into()),
            Tok::Eof,
        ]
    );
}

#[test]
fn string_interpolation() {
    assert_eq!(
        toks("\"{box} #{id}\""),
        vec![
            Tok::StrOpen,
            Tok::InterpStart,
            Tok::Ident("box".into()),
            Tok::InterpEnd,
            Tok::StrText(" #".into()),
            Tok::InterpStart,
            Tok::Ident("id".into()),
            Tok::InterpEnd,
            Tok::StrClose,
            Tok::Eof,
        ]
    );
}

#[test]
fn literal_braces() {
    assert_eq!(
        toks("\"a {{b}} c\""),
        vec![
            Tok::StrOpen,
            Tok::StrText("a {b} c".into()),
            Tok::StrClose,
            Tok::Eof,
        ]
    );
}

#[test]
fn trailing_comma_continues() {
    // A trailing comma joins the next line: no Newline between the elements.
    // (no Newline between a and b; the trailing newline at EOF is dropped)
    let t = toks("xs = a,\n     b\n");
    assert_eq!(
        t,
        vec![
            Tok::Ident("xs".into()),
            Tok::Eq,
            Tok::Ident("a".into()),
            Tok::Comma,
            Tok::Ident("b".into()),
            Tok::Eof,
        ]
    );
}

#[test]
fn multiline_ternary_joins() {
    // Ternary `?`/`:` at line start continue the previous expression.
    let src = "v =\n    c\n        ? x\n        : y\n";
    let t = toks(src);
    assert_eq!(
        t,
        vec![
            Tok::Ident("v".into()),
            Tok::Eq,
            Tok::Ident("c".into()),
            Tok::Question,
            Tok::Ident("x".into()),
            Tok::Colon,
            Tok::Ident("y".into()),
            Tok::Eof,
        ]
    );
}

#[test]
fn hyphen_identifiers() {
    assert_eq!(
        toks("nixpkgs.noto-fonts"),
        vec![
            Tok::Ident("nixpkgs".into()),
            Tok::Dot,
            Tok::Ident("noto-fonts".into()),
            Tok::Eof,
        ]
    );
    // subtraction still lexes as an operator when spaced
    assert_eq!(
        toks("a - b"),
        vec![
            Tok::Ident("a".into()),
            Tok::Minus,
            Tok::Ident("b".into()),
            Tok::Eof,
        ]
    );
}

#[test]
fn unterminated_string_reports_error() {
    let l = lex("\"oops");
    assert!(!l.errors.is_empty());
}

#[test]
fn raw_triple_quoted_string_is_verbatim() {
    // a raw string keeps braces and quotes literally, no interpolation
    let lexed = maca_lexer::lex("x = \"\"\"a { b } \"c\" {d}\"\"\"\n");
    let texts: Vec<String> = lexed
        .tokens
        .iter()
        .filter_map(|t| {
            if let maca_lexer::Tok::StrText(s) = &t.tok {
                Some(s.clone())
            } else {
                None
            }
        })
        .collect();
    assert_eq!(
        texts,
        vec!["a { b } \"c\" {d}".to_string()],
        "raw text not verbatim: {texts:?}"
    );
    // no interpolation tokens were produced
    assert!(
        !lexed
            .tokens
            .iter()
            .any(|t| matches!(t.tok, maca_lexer::Tok::InterpStart)),
        "raw string interpolated"
    );
}

// ---- literal braces in strings --------------------------------------------
//
// `{` starts an interpolation, so a literal brace must be escaped. Getting that
// wrong used to be silent: `"{"` opened an interpolation, the following `"`
// opened a *nested* string, and that string swallowed source up to the next
// quote — the program compiled with the wrong text baked in. (It hid a real bug
// in `tools/lint.maca`, whose single-line-`if` rule tested `contains("{")` and
// therefore never matched anything.) A `"…"` string now stops at end of line.

fn errs(src: &str) -> Vec<String> {
    maca_lexer::lex(src)
        .errors
        .into_iter()
        .map(|e| e.msg)
        .collect()
}

fn text(src: &str) -> Vec<String> {
    maca_lexer::lex(src)
        .tokens
        .into_iter()
        .filter_map(|t| match t.tok {
            maca_lexer::Tok::StrText(s) => Some(s),
            _ => None,
        })
        .collect()
}

#[test]
fn a_bare_brace_in_a_string_is_an_error_not_a_silent_swallow() {
    let e = errs("main() -> int {\n    ob = \"{\"\n    print(ob)\n    0\n}\n");
    assert!(
        e.iter().any(|m| m.contains("spans a line")),
        "expected a spans-a-line diagnostic, got {e:?}"
    );
    // and it says how to fix it
    assert!(
        e.iter().any(|m| m.contains("\\{") && m.contains("{{")),
        "the diagnostic should name both brace escapes, got {e:?}"
    );
}

#[test]
fn both_brace_escapes_produce_a_literal_brace() {
    assert!(errs(r#"x = "\{a\}""#).is_empty());
    assert_eq!(text(r#"x = "\{a\}""#), vec!["{a}"]);
    assert!(errs(r#"x = "{{a}}""#).is_empty());
    assert_eq!(text(r#"x = "{{a}}""#), vec!["{a}"]);
}

#[test]
fn interpolation_and_raw_strings_still_work() {
    // an unescaped brace is still an interpolation
    let toks = maca_lexer::lex(r#"x = "n={n}""#);
    assert!(toks.errors.is_empty(), "{:?}", toks.errors);
    assert!(
        toks.tokens
            .iter()
            .any(|t| t.tok == maca_lexer::Tok::InterpStart)
    );
    // a raw string is exempt — it may span lines and hold bare braces
    assert!(errs("x = \"\"\"a {\n} b\"\"\"").is_empty());
}

// ---- interpolation format specs --------------------------------------------
//
// `"{x:>8}"` ends in a format spec; `"{c ? a : b}"` ends in a ternary. Both use
// a colon inside an interpolation, so the lexer separates them by attachment —
// a spec's colon has no space before it, a ternary's does. That is the same
// rule that already distinguishes `x?` from `c ? x : y`.

fn specs(src: &str) -> Vec<String> {
    maca_lexer::lex(src)
        .tokens
        .into_iter()
        .filter_map(|t| match t.tok {
            maca_lexer::Tok::FmtSpec(s) => Some(s),
            _ => None,
        })
        .collect()
}

#[test]
fn a_format_spec_is_lexed_as_one_token() {
    assert_eq!(specs(r#""{pi:.2}""#), vec![".2"]);
    assert_eq!(specs(r#""{x:>8}""#), vec![">8"]);
    assert_eq!(specs(r#""{x:<8}""#), vec!["<8"]);
    assert_eq!(specs(r#""{x:^8}""#), vec!["^8"]);
    assert_eq!(specs(r#""{n:08}""#), vec!["08"]);
    assert_eq!(specs(r#""{pi:>10.3}""#), vec![">10.3"]);
    // the expression before it is lexed normally
    assert_eq!(specs(r#""{a[0]:^5} {r.f:>2}""#), vec!["^5", ">2"]);
}

#[test]
fn a_spaced_colon_is_still_a_ternary() {
    let toks = maca_lexer::lex(r#""{c ? a : b}""#);
    assert!(toks.errors.is_empty(), "{:?}", toks.errors);
    assert!(
        specs(r#""{c ? a : b}""#).is_empty(),
        "ternary read as a spec"
    );
    assert!(toks.tokens.iter().any(|t| t.tok == maca_lexer::Tok::Colon));
    // and a ternary whose arms are strings, as the handbook writes it
    assert!(specs(r#""{n > 0 ? "yes" : "no"}""#).is_empty());
}

#[test]
fn a_colon_outside_an_interpolation_is_not_a_spec() {
    // type annotations are the common case
    assert!(specs("f(x: int) -> int => x").is_empty());
    assert!(specs("Point = {\n    x: int\n}").is_empty());
}

#[test]
fn a_leading_boolean_operator_continues_the_line() {
    // A long condition breaks either way: the operator can end the line or
    // begin the next one. `&&` used to be accepted only at the end, while
    // `||` was accepted at both — so the same condition parsed or didn't
    // depending on which operator it happened to use.
    for (src, op) in [
        ("ok = a\n    && b\n", Tok::AmpAmp),
        ("ok = a\n    || b\n", Tok::BarBar),
    ] {
        assert_eq!(
            toks(src),
            vec![
                Tok::Ident("ok".into()),
                Tok::Eq,
                Tok::Ident("a".into()),
                op,
                Tok::Ident("b".into()),
                Tok::Eof,
            ],
            "{src:?} should be one expression"
        );
    }
}
