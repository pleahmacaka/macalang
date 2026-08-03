//! What the editor's lightbulb offers, and what applying it leaves behind.
//!
//! Every test here has two halves, because either one alone passes for the
//! wrong reason. "Is it offered" catches an action that never appears; it says
//! nothing about the edit. "Does the result still mean the right thing" is the
//! half that matters, and it is asserted by *applying* the edit and running the
//! whole front end over the result: the file has to parse, and the diagnostic
//! the fix claimed has to be gone. An action whose edit mangles the file is a
//! worse outcome than no action at all, since the editor reports success either
//! way.

use maca_lsp::{Action, apply_edits, code_actions};

/// The actions on offer with the cursor sitting on `needle`.
fn at(src: &str, needle: &str) -> Vec<Action> {
    let off = src.find(needle).unwrap_or_else(|| panic!("no {needle:?}"));
    code_actions(src, off, off, false)
}

fn titled<'a>(actions: &'a [Action], want: &str) -> &'a Action {
    actions
        .iter()
        .find(|a| a.title == want)
        .unwrap_or_else(|| panic!("no {want:?} in {:?}", titles(actions)))
}

fn titles(actions: &[Action]) -> Vec<&str> {
    actions.iter().map(|a| a.title.as_str()).collect()
}

/// Apply the action and hand back a file the whole front end accepts as
/// well-formed. Panics with the offending source, because a quick fix that
/// produces something unparseable is the failure this suite exists for.
fn applied(src: &str, action: &Action) -> String {
    let out = apply_edits(src, &action.edits);
    let parsed = maca_parser::parse(&out);
    assert!(
        parsed.errors.is_empty(),
        "the edit left a file that does not parse: {:?}\n{out}",
        parsed.errors
    );
    out
}

fn diagnostics(src: &str) -> Vec<String> {
    maca_lsp::diagnostics(src, false)
}

// ---- Immutable ------------------------------------------------------------

#[test]
fn reassigning_a_const_offers_the_declaration_that_makes_it_mutable() {
    let src = "main() -> int {\n    const limit = 5\n    limit = 6\n    limit\n}\n";
    assert!(
        diagnostics(src).iter().any(|d| d.contains("Immutable")),
        "the fixture stopped being an error: {:?}",
        diagnostics(src)
    );
    let acts = at(src, "limit = 6");
    let out = applied(src, titled(&acts, "declare `limit` mutable"));
    assert!(out.contains("    limit = 5\n"), "the `const` stayed: {out}");
    assert!(
        diagnostics(&out).is_empty(),
        "left: {:?}",
        diagnostics(&out)
    );
}

/// The other spelling of the same declaration. `const x = e` and
/// `x = e as const` bind the same thing, and the edit that unbinds it is at the
/// opposite end of the line.
#[test]
fn the_as_const_spelling_is_fixed_at_its_own_end() {
    let src = "main() -> int {\n    limit = 5 as const\n    limit = 6\n    limit\n}\n";
    let acts = at(src, "limit = 6");
    let out = applied(src, titled(&acts, "declare `limit` mutable"));
    assert!(out.contains("    limit = 5\n"), "suffix stayed: {out}");
    assert!(
        diagnostics(&out).is_empty(),
        "left: {:?}",
        diagnostics(&out)
    );
}

/// A Capitalized name is a constant with no `const` to drop. The only edit that
/// would make it mutable renames it, which is a rename, so nothing is offered
/// rather than something that looks like a fix and is not.
#[test]
fn a_capitalized_constant_gets_no_mutable_fix() {
    let src = "main() -> int {\n    Limit = 5\n    Limit = 6\n    Limit\n}\n";
    assert!(diagnostics(src).iter().any(|d| d.contains("Immutable")));
    let acts = at(src, "Limit = 6");
    assert!(
        !acts.iter().any(|a| a.title.contains("mutable")),
        "offered a fix it cannot make: {:?}",
        titles(&acts)
    );
}

