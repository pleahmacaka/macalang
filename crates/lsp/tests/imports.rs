/// The forms a page writes: an asset named by its extension, and named bindings out of a package.
#[test]
fn the_new_import_forms_are_clean_and_bind_their_names() {
    let src = "import \"npm:daisyui/dist/full.css\"\n\
               import \"theme.css\"\n\
               import { iconify_icon, pick_text } from \"npm:some-pkg\"\n\
               \n\
               main() -> int {\n\
               \x20   iconify_icon()\n\
               \x20   pick_text()\n\
               \x20   0\n\
               }\n";

    let diags = maca_lsp::diagnostics_located(src, false);
    assert!(
        diags.is_empty(),
        "the new import forms should be clean: {:?}",
        diags.iter().map(|d| &d.message).collect::<Vec<_>>()
    );
}

/// A string import that says nothing about what it is has to be refused rather than guessed at.
#[test]
fn an_asset_with_no_recognisable_extension_is_refused() {
    let diags = maca_lsp::diagnostics_located("import \"npm:iconify-icon\"\n", false);
    assert!(!diags.is_empty(), "a bare package name is not an asset");
    assert!(
        diags[0].message.contains("name what you want from it"),
        "and the message should say what to write instead: {}",
        diags[0].message
    );
}

/// A name a package does not bind is still undefined, or the form would launder typos.
#[test]
fn a_name_the_import_does_not_list_is_still_undefined() {
    let src = "import { one } from \"npm:pkg\"\n\
               \n\
               main() -> int {\n\
               \x20   two()\n\
               \x20   0\n\
               }\n";

    let diags = maca_lsp::diagnostics_located(src, false);
    assert!(
        diags.iter().any(|d| d.message.contains("two")),
        "`two` was never imported: {:?}",
        diags.iter().map(|d| &d.message).collect::<Vec<_>>()
    );
}

/// The printer round-trips both forms, so `maca fmt` does not rewrite them into the old spelling.
#[test]
fn formatting_keeps_the_form_that_was_written() {
    for line in ["import \"theme.css\"", "import { a, b } from \"npm:pkg\""] {
        let src = format!("{line}\n\nmain() -> int {{\n    0\n}}\n");
        let printed = maca_parser::print_module(&maca_parser::parse(&src).module);
        assert!(printed.contains(line), "`{line}` came back as:\n{printed}");
    }
}

/// A page's `data` and `stored` have no definition to find, and an editor holding no filesystem must still not call them undefined.
#[test]
fn the_host_forms_are_known_to_the_editor() {
    let src = "import { decode } from std/json\n\
               import { local_start, local_store } from web/storage\n\
               \n\
               Site = { title: str }\n\
               \n\
               site: Site = data(\"links.json\")\n\
               locked = stored(\"page.locked\", true)\n\
               \n\
               main() -> str => site.title\n";

    let diags = maca_lsp::diagnostics_located(src, false);
    assert!(
        diags.is_empty(),
        "a page that reads a file and a stored slot is clean: {:?}",
        diags.iter().map(|d| &d.message).collect::<Vec<_>>()
    );
}
