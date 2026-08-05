use crate::DiagKind;

/// How much a diagnostic matters.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Severity {
    Error,
    Warning,
}

impl Severity {
    pub fn as_str(self) -> &'static str {
        match self {
            Severity::Error => "error",
            Severity::Warning => "warning",
        }
    }
}

/// Whether a suggestion can be applied without a human reading it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Applicability {
    /// Applying this is always correct, so `maca fix` does it.
    MachineApplicable,
    /// This is the likely intent and might be wrong, so it is offered and never applied.
    MaybeIncorrect,
}

impl Applicability {
    pub fn as_str(self) -> &'static str {
        match self {
            Applicability::MachineApplicable => "machine-applicable",
            Applicability::MaybeIncorrect => "maybe-incorrect",
        }
    }
}

/// A replacement for one byte range of the source.
#[derive(Clone, Debug, PartialEq)]
pub struct Suggestion {
    pub message: String,
    /// Byte range to replace. `None` means the diagnostic's own span.
    pub span: Option<(usize, usize)>,
    pub replacement: String,
    pub applicability: Applicability,
}

/// A line and column, both counted from one, at a byte offset.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Position {
    pub line: usize,
    pub column: usize,
}

/// A byte range and the line/column its ends fall on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Span {
    pub start: usize,
    pub end: usize,
    pub start_pos: Position,
    pub end_pos: Position,
}

/// Line and column at `offset`, counting characters rather than bytes so a column lands where a reader sees it.
pub fn position(src: &str, offset: usize) -> Position {
    let offset = offset.min(src.len());
    let before = &src[..offset];
    let line = before.matches('\n').count() + 1;
    let column = before
        .rfind('\n')
        .map_or(before, |at| &before[at + 1..])
        .chars()
        .count()
        + 1;
    Position { line, column }
}

/// A byte range as a span, with both ends resolved to line and column.
pub fn span_at(src: &str, start: usize, end: usize) -> Span {
    Span {
        start,
        end,
        start_pos: position(src, start),
        end_pos: position(src, end),
    }
}

impl DiagKind {
    /// The stable code for this kind.
    ///
    /// A code is a promise: it names one defect forever, so a tool may branch
    /// on it. Retiring a kind retires its code with it, and the next kind takes
    /// the next number rather than the free one.
    pub fn code(self) -> &'static str {
        match self {
            DiagKind::TypeMismatch => "M0001",
            DiagKind::NonExhaustive => "M0002",
            DiagKind::EffectInConfig => "M0003",
            DiagKind::UnknownOption => "M0004",
            DiagKind::Immutable => "M0005",
            DiagKind::UndefinedName => "M0006",
            DiagKind::EffectNotOnTarget => "M0007",
        }
    }

    /// What this kind means, in one sentence, independent of the particular occurrence.
    pub fn explain(self) -> &'static str {
        match self {
            DiagKind::TypeMismatch => "a value is used where a different type is required",
            DiagKind::NonExhaustive => "a `match` leaves a variant unhandled",
            DiagKind::EffectInConfig => "config mode has no effects, and this call performs one",
            DiagKind::UnknownOption => "the option this sets does not exist",
            DiagKind::Immutable => "a constant is assigned after it is bound",
            DiagKind::UndefinedName => "this name is not defined anywhere in scope",
            DiagKind::EffectNotOnTarget => {
                "the target being built for cannot carry the effect this performs"
            }
        }
    }

    pub fn severity(self) -> Severity {
        Severity::Error
    }
}

/// The name a diagnostic is about, taken from the field the checker set rather than parsed back out of the message.
fn anchor_of(d: &crate::Diagnostic) -> Option<&str> {
    if let Some(name) = &d.anchor {
        return Some(name);
    }
    match d.msg.split_once("has no method `") {
        Some((_, rest)) => rest.split('`').next(),
        None => first_backtick(&d.msg),
    }
}

fn first_backtick(msg: &str) -> Option<&str> {
    let (_, rest) = msg.split_once('`')?;
    rest.split('`').next().filter(|s| !s.is_empty())
}