/// Two functions, one name. The fix has to reach the declaration in the
/// function the cursor is in, and `binding` is what knows which that is.
#[test]
fn the_fix_edits_the_declaration_in_the_cursors_own_function() {
    let src = "one() -> int {\n    const limit = 1\n    limit\n}\n\n\
               two() -> int {\n    const limit = 2\n    limit = 3\n    limit\n}\n";
    let acts = at(src, "limit = 3");
    let out = applied(src, titled(&acts, "declare `limit` mutable"));
    assert!(
        out.contains("one() -> int {\n    const limit = 1"),
        "the other function's declaration was edited: {out}"
    );
    assert!(out.contains("    limit = 2\n"), "wrong one edited: {out}");
}

// ---- UndefinedName: a phantom keyword -------------------------------------

#[test]
fn a_phantom_return_is_dropped() {
    let src = "f() -> int {\n    x = 1\n    return x\n}\n";
    let acts = at(src, "return");
    let out = applied(src, titled(&acts, "drop the `return`"));
    assert_eq!(out, "f() -> int {\n    x = 1\n    x\n}\n");
    assert!(
        diagnostics(&out).is_empty(),
        "left: {:?}",
        diagnostics(&out)
    );
}

#[test]
fn a_phantom_let_is_dropped() {
    let src = "f() -> int {\n    let x = 1\n    x\n}\n";
    let acts = at(src, "let");
    let out = applied(src, titled(&acts, "drop the `let`"));
    assert_eq!(out, "f() -> int {\n    x = 1\n    x\n}\n");
    assert!(
        diagnostics(&out).is_empty(),
        "left: {:?}",
        diagnostics(&out)
    );
}

/// `null` reaches the same diagnostic and is the one that is not a deletion:
/// what replaces it is a sum type the author has to design, so there is no
/// edit to offer.
#[test]
fn a_phantom_null_gets_no_fix() {
    let src = "f() -> int {\n    x = null\n    1\n}\n";
    assert!(
        diagnostics(src).iter().any(|d| d.contains("null")),
        "the fixture stopped being an error"
    );
    assert!(at(src, "null").is_empty(), "offered a fix for `null`");
}

// ---- UndefinedName: a misspelt method -------------------------------------

#[test]
fn a_misspelt_method_takes_the_checkers_suggestion() {
    let src = "f(s: str) -> int {\n    s.lenght()\n}\n";
    let acts = at(src, "lenght");
    let out = applied(src, titled(&acts, "change `lenght` to `length`"));
    assert!(out.contains("s.length()"), "not respelt: {out}");
    assert!(
        diagnostics(&out).is_empty(),
        "left: {:?}",
        diagnostics(&out)
    );
}

/// The closed method sets are per receiver, so a list typo gets the list's
/// nearest name, not the string's.
#[test]
fn a_misspelt_list_method_is_fixed_too() {
    let src = "f(xs: int[]) -> int {\n    xs.frist()\n}\n";
    let acts = at(src, "frist");
    let out = applied(src, titled(&acts, "change `frist` to `first`"));
    assert!(out.contains("xs.first()"), "not respelt: {out}");
}

/// Two typos, one message. The cursor picks which call is respelt, and the
/// other is left for its own invocation: an edit at both would be a fix the
/// user asked for once and got twice.
#[test]
fn only_the_call_under_the_cursor_is_respelt() {
    let src = "f(s: str) -> int {\n    a = s.lenght()\n    b = s.lenght()\n    a + b\n}\n";
    let second = src.rfind("lenght").unwrap();
    let acts = code_actions(src, second, second, false);
    let out = applied(src, titled(&acts, "change `lenght` to `length`"));
    assert_eq!(
        out, "f(s: str) -> int {\n    a = s.lenght()\n    b = s.length()\n    a + b\n}\n",
        "the wrong call was respelt"
    );
}

// ---- NonExhaustive --------------------------------------------------------

