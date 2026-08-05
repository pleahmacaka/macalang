mod common;
use common::*;

use serde_json::Value;
use std::process::Command;

/// A file with one defect of each kind that the checker can reach without a toolchain.
const BROKEN: &str = "main() -> int {\n\
                      \x20   let x = 1\n\
                      \x20   total = 0 as const\n\
                      \x20   total = 5\n\
                      \x20   nope()\n\
                      \x20   x\n\
                      }\n";

fn scratch(name: &str, source: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join("maca-check-json");
    std::fs::create_dir_all(&dir).expect("scratch dir");
    let file = dir.join(name);
    std::fs::write(&file, source).expect("write source");
    file
}

fn check_json(file: &std::path::Path) -> Value {
    let out = Command::new(maca())
        .args(["check", "--json", &file.to_string_lossy()])
        .output()
        .expect("spawn maca check");
    serde_json::from_slice(&out.stdout).unwrap_or_else(|e| {
        panic!(
            "not JSON: {e}\n{}\n{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        )
    })
}

/// The schema is committed for consumers, so it has to describe what is actually printed.
#[test]
fn the_output_matches_the_committed_schema() {
    let schema: Value = serde_json::from_str(
        &std::fs::read_to_string(repo().join("docs/check-json.schema.json"))
            .expect("the schema is committed"),
    )
    .expect("the schema is JSON");

    let out = check_json(&scratch("schema.maca", BROKEN));
    assert_eq!(out["format"], 1);

    let required: Vec<&str> = schema["$defs"]["diagnostic"]["required"]
        .as_array()
        .expect("required list")
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect();
    let diags = out["diagnostics"].as_array().expect("a diagnostics array");
    assert!(!diags.is_empty(), "the broken file produced nothing");

    for d in diags {
        for field in &required {
            assert!(
                d.get(field).is_some(),
                "the schema requires `{field}` and the output has no such key:\n{d:#}"
            );
        }
        let code = d["code"].as_str().unwrap();
        assert!(
            code.len() == 5
                && code.starts_with('M')
                && code[1..].chars().all(|c| c.is_ascii_digit()),
            "`{code}` is not an M0000 code"
        );
        for s in d["suggestions"].as_array().unwrap() {
            assert!(
                matches!(
                    s["applicability"].as_str(),
                    Some("machine-applicable" | "maybe-incorrect")
                ),
                "unknown applicability: {s:#}"
            );
        }
    }
}

/// A span the reader cannot trust is worse than none, so it has to name the text it points at.
#[test]
fn a_span_points_at_the_text_the_message_is_about() {
    let file = scratch("span.maca", BROKEN);
    let src = std::fs::read_to_string(&file).unwrap();
    let out = check_json(&file);

    let d = out["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .find(|d| d["message"].as_str().is_some_and(|m| m.contains("`let`")))
        .expect("the `let` diagnostic");

    let start = d["span"]["start"].as_u64().unwrap() as usize;
    let end = d["span"]["end"].as_u64().unwrap() as usize;
    assert_eq!(&src[start..end], "let", "the span does not cover `let`");
    assert_eq!(d["span"]["start_line"], 2);
    assert_eq!(d["span"]["start_column"], 5);
}

/// `maca fix` is only allowed to touch what is safe, and has to leave the rest for a person.
#[test]
fn fix_applies_the_safe_edit_and_leaves_the_rest() {
    let file = scratch("fix.maca", BROKEN);
    let before = check_json(&file);
    let count = |v: &Value| v["diagnostics"].as_array().unwrap().len();

    let out = Command::new(maca())
        .args(["fix", &file.to_string_lossy()])
        .output()
        .expect("spawn maca fix");
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );

    let after_src = std::fs::read_to_string(&file).unwrap();
    assert!(
        !after_src.contains("let x"),
        "the machine-applicable edit did not land:\n{after_src}"
    );
    assert!(
        after_src.contains("as const") && after_src.contains("nope()"),
        "fix touched something it was not allowed to:\n{after_src}"
    );

    let after = check_json(&file);
    assert!(
        count(&after) < count(&before),
        "fixing removed no diagnostic: {} then {}",
        count(&before),
        count(&after)
    );
}

/// The language server and `--json` read one resolver, so a diagnostic lands in one place rather than two.
#[test]
fn the_language_server_places_a_diagnostic_where_the_json_does() {
    let file = scratch("agree.maca", BROKEN);
    let src = std::fs::read_to_string(&file).unwrap();
    let out = check_json(&file);

    let from_lsp = maca_lsp::diagnostics_located(&src, false);
    assert!(!from_lsp.is_empty(), "the server found nothing");

    for located in &from_lsp {
        let matched = out["diagnostics"].as_array().unwrap().iter().any(|d| {
            d["span"]["start"].as_u64() == Some(located.start as u64)
                && d["span"]["end"].as_u64() == Some(located.end as u64)
        });
        assert!(
            matched,
            "the server puts a diagnostic at {}..{} and `--json` has nothing there:\n{}",
            located.start, located.end, located.message
        );
    }
}

/// A file that is fine says so by exiting zero with an empty list, which is what an agent loop tests.
#[test]
fn a_clean_file_is_an_empty_list_and_a_zero_exit() {
    let file = scratch("clean.maca", "main() -> int {\n    0\n}\n");
    let out = Command::new(maca())
        .args(["check", "--json", &file.to_string_lossy()])
        .output()
        .expect("spawn maca check");
    assert!(out.status.success(), "a clean file should exit zero");

    let v: Value = serde_json::from_slice(&out.stdout).expect("JSON");
    assert_eq!(v["diagnostics"].as_array().unwrap().len(), 0);
}
