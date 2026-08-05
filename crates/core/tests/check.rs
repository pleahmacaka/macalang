use maca_core::{DiagKind, Mode, check};
use std::path::PathBuf;

/// The source as the compiler sees it: with `import a/b` resolved and inlined.
fn read(rel: &str) -> String {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(rel);
    maca_parser::imports::load_with_imports(&p).unwrap_or_else(|e| panic!("{rel}: {e}"))
}

fn diags(rel: &str, mode: Mode) -> Vec<maca_core::Diagnostic> {
    let src = read(rel);
    let parsed = maca_parser::parse(&src);
    assert!(
        parsed.errors.is_empty(),
        "{rel} parse errors: {:?}",
        parsed.errors
    );
    check(&parsed.module, mode)
}

fn assert_clean(rel: &str, mode: Mode) {
    let d = diags(rel, mode);
    assert!(d.is_empty(), "{rel} should typecheck, got: {d:?}");
}

fn assert_has(rel: &str, mode: Mode, kind: DiagKind) {
    let d = diags(rel, mode);
    assert!(
        d.iter().any(|x| x.kind == kind),
        "{rel} expected {kind:?}, got: {d:?}"
    );
}

#[test]
fn hello_ok() {
    assert_clean("apps/examples/hello.maca", Mode::Program);
}
#[test]
fn taskr_ok() {
    assert_clean("apps/examples/taskr.maca", Mode::Program);
}
#[test]
fn counter_ok() {
    assert_clean("apps/examples/counter.maca", Mode::Program);
}
#[test]
fn dot_ok() {
    assert_clean("apps/examples/dot.maca", Mode::Program);
}
#[test]
fn system_ok() {
    assert_clean("apps/examples/system.maca", Mode::Config);
}
#[test]
fn operators_ok() {
    assert_clean("apps/examples/operators.maca", Mode::Program);
}
#[test]
fn generic_ok() {
    assert_clean("apps/examples/generic.maca", Mode::Program);
}
#[test]
fn keywords_ok() {
    assert_clean("apps/examples/keywords.maca", Mode::Program);
}
#[test]
fn indexing_ok() {
    assert_clean("apps/examples/indexing.maca", Mode::Program);
}
#[test]
fn record_update_ok() {
    assert_clean("apps/examples/record_update.maca", Mode::Program);
}
#[test]
fn recursive_sum_ok() {
    assert_clean("apps/examples/tree.maca", Mode::Program);
}
#[test]
fn recursive_record_ok() {
    assert_clean("apps/examples/recursive_record.maca", Mode::Program);
}
#[test]
fn sum_record_ok() {
    assert_clean("apps/examples/sum_record.maca", Mode::Program);
}

#[test]
fn type_mismatch_rejected() {
    assert_has(
        "apps/examples/bad/type_mismatch.maca",
        Mode::Program,
        DiagKind::TypeMismatch,
    );
}
#[test]
fn nonexhaustive_rejected() {
    assert_has(
        "apps/examples/bad/nonexhaustive.maca",
        Mode::Program,
        DiagKind::NonExhaustive,
    );
}
#[test]
fn effect_in_config_rejected() {
    assert_has(
        "apps/examples/bad/effect_in_config.maca",
        Mode::Config,
        DiagKind::EffectInConfig,
    );
}
/// An effect in *statement* position, not only in a binding's value.
#[test]
fn effect_statement_in_config_rejected() {
    assert_has(
        "apps/examples/bad/effect_statement_in_config.maca",
        Mode::Config,
        DiagKind::EffectInConfig,
    );
}
#[test]
fn unknown_option_rejected() {
    assert_has(
        "apps/examples/bad/unknown_option.maca",
        Mode::Config,
        DiagKind::UnknownOption,
    );
}
#[test]
fn arg_mismatch_rejected() {
    assert_has(
        "apps/examples/bad/arg_mismatch.maca",
        Mode::Program,
        DiagKind::TypeMismatch,
    );
}
#[test]
fn arity_rejected() {
    assert_has(
        "apps/examples/bad/arity.maca",
        Mode::Program,
        DiagKind::TypeMismatch,
    );
}
/// A variadic parameter is a lowered declaration now, not a rejected one.
#[test]
fn variadic_typechecks() {
    assert_clean("crates/driver/tests/programs/variadic.maca", Mode::Program);
}