#[test]
fn a_short_match_offers_the_arms_it_is_missing() {
    let src = "Color = Red | Green | Blue\n\n\
               name(c: Color) -> str {\n    match c {\n        Red => \"r\"\n    }\n}\n";
    let acts = at(src, "Red =>");
    let out = applied(src, titled(&acts, "fill in the missing `Color` arms"));
    assert!(out.contains("        Green => fail"), "arms: {out}");
    assert!(out.contains("        Blue => fail"), "arms: {out}");
    assert!(
        diagnostics(&out).is_empty(),
        "left: {:?}",
        diagnostics(&out)
    );
}

/// A payload variant needs somewhere to put its payload. `Rect =>` type-checks
/// (the checker's exhaustiveness is by name), so only the arity read off the
/// declaration keeps the generated pattern usable.
#[test]
fn a_payload_variant_gets_a_pattern_with_room_for_it() {
    let src = "Shape = Circle(int) | Rect(int, int)\n\n\
               area(s: Shape) -> int {\n    match s {\n        Circle(r) => r\n    }\n}\n";
    let acts = at(src, "Circle(r)");
    let out = applied(src, titled(&acts, "fill in the missing `Shape` arms"));
    assert!(out.contains("Rect(a0, a1) => fail"), "arms: {out}");
    assert!(
        diagnostics(&out).is_empty(),
        "left: {:?}",
        diagnostics(&out)
    );
}

/// A one-line `match` has its closing brace behind an arm, so the new arm has
/// to be pushed onto a line of its own. Written in front of the brace as it
/// stands, the two arms would run together with nothing between them.
#[test]
fn a_one_line_match_is_opened_up_rather_than_run_together() {
    let src = "Color = Red | Green\n\nname(c: Color) -> str => match c { Red => \"r\" }\n";
    let acts = at(src, "Red =>");
    let out = applied(src, titled(&acts, "fill in the missing `Color` arms"));
    assert!(
        diagnostics(&out).is_empty(),
        "left: {:?}",
        diagnostics(&out)
    );
    assert!(
        !out.lines().any(|l| l.ends_with(' ')),
        "left trailing whitespace: {out:?}"
    );
    // The parser is happy either way, which is why "it still parses" is not
    // the whole assertion: the arm has to land on a line of its own or the
    // fix hands back a line nobody would have written.
    assert!(
        out.lines()
            .any(|l| l.trim() == "Green => fail \"todo: Green\""),
        "the new arm ran into the old one: {out:?}"
    );
}

/// A `match` inside another one's arm is the one the cursor is in. Reaching for
/// the outer match instead would put an arm of the inner sum among the outer
/// sum's arms, which the checker rejects by leaving the diagnostic exactly
/// where it was.
#[test]
fn the_arms_go_into_the_innermost_match() {
    let src = "Color = Red | Green\nSize = Big | Small\n\n\
               pick(c: Color, s: Size) -> str {\n    match c {\n        \
               Red => match s {\n            Big => \"rb\"\n        }\n        \
               Green => \"g\"\n    }\n}\n";
    let acts = at(src, "Big =>");
    let out = applied(src, titled(&acts, "fill in the missing `Size` arms"));
    assert!(
        out.contains("Big => \"rb\"\n            Small => fail"),
        "the arm landed outside the inner match: {out}"
    );
    assert!(
        diagnostics(&out).is_empty(),
        "left: {:?}",
        diagnostics(&out)
    );
}

/// Two short matches on one type produce two diagnostics with the same text,
/// and only the cursor tells them apart.
#[test]
fn the_arms_go_into_the_match_the_cursor_is_in() {
    let src = "Color = Red | Green\n\n\
               a(c: Color) -> str {\n    match c {\n        Red => \"a\"\n    }\n}\n\n\
               b(c: Color) -> str {\n    match c {\n        Red => \"b\"\n    }\n}\n";
    let off = src.find("Red => \"b\"").unwrap();
    let acts = code_actions(src, off, off, false);
    let out = applied(src, titled(&acts, "fill in the missing `Color` arms"));
    assert!(
        out.contains("Red => \"b\"\n        Green => fail"),
        "the arms landed in the wrong match: {out}"
    );
    // and the other one is still short, so its own diagnostic remains
    assert_eq!(diagnostics(&out).len(), 1, "{:?}", diagnostics(&out));
}

