//! Guard the Zed extension's tree-sitter queries against grammar drift.
//!
//! The `.scm` files are consumed by Zed, not by the Rust workspace, so nothing
//! else would notice if a grammar rule were renamed out from under them. These
//! checks are deliberately lightweight (no tree-sitter dependency): every node
//! type a query names must still exist in `grammar.js`.

use std::path::PathBuf;

fn repo(rel: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(rel)
}

fn read(rel: &str) -> String {
    std::fs::read_to_string(repo(rel)).unwrap_or_else(|e| panic!("read {rel}: {e}"))
}

/// Node types named by a query: the identifiers written as `(name`. Comments,
/// string literals (a regex inside `#match?` can contain `(alt|alt)`), and
/// `(#predicate? …)` forms are skipped, so only real node names are returned.
fn node_types(scm: &str) -> Vec<String> {
    let mut out = Vec::new();
    let b = scm.as_bytes();
    let mut i = 0;
    while i < b.len() {
        // skip comments
        if b[i] == b';' {
            while i < b.len() && b[i] != b'\n' {
                i += 1;
            }
            continue;
        }
        // skip string literals
        if b[i] == b'"' {
            i += 1;
            while i < b.len() && b[i] != b'"' {
                i += if b[i] == b'\\' { 2 } else { 1 };
            }
            i += 1;
            continue;
        }
        if b[i] == b'(' {
            let mut j = i + 1;
            while j < b.len() && (b[j].is_ascii_alphanumeric() || b[j] == b'_') {
                j += 1;
            }
            if j > i + 1 && b[i + 1].is_ascii_lowercase() {
                out.push(scm[i + 1..j].to_string());
            }
            i = j;
            continue;
        }
        i += 1;
    }
    out.sort();
    out.dedup();
    out
}

#[test]
fn zed_queries_reference_only_real_grammar_rules() {
    let grammar = read("editor/tree-sitter-maca/grammar.js");
    for q in ["outline.scm", "indents.scm", "highlights.scm"] {
        let scm = read(&format!("editor/zed-maca/languages/maca/{q}"));
        for node in node_types(&scm) {
            assert!(
                grammar.contains(&format!("{node}:")),
                "{q} references node `{node}`, which no longer exists in grammar.js"
            );
        }
    }
}

#[test]
fn outline_query_covers_functions_and_type_declarations() {
    // The symbol picker must list both kinds of top-level definition; a
    // regression here silently empties Zed's outline.
    let scm = read("editor/zed-maca/languages/maca/outline.scm");
    assert!(
        scm.contains("(function"),
        "outline lost function definitions"
    );
    assert!(
        scm.contains("(type_decl"),
        "outline lost type declarations (records/sums)"
    );
    assert!(
        scm.contains("@name") && scm.contains("@item"),
        "outline missing captures"
    );
}