/// The rules it does have belong to *this* layer.
#[test]
fn variadic_misuse_rejected() {
    for bad in [
        "bad_variadic_value",
        "bad_variadic_arity",
        "bad_variadic_not_last",
        "bad_variadic_unannotated",
        "bad_variadic_main",
    ] {
        assert_has(
            &format!("crates/driver/tests/programs/{bad}.maca"),
            Mode::Program,
            DiagKind::TypeMismatch,
        );
    }
}
/// A record literal has to name every field the record declares.
#[test]
fn missing_record_field_rejected() {
    assert_has(
        "apps/examples/bad/missing_field.maca",
        Mode::Program,
        DiagKind::TypeMismatch,
    );
}

/// And no field it doesn't declare.
#[test]
fn unknown_record_field_rejected() {
    assert_has(
        "apps/examples/bad/unknown_field.maca",
        Mode::Program,
        DiagKind::TypeMismatch,
    );
}

#[test]
fn branch_mismatch_rejected() {
    assert_has(
        "apps/examples/bad/branch_mismatch.maca",
        Mode::Program,
        DiagKind::TypeMismatch,
    );
}

#[test]
fn loops_ok() {
    assert_clean("apps/examples/loops.maca", Mode::Program);
}

#[test]
fn while_cond_must_be_bool() {
    assert_has(
        "apps/examples/bad/while_cond.maca",
        Mode::Program,
        DiagKind::TypeMismatch,
    );
}

#[test]
fn payload_sum_ok() {
    assert_clean("apps/examples/payload_sum.maca", Mode::Program);
}

#[test]
fn range_ok() {
    assert_clean("apps/examples/range.maca", Mode::Program);
}

#[test]
fn tour_ok() {
    assert_clean("apps/examples/tour.maca", Mode::Program);
}

#[test]
fn range_end_must_be_int() {
    assert_has(
        "apps/examples/bad/range_end.maca",
        Mode::Program,
        DiagKind::TypeMismatch,
    );
}

#[test]
fn reassign_constant_rejected() {
    assert_has(
        "apps/examples/bad/reassign_const.maca",
        Mode::Program,
        DiagKind::Immutable,
    );
}

#[test]
fn mutable_reassign_ok() {
    assert_clean("apps/examples/loops.maca", Mode::Program);
}

#[test]
fn collections_and_math_examples_typecheck() {
    assert_clean("apps/examples/collections.maca", Mode::Program);
    assert_clean("apps/examples/math.maca", Mode::Program);
    assert_clean("apps/examples/dot.maca", Mode::Program);
}

#[test]
fn async_example_typechecks() {
    assert_clean("apps/examples/async.maca", Mode::Program);
}

#[test]
fn async_effect_is_inferred_and_banned_in_config() {
    let src = "svc.value = spawn work(1)\n";
    let parsed = maca_parser::parse(src);
    let d = check(&parsed.module, Mode::Config);
    let eff = d.iter().find(|x| x.kind == DiagKind::EffectInConfig);
    assert!(eff.is_some(), "async effect not flagged in config: {d:?}");
    assert!(
        eff.unwrap().msg.contains("async"),
        "effect name missing: {}",
        eff.unwrap().msg
    );
}

#[test]
fn undefined_call_rejected() {
    assert_has(
        "apps/examples/bad/undefined_call.maca",
        Mode::Program,
        DiagKind::UndefinedName,
    );
}

/// A capitalized pattern is a constructor, so a misspelt one is a name nothing declares rather than a binding that matches everything.
#[test]
fn misspelt_variant_pattern_rejected() {
    assert_has(
        "apps/examples/bad/misspelt_variant.maca",
        Mode::Program,
        DiagKind::UndefinedName,
    );
}

