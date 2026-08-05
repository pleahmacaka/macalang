use maca_backend_js::{css_escape, order, rule};

#[test]
fn variants_wrap_the_rule() {
    assert_eq!(
        rule("hover:bg-white").unwrap(),
        ".hover\\:bg-white:hover { background-color:#ffffff; }"
    );
    assert_eq!(
        rule("first:mt-0").unwrap(),
        ".first\\:mt-0:first-child { margin-top:0; }"
    );
    assert!(rule("after:content-none").unwrap().contains("::after"));
    assert!(rule("marker:text-black").unwrap().contains("::marker"));
    assert!(rule("open:font-bold").unwrap().contains("[open]"));
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

/// Without arbitrary values the system is a fixed menu, and anything off the scale sends you back to writing CSS by hand.
#[test]
fn arbitrary_values_work() {
    assert!(rule("text-[0.9em]").unwrap().contains("font-size:0.9em"));
    assert!(rule("max-w-[42rem]").unwrap().contains("max-width:42rem"));
    assert!(rule("dark:bg-[#191919]").unwrap().contains("#191919"));
    assert!(
        rule("grid-cols-[16rem_minmax(0,44rem)]")
            .unwrap()
            .contains("grid-template-columns:16rem minmax(0,44rem)"),
    );
}

/// An unescaped selector doesn't warn: the browser drops the whole rule.
#[test]
fn selectors_escape_every_special_character() {
    let r = rule("text-[0.9em]").unwrap();
    assert!(r.starts_with(".text-\\[0\\.9em\\]"), "{r}");
    let r = rule("grid-cols-[16rem_minmax(0,1fr)]").unwrap();
    assert!(r.contains("\\(0\\,1fr\\)"), "{r}");
    assert_eq!(css_escape("dark:bg-[#191919]"), "dark\\:bg-\\[\\#191919\\]");
}

/// CSS breaks ties by source order, so a variant must be emitted after the plain utility it overrides.
#[test]
fn plain_utilities_sort_before_the_variants_that_override_them() {
    assert!(order("grid") < order("max-md:block"));
    assert!(order("bg-white") < order("dark:bg-black"));
    assert!(order("block") < order("md:flex"));
    assert!(order("max-lg:block") < order("max-sm:block"));
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

/// Utilities a document needs that an app doesn't: the set that was missing.
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
    assert!(rule("max-w-2xl").unwrap().contains("42rem"));
    assert!(rule("underline-offset-2").unwrap().contains("2px"));
}

/// A border width with no style is invisible, because CSS defaults `border-style` to `none`.
#[test]
fn every_border_spelling_carries_its_style() {
    for (class, sides) in [
        ("border-l", &["left"] as &[&str]),
        ("border-x", &["left", "right"]),
        ("border-y", &["top", "bottom"]),
        ("border-l-2", &["left"]),
        ("border-t-4", &["top"]),
        ("border-r-2", &["right"]),
        ("border-b-8", &["bottom"]),
        ("border-x-2", &["left", "right"]),
        ("border-y-4", &["top", "bottom"]),
        ("border-x-[3px]", &["left", "right"]),
        ("border-y-[0.5rem]", &["top", "bottom"]),
    ] {
        let css =
            maca_backend_js::rule(class).unwrap_or_else(|| panic!("{class} generates no rule"));
        for side in sides {
            assert!(
                css.contains(&format!("border-{side}-style:solid"))
                    && css.contains(&format!("border-{side}-width:")),
                "{class} is missing a width or style on {side}: {css}"
            );
        }
    }
}

/// Every utility the front page and the API reference reach for.
#[test]
fn the_utilities_the_site_uses_all_generate_rules() {
    for class in [
        "scroll-mt-6",
        "scroll-mt-[7rem]",
        "scroll-mb-2",
        "break-keep",
        "px-[3px]",
        "py-[0.05rem]",
        "mx-[2px]",
        "my-[2px]",
        "pt-[1px]",
        "pb-[1px]",
        "pl-[1px]",
        "pr-[1px]",
        "ml-[1px]",
        "mr-[1px]",
        "gap-x-[4px]",
        "gap-y-[4px]",
        "left-[-9999px]",
        "right-[2px]",
        "bottom-[2px]",
        "inset-[18px]",
        "focus:left-2",
        "focus:z-10",
        "focus:underline",
        "min-w-0",
        "text-right",
        "grid-cols-2",
    ] {
        assert!(
            maca_backend_js::rule(class).is_some(),
            "{class} generates no rule, so it is silently dead on the page"
        );
    }
}

/// `font-mono` must not lead with a proportional family.
#[test]
fn the_monospace_stack_is_monospace() {
    let css = maca_backend_js::rule("font-mono").unwrap();
    assert!(
        !css.contains("Pretendard"),
        "font-mono leads with a proportional family: {css}"
    );
    assert!(css.contains("ui-monospace"));
    assert!(
        maca_backend_js::rule("font-sans")
            .unwrap()
            .contains("Pretendard"),
        "font-sans should still prefer Pretendard"
    );
}

/// `apps/build_site/build_site.maca` reimplements this escaper in Maca, because it has to build the same selector the stylesheet contains in order to ask whether a class has a rule.
#[test]
fn the_escaper_matches_the_one_build_site_reimplements() {
    for (class, want) in [
        ("max-w-[64rem]", r"max-w-\[64rem\]"),
        ("dark:bg-zinc-950", r"dark\:bg-zinc-950"),
        ("py-[0.05rem]", r"py-\[0\.05rem\]"),
        ("w-1/2", r"w-1\/2"),
        ("p-[1px_2px]", r"p-\[1px_2px\]"),
        ("border-l-2", "border-l-2"),
    ] {
        assert_eq!(css_escape(class), want, "escaping {class}");
    }

    for ch in "/.:[](),#%'\"!$&*+;<=>?@^`{|}~".chars() {
        let one = ch.to_string();
        assert_eq!(
            css_escape(&one),
            format!("\\{ch}"),
            "{ch:?} should be escaped"
        );
    }
    for ch in "abzAZ09-_".chars() {
        let one = ch.to_string();
        assert_eq!(css_escape(&one), one, "{ch:?} should not be escaped");
    }
}
