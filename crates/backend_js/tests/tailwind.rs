//! The utility engine: variants, arbitrary values, and rule ordering.
//!
//! Without variants a utility system can only express the unconditional case,
//! so anything with a theme, a hover state or a breakpoint falls back to
//! hand-written CSS — which is the thing utilities exist to replace. These are
//! the cases that kept `apps/tomo` on a raw `<style>` block.

use maca_backend_js::{css_escape, order, rule};

#[test]
fn variants_wrap_the_rule() {
    // state variants extend the selector
    assert_eq!(
        rule("hover:bg-white").unwrap(),
        ".hover\\:bg-white:hover { background-color:#ffffff; }"
    );
    assert_eq!(
        rule("first:mt-0").unwrap(),
        ".first\\:mt-0:first-child { margin-top:0; }"
    );
    // pseudo-elements
    assert!(rule("after:content-none").unwrap().contains("::after"));
    assert!(rule("marker:text-black").unwrap().contains("::marker"));
    // `[open]`, for a <details>
    assert!(rule("open:font-bold").unwrap().contains("[open]"));
    // media variants wrap it
    assert!(
        rule("dark:bg-black")
            .unwrap()
            .starts_with("@media(prefers-color-scheme:dark)")
    );
    assert!(
        rule("md:flex")
            .unwrap()
            .starts_with("@media(min-width:48rem)")
    );
    assert!(
        rule("max-md:block")
            .unwrap()
            .starts_with("@media(max-width:48rem)")
    );
}

#[test]
fn variants_chain() {
    let r = rule("dark:hover:bg-white").unwrap();
    assert!(r.starts_with("@media(prefers-color-scheme:dark)"), "{r}");
    assert!(r.contains(":hover"), "{r}");
}

/// Without arbitrary values the system is a fixed menu, and anything off the
/// scale sends you back to writing CSS by hand.
#[test]
fn arbitrary_values_work() {
    assert!(rule("text-[0.9em]").unwrap().contains("font-size:0.9em"));
    assert!(rule("max-w-[42rem]").unwrap().contains("max-width:42rem"));
    assert!(rule("dark:bg-[#191919]").unwrap().contains("#191919"));
    // underscores become spaces, so a multi-part value fits in an attribute
    assert!(
        rule("grid-cols-[16rem_minmax(0,44rem)]")
            .unwrap()
            .contains("grid-template-columns:16rem minmax(0,44rem)"),
    );
}

/// An unescaped selector doesn't warn — the browser drops the whole rule.
#[test]
fn selectors_escape_every_special_character() {
    let r = rule("text-[0.9em]").unwrap();
    assert!(r.starts_with(".text-\\[0\\.9em\\]"), "{r}");
    let r = rule("grid-cols-[16rem_minmax(0,1fr)]").unwrap();
    assert!(r.contains("\\(0\\,1fr\\)"), "{r}");
    assert_eq!(css_escape("dark:bg-[#191919]"), "dark\\:bg-\\[\\#191919\\]");
}

/// CSS breaks ties by source order, so a variant must be emitted after the
/// plain utility it overrides. Getting this wrong made `max-md:block` lose to
/// `grid` and the narrow layout silently never applied.
#[test]
fn plain_utilities_sort_before_the_variants_that_override_them() {
    assert!(order("grid") < order("max-md:block"));
    assert!(order("bg-white") < order("dark:bg-black"));
    assert!(order("block") < order("md:flex"));
    // a smaller max-width query is the more specific one, so it comes later
    assert!(order("max-lg:block") < order("max-sm:block"));
    // and a larger min-width query is the more specific one
    assert!(order("sm:flex") < order("lg:flex"));
}

#[test]
fn unknown_classes_are_dropped_not_guessed() {
    assert!(rule("not-a-utility").is_none());
    assert!(rule("wrap").is_none());
    assert!(
        rule("bogus:flex").is_none(),
        "an unknown variant must not pass"
    );
}

/// Utilities a document needs that an app doesn't — the set that was missing.
#[test]
fn document_utilities_exist() {
    for c in [
        "list-none",
        "border-collapse",
        "align-top",
        "font-serif",
        "no-underline",
        "underline-offset-2",
        "leading-relaxed",
        "max-w-2xl",
        "mx-auto",
        "sticky",
        "top-0",
        "shrink-0",
        "shadow-lg",
        "overscroll-contain",
    ] {
        assert!(rule(c).is_some(), "`{c}` should be a utility");
    }
    // the reading measure and a 2px underline offset are exact, not approximate
    assert!(rule("max-w-2xl").unwrap().contains("42rem"));
    assert!(rule("underline-offset-2").unwrap().contains("2px"));
}
