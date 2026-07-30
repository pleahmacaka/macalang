//! Every UFCS method, lowered to JS and executed.
//!
//! `jcall` handed each method name straight to JS, which was wrong twice over.
//! Some do not exist there — `.length` is a property, so `xs.length()` threw
//! `TypeError`. The dangerous ones are those that exist and mean something else:
//! `push` returns the new *length*, and `sort` compares as strings, so
//! `[10, 9, 2].sort()` came back `[10, 2, 9]`.
//!
//! Expected values here are the answers the **native** backend gives for the
//! same expression — the native path is the reference, and a target that
//! disagrees with it is wrong even when its own answer looks reasonable.

use std::io::Write;
use std::process::Command;

/// Emit `src`, then evaluate each expression in `calls` under Node.
fn run(src: &str, calls: &[&str]) -> Vec<String> {
    let p = maca_parser::parse(src);
    assert!(p.errors.is_empty(), "parse: {:?}", p.errors);
    let js = maca_backend_js::emit(&p.module).js;

    let mut h = std::collections::hash_map::DefaultHasher::new();
    std::hash::Hash::hash(&(src, calls), &mut h);
    let key = std::hash::Hasher::finish(&h);
    let dir = std::env::temp_dir().join(format!("maca-js-meth-{}-{key:x}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let module = dir.join("app.js");
    std::fs::File::create(&module)
        .unwrap()
        .write_all(js.as_bytes())
        .unwrap();

    let mut d = String::from("const m = require(\"./app.js\");\nwith (m) {\n");
    for c in calls {
        d.push_str(&format!("  console.log(JSON.stringify({c}));\n"));
    }
    d.push_str("}\n");
    let driver = dir.join("run.js");
    std::fs::File::create(&driver)
        .unwrap()
        .write_all(d.as_bytes())
        .unwrap();

    let out = Command::new("node")
        .arg(&driver)
        .output()
        .expect("node is required for the JS backend tests");
    assert!(
        out.status.success(),
        "node failed\n--- stderr ---\n{}\n--- js ---\n{js}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .map(|l| l.to_string())
        .collect()
}

/// One Maca function per method, so each can be called from JS by name.
const PROBE: &str = r#"
s_length() -> int => "hello".length()
s_upper() -> str => "aB".upper()
s_lower() -> str => "aB".lower()
s_trim() -> str => "  x  ".trim()
s_contains() -> bool => "hello".contains("ell")
s_starts_with() -> bool => "hello".starts_with("he")
s_ends_with() -> bool => "hello".ends_with("lo")
s_replace() -> str => "aXbXc".replace("X", "-")
s_substr() -> str => "hello".substr(1, 3)
s_slice() -> str => "hello".slice(1, 3)
s_index_of() -> int => "hello".index_of("l")
s_index_of_missing() -> int => "hello".index_of("z")
s_repeat() -> str => "ab".repeat(3)
s_pad_start() -> str => "7".pad_start(3, "0")
s_pad_end() -> str => "7".pad_end(3, "0")
s_pad_center() -> str => "x".pad_center(6, ".")
s_split() -> int => "a,b,c".split(",").length()
s_chars() -> int => "abc".chars().length()
s_at() -> str => "abc".at(1)
s_is_whitespace() -> bool => " ".is_whitespace()
s_is_ascii_digit() -> bool => "5".is_ascii_digit()
s_is_alpha() -> bool => "z".is_alpha()

l_length() -> int => [1, 2, 3].length()
l_map() -> int => [1, 2, 3].map(v => v * 2).sum()
l_filter() -> int => [1, 2, 3, 4].filter(v => v > 2).length()
l_reduce() -> int => [1, 2, 3].reduce(0, (a, b) => a + b)
l_fold() -> int => [1, 2, 3].fold(10, (a, b) => a + b)
l_sort() -> int[] => [10, 9, 2].sort()
l_sort_str() -> str => ["b", "a", "c"].sort().join("|")
l_reverse() -> int[] => [1, 2, 3].reverse()
l_push() -> int[] => [1, 2].push(3)
l_pop() -> int[] => [1, 2, 3].pop()
l_slice() -> int[] => [1, 2, 3, 4].slice(1, 3)
l_contains() -> bool => [1, 2].contains(2)
l_index_of() -> int => [5, 6].index_of(6)
l_index_of_missing() -> int => [5, 6].index_of(9)
l_sum() -> int => [1, 2, 3].sum()
l_min() -> int => [3, 1, 2].min()
l_max() -> int => [3, 1, 2].max()
l_first() -> int => [7, 8].first()
l_last() -> int => [7, 8].last()
l_get() -> int => [7, 8].get(1)
l_join() -> str => ["a", "b"].join("-")
l_parallel() -> int => [1, 2, 3].parallel(v => v * 2).sum()

main() -> int => 0
"#;

/// (function, the answer the native backend gives).
const EXPECTED: &[(&str, &str)] = &[
    ("s_length", "5"),
    ("s_upper", "\"AB\""),
    ("s_lower", "\"ab\""),
    ("s_trim", "\"x\""),
    ("s_contains", "true"),
    ("s_starts_with", "true"),
    ("s_ends_with", "true"),
    // every occurrence, which JS `replace` would not have done
    ("s_replace", "\"a-b-c\""),
    // `substr(start, len)`, not JS `slice(start, end)`
    ("s_substr", "\"ell\""),
    ("s_slice", "\"el\""),
    ("s_index_of", "2"),
    ("s_index_of_missing", "-1"),
    ("s_repeat", "\"ababab\""),
    ("s_pad_start", "\"007\""),
    ("s_pad_end", "\"700\""),
    // the shortfall splits left-biased
    ("s_pad_center", "\"..x...\""),
    ("s_split", "3"),
    ("s_chars", "3"),
    ("s_at", "\"b\""),
    ("s_is_whitespace", "true"),
    ("s_is_ascii_digit", "true"),
    ("s_is_alpha", "true"),
    ("l_length", "3"),
    ("l_map", "12"),
    ("l_filter", "2"),
    ("l_reduce", "6"),
    ("l_fold", "16"),
    // the one that mattered most: JS's default sort puts 10 before 9
    ("l_sort", "[2,9,10]"),
    ("l_sort_str", "\"a|b|c\""),
    ("l_reverse", "[3,2,1]"),
    // JS `push` returns the new length, not the list
    ("l_push", "[1,2,3]"),
    ("l_pop", "[1,2]"),
    ("l_slice", "[2,3]"),
    ("l_contains", "true"),
    ("l_index_of", "1"),
    ("l_index_of_missing", "-1"),
    ("l_sum", "6"),
    ("l_min", "1"),
    ("l_max", "3"),
    ("l_first", "7"),
    ("l_last", "8"),
    ("l_get", "8"),
    ("l_join", "\"a-b\""),
    ("l_parallel", "12"),
];

#[test]
fn every_method_computes_what_the_native_backend_computes() {
    let calls: Vec<String> = EXPECTED.iter().map(|(f, _)| format!("{f}()")).collect();
    let refs: Vec<&str> = calls.iter().map(|s| s.as_str()).collect();
    let got = run(PROBE, &refs);
    assert_eq!(got.len(), EXPECTED.len(), "missing output: {got:?}");
    let mut wrong = Vec::new();
    for ((name, want), have) in EXPECTED.iter().zip(&got) {
        if want != have {
            wrong.push(format!("{name}: want {want}, got {have}"));
        }
    }
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}

#[test]
fn every_name_in_both_closed_sets_has_a_lowering() {
    // The sets are the contract. Without this, adding a method to `maca-core`
    // silently leaves JS emitting a name that does not exist there.
    let covered: std::collections::BTreeSet<&str> = EXPECTED
        .iter()
        .map(|(f, _)| {
            let bare = f.trim_start_matches("s_").trim_start_matches("l_");
            bare.trim_end_matches("_missing").trim_end_matches("_str")
        })
        .collect();
    let mut missing = Vec::new();
    for m in maca_core::STR_METHODS.iter().chain(maca_core::LIST_METHODS) {
        if !covered.contains(m) {
            missing.push(*m);
        }
    }
    assert!(
        missing.is_empty(),
        "no JS lowering exercised for: {missing:?}"
    );
}

#[test]
fn a_list_method_does_not_mutate_its_receiver() {
    // Maca lists are values. JS `sort`, `reverse` and `push` all mutate, so the
    // answer could look right while a second holder of the list saw the change.
    let src = r#"
after_sort() -> str {
    xs = [3, 1, 2]
    ys = xs.sort()
    str(xs.get(0)) ++ "|" ++ str(ys.get(0))
}

after_reverse() -> str {
    xs = [1, 2, 3]
    ys = xs.reverse()
    str(xs.get(0)) ++ "|" ++ str(ys.get(0))
}

after_push() -> str {
    xs = [1, 2]
    ys = xs.push(9)
    str(xs.length()) ++ "|" ++ str(ys.length())
}

main() -> int => 0
"#;
    let got = run(src, &["after_sort()", "after_reverse()", "after_push()"]);
    assert_eq!(got, vec!["\"3|1\"", "\"1|3\"", "\"2|3\""]);
}

#[test]
fn a_helper_is_emitted_only_when_it_is_used() {
    let with =
        maca_backend_js::emit(&maca_parser::parse("f() -> int[] => [2, 1].sort()\n").module).js;
    assert!(with.contains("function _msort"), "helper missing:\n{with}");
    assert!(
        with.contains("function _mcmp"),
        "_msort needs _mcmp:\n{with}"
    );

    let without = maca_backend_js::emit(&maca_parser::parse("f() -> int => 1 + 1\n").module).js;
    assert!(
        !without.contains("function _msort"),
        "unused helper emitted:\n{without}"
    );
}

#[test]
fn use_strict_stays_the_first_statement() {
    let js = maca_backend_js::emit(&maca_parser::parse(PROBE).module).js;
    assert!(
        js.trim_start().starts_with("\"use strict\""),
        "directive is no longer first:\n{}",
        &js[..js.len().min(200)]
    );
}

#[test]
fn a_field_holding_a_function_is_still_called_directly() {
    // Only Maca's own method names are rewritten; anything else is a record
    // field or a foreign JS call and must pass through untouched.
    let js = maca_backend_js::emit(&maca_parser::parse("f(r) -> int => r.handler(1)\n").module).js;
    assert!(js.contains("r.handler(1)"), "rewrote a field call:\n{js}");
}
