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
    assert!(l.errors.is_empty(), "{name} lexed with errors: {:?}", l.errors);
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
    assert_eq!(toks("x = /tmp"), vec![
        Tok::Ident("x".into()),
        Tok::Eq,
        Tok::Path("/tmp".into()),
        Tok::Eof,
    ]);
    // between operands -> divide/join operator
    assert_eq!(toks("a / b"), vec![
        Tok::Ident("a".into()),
        Tok::Slash,
        Tok::Ident("b".into()),
        Tok::Eof,
    ]);
    // relative and home paths
    assert_eq!(toks("import ./x.maca"), vec![
        Tok::Import,
        Tok::Path("./x.maca".into()),
        Tok::Eof,
    ]);
    assert!(matches!(toks("y = ~/cfg")[2], Tok::Path(ref p) if p == "~/cfg"));
}

#[test]
fn attached_vs_spaced_question() {
    assert_eq!(toks("x?"), vec![Tok::Ident("x".into()), Tok::QuestionPost, Tok::Eof]);
    assert_eq!(toks("c ? x : y"), vec![
        Tok::Ident("c".into()),
        Tok::Question,
        Tok::Ident("x".into()),
        Tok::Colon,
        Tok::Ident("y".into()),
        Tok::Eof,
    ]);
}

#[test]
fn string_interpolation() {
    assert_eq!(toks("\"{box} #{id}\""), vec![
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
    ]);
}

#[test]
fn literal_braces() {
    assert_eq!(toks("\"a {{b}} c\""), vec![
        Tok::StrOpen,
        Tok::StrText("a {b} c".into()),
        Tok::StrClose,
        Tok::Eof,
    ]);
}

#[test]
fn trailing_comma_continues() {
    // A trailing comma joins the next line: no Newline between the elements.
    // (no Newline between a and b; the trailing newline at EOF is dropped)
    let t = toks("xs = a,\n     b\n");
    assert_eq!(t, vec![
        Tok::Ident("xs".into()),
        Tok::Eq,
        Tok::Ident("a".into()),
        Tok::Comma,
        Tok::Ident("b".into()),
        Tok::Eof,
    ]);
}

#[test]
fn multiline_ternary_joins() {
    // Ternary `?`/`:` at line start continue the previous expression.
    let src = "v =\n    c\n        ? x\n        : y\n";
    let t = toks(src);
    assert_eq!(t, vec![
        Tok::Ident("v".into()),
        Tok::Eq,
        Tok::Ident("c".into()),
        Tok::Question,
        Tok::Ident("x".into()),
        Tok::Colon,
        Tok::Ident("y".into()),
        Tok::Eof,
    ]);
}

#[test]
fn hyphen_identifiers() {
    assert_eq!(toks("nixpkgs.noto-fonts"), vec![
        Tok::Ident("nixpkgs".into()),
        Tok::Dot,
        Tok::Ident("noto-fonts".into()),
        Tok::Eof,
    ]);
    // subtraction still lexes as an operator when spaced
    assert_eq!(toks("a - b"), vec![
        Tok::Ident("a".into()),
        Tok::Minus,
        Tok::Ident("b".into()),
        Tok::Eof,
    ]);
}

#[test]
fn unterminated_string_reports_error() {
    let l = lex("\"oops");
    assert!(!l.errors.is_empty());
}