/// The diagnostic names the variant that was meant, and a lowercase pattern still binds.
#[test]
fn a_misspelt_variant_is_told_which_one_was_meant() {
    let said: Vec<String> = diags("apps/examples/bad/misspelt_variant.maca", Mode::Program)
        .iter()
        .map(|d| d.msg.clone())
        .collect();
    assert!(
        said.iter().any(|m| m.contains("did you mean `Busy`?")),
        "{said:?}"
    );

    let binding = "Mode = Idle | Busy\n\nname(m: Mode) -> str =>\n    \
                   match m {\n        Idle => \"idle\"\n        other => \"{other}\"\n    }\n";
    let ok = maca_parser::parse(binding);
    let left = check(&ok.module, Mode::Program);
    assert!(
        left.is_empty(),
        "a lowercase pattern binds what it matched: {left:?}"
    );
}

#[test]
fn ui_element_tags_are_not_undefined() {
    let src = "main() -> Element =>\n    div(\n        button(\"ok\")\n        input(placeholder=\"name\")\n    )\n";
    let parsed = maca_parser::parse(src);
    let d = check(&parsed.module, Mode::Program);
    assert!(
        !d.iter().any(|x| x.kind == DiagKind::UndefinedName),
        "UI tags wrongly flagged: {d:?}"
    );
}

/// A keyword Maca doesn't have gets told what Maca does instead, and told it *first*.
#[test]
fn phantom_keywords_lead_with_what_maca_does() {
    for (word, opens_with) in [
        ("let", "write `x = e`"),
        ("var", "write `x = e`"),
        ("fn", "write the signature straight out"),
        ("func", "write the signature straight out"),
        ("def", "write the signature straight out"),
        ("type", "declare a type by binding it"),
        ("async", "async is an inferred effect"),
        ("null", "a sum type with an empty variant"),
        ("nil", "a sum type with an empty variant"),
    ] {
        let src = format!("f() -> int {{\n    {word}\n    1\n}}\n");
        let parsed = maca_parser::parse(&src);
        let d = check(&parsed.module, Mode::Program);
        let lead = format!("`{word}`: ");
        let hit = d
            .iter()
            .find(|x| x.kind == DiagKind::UndefinedName && x.msg.starts_with(&lead))
            .unwrap_or_else(|| panic!("no hint for `{word}`: {d:?}"));
        let hint = &hit.msg[lead.len()..];
        assert!(
            hint.starts_with(opens_with),
            "`{word}` should open with {opens_with:?}, opens with {hint:?}"
        );
        assert!(
            !hint.starts_with("Maca has no"),
            "`{word}` leads with the denial again: {hint:?}"
        );
    }
}

/// `return` left the phantom list because Maca has it now, and it takes the other words' places with it.
#[test]
fn return_is_a_keyword_and_not_a_phantom_one() {
    let d = check_src("f() -> int {\n    return 1\n}\n");
    assert!(
        d.is_empty(),
        "`return` should be accepted, not hinted at: {d:?}"
    );
    for word in ["let", "type", "null"] {
        let src = format!("f() -> int {{\n    {word}\n    1\n}}\n");
        let d = check_src(&src);
        assert!(
            d.iter()
                .any(|x| x.kind == DiagKind::UndefinedName
                    && x.msg.starts_with(&format!("`{word}`: "))),
            "`{word}` stopped being a phantom keyword: {d:?}"
        );
    }
}

/// The hint fires only on names that are defined nowhere, so a program that legitimately binds one of these words still compiles.
#[test]
fn a_defined_name_is_never_a_phantom_keyword() {
    let src = "type(n: int) -> int => n + 1\n\nmain() -> int => type(41)\n";
    let parsed = maca_parser::parse(src);
    let d = check(&parsed.module, Mode::Program);
    assert!(
        !d.iter().any(|x| x.kind == DiagKind::UndefinedName),
        "a user-defined `type` was flagged as a phantom keyword: {d:?}"
    );
}

/// Check a source string in program mode.
fn check_src(src: &str) -> Vec<maca_core::Diagnostic> {
    let parsed = maca_parser::parse(src);
    assert!(parsed.errors.is_empty(), "{src}: {:?}", parsed.errors);
    check(&parsed.module, Mode::Program)
}