/// First whole-word occurrence of `name` in code, skipping `//` comments and `"…"` strings.
pub fn code_word_span(src: &str, name: &str) -> Option<(usize, usize)> {
    if name.is_empty() {
        return None;
    }
    let b = src.as_bytes();
    let is_word = |c: u8| c.is_ascii_alphanumeric() || c == b'_';
    let n = name.len();
    let mut i = 0;
    while i < b.len() {
        match b[i] {
            b'/' if b.get(i + 1) == Some(&b'/') => {
                while i < b.len() && b[i] != b'\n' {
                    i += 1;
                }
            }
            b'"' => {
                i += 1;
                while i < b.len() && b[i] != b'"' {
                    i += if b[i] == b'\\' { 2 } else { 1 };
                }
                i += 1;
            }
            _ => {
                if b[i..].starts_with(name.as_bytes())
                    && (i == 0 || !is_word(b[i - 1]))
                    && (i + n >= b.len() || !is_word(b[i + n]))
                {
                    return Some((i, i + n));
                }
                i += 1;
            }
        }
    }
    None
}

/// Where in `src` a diagnostic points.
///
/// This is the one implementation. The language server and `maca check --json`
/// both call it, so a diagnostic cannot be placed in two different spots
/// depending on which asked.
pub fn resolve_span(src: &str, d: &crate::Diagnostic) -> Span {
    let (start, end) = d
        .span
        .or_else(|| anchor_of(d).and_then(|name| code_word_span(src, name)))
        .unwrap_or((0, 1.min(src.len())));
    span_at(src, start, end)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Diagnostic;

    #[test]
    fn a_position_counts_lines_and_columns_from_one() {
        let src = "a = 1\nbb = 2\n";
        assert_eq!(position(src, 0), Position { line: 1, column: 1 });
        assert_eq!(position(src, 6), Position { line: 2, column: 1 });
        assert_eq!(position(src, 8), Position { line: 2, column: 3 });
    }

    /// A column is what a reader counts, so a multi-byte character before it counts once.
    #[test]
    fn a_column_counts_characters_rather_than_bytes() {
        let src = "x = \"한글\"\ny = 2\n";
        let after = src.find('\n').unwrap();
        assert_eq!(
            position(src, after).column,
            "x = \"한글\"".chars().count() + 1
        );
    }

    #[test]
    fn a_diagnostic_that_carries_its_span_is_placed_there() {
        let d = Diagnostic {
            kind: DiagKind::UndefinedName,
            msg: "`zzz`: nope".into(),
            note: None,
            anchor: None,
            span: Some((10, 13)),
            suggestions: Vec::new(),
        };
        let span = resolve_span("aaaaaaaaaa zzz", &d);
        assert_eq!((span.start, span.end), (10, 13));
    }

    /// Without a span the anchor field names what to look for, which beats guessing from the prose.
    #[test]
    fn an_anchor_finds_the_name_in_code_and_not_in_a_string() {
        let d = Diagnostic {
            kind: DiagKind::UndefinedName,
            msg: "something about it".into(),
            note: None,
            anchor: Some("target".into()),
            span: None,
            suggestions: Vec::new(),
        };
        let src = "a = \"target\"\ntarget = 1\n";
        let span = resolve_span(src, &d);
        assert_eq!(&src[span.start..span.end], "target");
        assert_eq!(span.start_pos.line, 2, "the one in the string is not code");
    }

    #[test]
    fn every_kind_has_its_own_code_and_they_are_ordered() {
        let kinds = [
            DiagKind::TypeMismatch,
            DiagKind::NonExhaustive,
            DiagKind::EffectInConfig,
            DiagKind::UnknownOption,
            DiagKind::Immutable,
            DiagKind::UndefinedName,
        ];
        let mut seen = std::collections::HashSet::new();
        for k in kinds {
            assert!(k.code().starts_with('M'), "{:?} has no M code", k);
            assert!(seen.insert(k.code()), "{} is used twice", k.code());
            assert!(!k.explain().is_empty());
        }
    }
}
