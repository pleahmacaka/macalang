use maca_core::{Applicability, Diagnostic, Span};
use serde_json::{Value, json};
use std::path::Path;

/// The version of the `--json` shape, so a consumer can refuse a format it does not know.
pub const FORMAT: u32 = 1;

/// Parse and check one file, returning its diagnostics beside the source they are about.
fn diagnose(path: &Path, config: bool) -> Result<(String, Vec<Diagnostic>), String> {
    let src = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let whole = maca_parser::imports::load_with_imports(path).unwrap_or_else(|_| src.clone());
    let parsed = maca_parser::parse(&whole);
    if !parsed.errors.is_empty() {
        return Ok((
            src,
            parsed
                .errors
                .iter()
                .map(|m| {
                    Diagnostic::new(maca_core::DiagKind::TypeMismatch, m.clone())
                        .with_note("the file did not parse, so nothing after this was checked")
                })
                .collect(),
        ));
    }
    let mode = if config {
        maca_core::Mode::Config
    } else {
        maca_core::Mode::Program
    };
    let diags = maca_core::check(&parsed.module, mode);
    Ok((src, diags))
}

fn span_json(span: &Span) -> Value {
    json!({
        "start": span.start,
        "end": span.end,
        "start_line": span.start_pos.line,
        "start_column": span.start_pos.column,
        "end_line": span.end_pos.line,
        "end_column": span.end_pos.column,
    })
}

/// One diagnostic as the object `maca check --json` prints.
fn diag_json(path: &Path, src: &str, d: &Diagnostic) -> Value {
    let span = maca_core::resolve_span(src, d);
    let suggestions: Vec<Value> = d
        .suggestions
        .iter()
        .map(|s| {
            let at = s
                .span
                .map(|(a, b)| maca_core::span_at(src, a, b))
                .unwrap_or(span);
            json!({
                "message": s.message,
                "span": span_json(&at),
                "replacement": s.replacement,
                "applicability": s.applicability.as_str(),
            })
        })
        .collect();
    json!({
        "code": d.kind.code(),
        "severity": d.kind.severity().as_str(),
        "message": d.msg,
        "explain": d.kind.explain(),
        "note": d.note,
        "file": path.display().to_string(),
        "span": span_json(&span),
        "suggestions": suggestions,
    })
}

/// `maca check [file…] [--json] [--config]`: diagnostics, for a person or for a program.
pub fn cmd_check(args: &[String]) {
    let json_out = args.iter().any(|a| a == "--json");
    let config = args.iter().any(|a| a == "--config");
    let files: Vec<&String> = args.iter().filter(|a| !a.starts_with("--")).collect();
    if files.is_empty() {
        eprintln!("usage: maca check <file.maca>… [--json] [--config]");
        std::process::exit(2);
    }

    let mut all = Vec::new();
    let mut failed = false;
    for file in &files {
        let path = Path::new(file.as_str());
        match diagnose(path, config) {
            Ok((src, diags)) => {
                for d in &diags {
                    if json_out {
                        all.push(diag_json(path, &src, d));
                    } else {
                        let span = maca_core::resolve_span(&src, d);
                        println!(
                            "{}:{}:{}: {} [{}] {}",
                            path.display(),
                            span.start_pos.line,
                            span.start_pos.column,
                            d.kind.severity().as_str(),
                            d.kind.code(),
                            d.msg
                        );
                    }
                    failed = true;
                }
            }
            Err(e) => {
                eprintln!("maca check: {e}");
                failed = true;
            }
        }
    }

    if json_out {
        let out = json!({ "format": FORMAT, "diagnostics": all });
        println!("{}", serde_json::to_string_pretty(&out).unwrap_or_default());
    }
    if failed {
        std::process::exit(1);
    }
}

/// Apply every suggestion that is safe to apply without a human reading it.
///
/// Edits are sorted by start and applied from the back, so an earlier edit
/// does not move the offsets a later one was measured against. Overlapping
/// edits are dropped rather than layered: two rewrites of the same bytes cannot
/// both be right, and guessing which wins is how a fixer corrupts a file.
fn apply(src: &str, edits: &mut [(usize, usize, String)]) -> (String, usize) {
    edits.sort_by_key(|(start, _, _)| *start);
    let mut out = src.to_string();
    let mut applied = 0;
    let mut last_start = usize::MAX;
    for (start, end, text) in edits.iter().rev() {
        if *end > last_start || *end > out.len() || start > end {
            continue;
        }
        out.replace_range(*start..*end, text);
        last_start = *start;
        applied += 1;
    }
    (out, applied)
}