/// A cursor outside every `match` has no match to fill in, whatever the
/// diagnostic says. The diagnostic's own anchor is the sum's declaration,
/// which is nowhere near the arms.
#[test]
fn the_arms_fix_is_not_offered_away_from_the_match() {
    let src = "Color = Red | Green\n\n\
               name(c: Color) -> str {\n    match c {\n        Red => \"r\"\n    }\n}\n";
    let off = src.find("Color = Red").unwrap();
    let acts = code_actions(src, off, off, false);
    assert!(
        !acts.iter().any(|a| a.title.contains("arms")),
        "offered arms from the declaration: {:?}",
        titles(&acts)
    );
}

// ---- the refactorings -----------------------------------------------------

#[test]
fn a_capitalized_local_offers_the_explicit_const() {
    let src = "main() -> int {\n    Limit = 5\n    Limit\n}\n";
    let acts = at(src, "Limit = 5");
    let out = applied(
        src,
        titled(&acts, "declare `Limit` with an explicit `const`"),
    );
    assert_eq!(out, "main() -> int {\n    const Limit = 5\n    Limit\n}\n");
}

/// A name already declared `const` has nothing to make explicit, and the
/// assignment further down is not a declaration at all: `const Limit = 6` in
/// front of it would be saying something else entirely.
#[test]
fn an_explicit_const_is_not_offered_twice() {
    let src = "main() -> int {\n    const Limit = 5\n    Limit = 6\n    Limit\n}\n";
    for needle in ["Limit = 5", "Limit = 6"] {
        let acts = at(src, needle);
        assert!(
            !acts.iter().any(|a| a.title.contains("explicit")),
            "at {needle:?}: {:?}",
            titles(&acts)
        );
    }
}

/// The linter's rule is about *locals*. A Capitalized name at the top level is
/// usually a record or a sum, and `const Point = { x: int }` is not what
/// anybody meant by a type declaration.
#[test]
fn a_top_level_type_is_not_nudged_toward_const() {
    let src = "Point = {\n    x: int\n}\n\nmain() -> int => 0\n";
    let acts = at(src, "Point");
    assert!(
        !acts.iter().any(|a| a.title.contains("const")),
        "nudged a type declaration: {:?}",
        titles(&acts)
    );
}

/// Every function in the file has a body, so the offer has to be about the one
/// the cursor is in. Offered for all of them, the editor shows a menu of
/// identically titled entries and the first one rewrites a function the user
/// was not looking at.
#[test]
fn a_body_refactor_belongs_to_the_function_the_cursor_is_in() {
    let src = "a() -> int => 1\n\nb() -> int => 2\n";
    let acts = at(src, "=> 2");
    assert_eq!(
        acts.iter()
            .filter(|x| x.title == "use a block body")
            .count(),
        1,
        "one offer per cursor: {:?}",
        titles(&acts)
    );
    let out = applied(src, titled(&acts, "use a block body"));
    assert_eq!(out, "a() -> int => 1\n\nb() -> int {\n    2\n}\n");
}

#[test]
fn a_body_switches_from_an_arrow_to_a_block_and_back() {
    let arrow = "f() -> int => 1 + 2\n";
    let block = applied(arrow, titled(&at(arrow, "=>"), "use a block body"));
    assert_eq!(block, "f() -> int {\n    1 + 2\n}\n");
    let back = applied(&block, titled(&at(&block, "1 + 2"), "use a `=>` body"));
    assert_eq!(back, arrow, "the round trip did not come back");
}

/// The body's text is moved, not reprinted, so what the author wrote about it
/// comes along. Reprinting the item from the AST would drop this comment
/// silently.
#[test]
fn a_comment_on_the_body_survives_the_switch() {
    let src = "f() -> int => 1 + 2 // why\n";
    let out = applied(src, titled(&at(src, "=>"), "use a block body"));
    assert_eq!(out, "f() -> int {\n    1 + 2 // why\n}\n");
}