fn assert_src_clean(src: &str) {
    let d = check_src(src);
    assert!(d.is_empty(), "should typecheck:\n{src}\ngot: {d:?}");
}

/// `Point = { x: int, y: int }` and `{ x = 5, y = 6 }` are the same type.
#[test]
fn a_record_literal_meets_the_record_type_it_is_written_into() {
    let decl = "Point = { x: int, y: int }\nLine = { a: Point, b: Point }\n\n";
    for (what, src) in [
        (
            "a local binding",
            "main() -> int {\n    p: Point = { x = 5, y = 6 }\n    p.x\n}\n",
        ),
        (
            "a top-level binding",
            "Origin: Point = { x = 0, y = 0 }\n\nmain() -> int => Origin.x\n",
        ),
        (
            "return position",
            "mk() -> Point => { x = 1, y = 2 }\n\nmain() -> int => mk().x\n",
        ),
        (
            "a parameter",
            "far(p: Point) -> int => p.x\n\nmain() -> int => far({ x = 1, y = 2 })\n",
        ),
        (
            "a nested field",
            "main() -> int {\n    l: Line = { a = { x = 1, y = 2 }, b = { x = 3, y = 4 } }\n    l.a.x\n}\n",
        ),
        (
            "a list element",
            "main() -> int {\n    ps: Point[] = [{ x = 1, y = 2 }]\n    ps[0].y\n}\n",
        ),
    ] {
        let d = check_src(&format!("{decl}{src}"));
        assert!(d.is_empty(), "{what} should typecheck:\n{src}\ngot: {d:?}");
    }
}

/// Meeting the declaration is also what checks the literal against it.
#[test]
fn a_literal_written_into_a_record_type_must_name_its_fields() {
    for (src, want) in [
        (
            "main() -> int {\n    p: Point = { x = 5 }\n    p.x\n}\n",
            "missing field `y`",
        ),
        (
            "main() -> int {\n    p: Point = { x = 5, y = 6, z = 7 }\n    p.x\n}\n",
            "unexpected field `z`",
        ),
        (
            "main() -> int {\n    p: Point = { x = 5, y = \"six\" }\n    p.x\n}\n",
            "expected int, found str",
        ),
        (
            "main() -> int {\n    p: Point = {}\n    p.x\n}\n",
            "missing field `x`",
        ),
    ] {
        let d = check_src(&format!("Point = {{ x: int, y: int }}\n\n{src}"));
        assert!(
            d.iter()
                .any(|x| x.kind == DiagKind::TypeMismatch && x.msg.contains(want)),
            "expected {want:?} for:\n{src}\ngot: {d:?}"
        );
    }
}

/// Two anonymous literals still meet structurally, and stay open.
#[test]
fn two_anonymous_record_literals_still_meet_structurally() {
    assert_src_clean(
        "pick(c: bool) -> int {\n    p = c ? { x = 1 } : { x = 2 }\n    p.x\n}\n\
         \nmain() -> int => pick(true)\n",
    );
}

/// A real mismatch still is one, and is reported the way the author wrote it.
#[test]
fn expected_names_the_annotation_and_found_names_the_value() {
    for (src, want) in [
        (
            "Point = { x: int, y: int }\n\nmain() -> int {\n    p: Point = 3\n    p.x\n}\n",
            "expected Point, found int",
        ),
        (
            "f() -> str => 42\n\nmain() -> int => 0\n",
            "expected str, found int",
        ),
        (
            "h(n: int) -> int => n\n\nmain() -> int => h(\"s\")\n",
            "expected int, found str",
        ),
        (
            "main() -> int {\n    while 7 {\n        break\n    }\n    0\n}\n",
            "expected bool, found int",
        ),
        (
            "main() -> int {\n    for i in \"a\"..\"b\" {\n        info(\"{i}\")\n    }\n    0\n}\n",
            "expected int, found str",
        ),
    ] {
        let d = check_src(src);
        assert!(
            d.iter()
                .any(|x| x.kind == DiagKind::TypeMismatch && x.msg.contains(want)),
            "expected {want:?} for:\n{src}\ngot: {d:?}"
        );
    }
}