/// `maca fix [file…]`: apply the machine-applicable suggestions in place.
pub fn cmd_fix(args: &[String]) {
    let dry = args.iter().any(|a| a == "--dry-run");
    let config = args.iter().any(|a| a == "--config");
    let files: Vec<&String> = args.iter().filter(|a| !a.starts_with("--")).collect();
    if files.is_empty() {
        eprintln!("usage: maca fix <file.maca>… [--dry-run] [--config]");
        std::process::exit(2);
    }

    for file in files {
        let path = Path::new(file.as_str());
        let Ok((src, diags)) = diagnose(path, config) else {
            eprintln!("maca fix: cannot read {}", path.display());
            std::process::exit(1);
        };
        let mut edits: Vec<(usize, usize, String)> = Vec::new();
        let mut offered = 0;
        for d in &diags {
            let span = maca_core::resolve_span(&src, d);
            for s in &d.suggestions {
                if s.applicability == Applicability::MachineApplicable {
                    let (start, end) = s.span.unwrap_or((span.start, span.end));
                    edits.push((start, end, s.replacement.clone()));
                } else {
                    offered += 1;
                }
            }
        }
        let (fixed, applied) = apply(&src, &mut edits);
        if applied > 0
            && !dry
            && let Err(e) = std::fs::write(path, &fixed)
        {
            eprintln!("maca fix: {}: {e}", path.display());
            std::process::exit(1);
        }
        let what = if dry { "would apply" } else { "applied" };
        println!(
            "{}: {what} {applied}, left {offered} that need reading",
            path.display()
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use maca_core::{DiagKind, Suggestion};

    fn suggestion(text: &str, at: (usize, usize), how: Applicability) -> Suggestion {
        Suggestion {
            message: "try this".into(),
            span: Some(at),
            replacement: text.into(),
            applicability: how,
        }
    }

    /// Applying from the back is what keeps the second edit's offsets meaningful.
    #[test]
    fn two_edits_do_not_move_each_other() {
        let src = "aaa bbb ccc";
        let mut edits = vec![(0, 3, "X".to_string()), (8, 11, "Z".to_string())];
        let (out, n) = apply(src, &mut edits);
        assert_eq!(out, "X bbb Z");
        assert_eq!(n, 2);
    }

    /// Two rewrites of the same bytes cannot both be right, so neither is layered onto the other.
    #[test]
    fn overlapping_edits_do_not_both_land() {
        let src = "aaa bbb";
        let mut edits = vec![(0, 5, "X".to_string()), (3, 7, "Y".to_string())];
        let (out, n) = apply(src, &mut edits);
        assert_eq!(n, 1, "one of the two is dropped, not merged");
        assert!(
            out == "aaaY" || out == "Xbb",
            "whichever landed is intact: {out}"
        );
    }

    #[test]
    fn an_edit_past_the_end_is_refused_rather_than_panicking() {
        let mut edits = vec![(0, 99, "X".to_string())];
        let (out, n) = apply("short", &mut edits);
        assert_eq!((out.as_str(), n), ("short", 0));
    }

    /// `maca fix` is only allowed to touch what is safe without a human reading it.
    #[test]
    fn only_machine_applicable_suggestions_are_applied() {
        let src = "value = 1\n";
        let d = Diagnostic::new(DiagKind::Immutable, "`value` is constant")
            .with_anchor("value")
            .with_suggestion(suggestion("other", (0, 5), Applicability::MaybeIncorrect));
        let mut edits: Vec<(usize, usize, String)> = d
            .suggestions
            .iter()
            .filter(|s| s.applicability == Applicability::MachineApplicable)
            .map(|s| {
                let (a, b) = s.span.unwrap();
                (a, b, s.replacement.clone())
            })
            .collect();
        let (out, n) = apply(src, &mut edits);
        assert_eq!(
            (out.as_str(), n),
            (src, 0),
            "a maybe-incorrect fix is offered, never applied"
        );
    }

    #[test]
    fn a_diagnostic_carries_its_code_span_and_applicability() {
        let src = "main() -> int {\n    nope()\n    0\n}\n";
        let d = Diagnostic::new(DiagKind::UndefinedName, "`nope` is not defined")
            .with_anchor("nope")
            .with_note("define it, or import the module that does")
            .with_suggestion(suggestion("info", (20, 24), Applicability::MaybeIncorrect));
        let v = diag_json(Path::new("a.maca"), src, &d);

        assert_eq!(v["code"], "M0006");
        assert_eq!(v["severity"], "error");
        assert_eq!(v["span"]["start_line"], 2);
        assert_eq!(v["note"], "define it, or import the module that does");
        assert_eq!(v["suggestions"][0]["applicability"], "maybe-incorrect");
        assert!(v["explain"].as_str().is_some_and(|s| !s.is_empty()));
    }
}