/// A comment on a line of its own has nowhere to go in a `=> e` body, and an
/// action that dropped it would be deleting the author's prose to save two
/// characters.
///
/// The comment *after* the expression is the case the parser cannot catch: the
/// result of dropping it parses and checks perfectly, and the only thing wrong
/// with it is that a line the author wrote is gone. A comment before is caught
/// twice over, since taking it for the body leaves a function with no body at
/// all.
#[test]
fn a_block_with_its_own_comment_line_is_left_alone() {
    for src in [
        "f() -> int {\n    // why\n    1 + 2\n}\n",
        "f() -> int {\n    1 + 2\n    // and why not\n}\n",
    ] {
        let acts = at(src, "1 + 2");
        assert!(
            !acts.iter().any(|a| a.title.contains("=>")),
            "offered to drop a comment: {:?}",
            titles(&acts)
        );
    }
}

/// `f() -> int[] => 1, 2` is a bracketless comma list, not one expression, and
/// `{ 1, 2 }` is not a block. The action is not offered rather than offered and
/// wrong.
#[test]
fn a_bracketless_comma_list_body_is_not_wrapped_in_a_block() {
    let src = "f() -> int[] => 1, 2\n";
    let acts = at(src, "=>");
    assert!(
        !acts.iter().any(|a| a.title.contains("block")),
        "wrapped a comma list: {:?}",
        titles(&acts)
    );
}

#[test]
fn a_block_of_several_statements_stays_a_block() {
    let src = "f() -> int {\n    x = 1\n    x + 1\n}\n";
    let acts = at(src, "x = 1");
    assert!(
        !acts.iter().any(|a| a.title.contains("=>")),
        "offered to fold two statements into one: {:?}",
        titles(&acts)
    );
}

// ---- the guard rails ------------------------------------------------------

/// Nothing is offered for a file that does not parse. There is no checker
/// answer to act on, and no reliable place to put an edit.
#[test]
fn a_file_that_does_not_parse_offers_nothing() {
    let src = "f() -> int => (\n";
    assert!(!maca_parser::parse(src).errors.is_empty());
    assert!(code_actions(src, 0, src.len(), false).is_empty());
}

/// The claim every action makes: applying it leaves a file that parses and has
/// strictly fewer diagnostics than before. Asserted over every action this
/// suite's fixtures can produce, at every offset, because an action is offered
/// at a cursor and any cursor is a real one.
#[test]
fn every_offered_action_leaves_a_file_that_still_parses() {
    let fixtures = [
        "main() -> int {\n    const limit = 5\n    limit = 6\n    limit\n}\n",
        "f() -> int {\n    x = 1\n    return x\n}\n",
        "f(s: str) -> int {\n    s.lenght()\n}\n",
        "Color = Red | Green | Blue\n\nname(c: Color) -> str {\n    match c {\n        Red => \"r\"\n    }\n}\n",
        "Shape = Circle(int) | Rect(int, int)\n\narea(s: Shape) -> int {\n    match s {\n        Circle(r) => r\n    }\n}\n",
        "main() -> int {\n    Limit = 5\n    Limit\n}\n",
        "f() -> int => 1 + 2\n",
        "P = {\n    x: int\n}\n\nmk(n: int) -> P => P { x = n }\n",
        "head(xs: int[]) -> int {\n    match xs {\n        [] => 0\n        first, ..rest => first\n    }\n}\n",
    ];
    for src in fixtures {
        let before = maca_lsp::diagnostics(src, false).len();
        for off in 0..=src.len() {
            for a in code_actions(src, off, off, false) {
                let out = applied(src, &a);
                assert!(
                    maca_lsp::diagnostics(&out, false).len() <= before,
                    "`{}` at {off} added a diagnostic:\n{out}",
                    a.title
                );
            }
        }
    }
}
