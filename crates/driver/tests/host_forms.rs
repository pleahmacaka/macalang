mod common;
use common::*;

use std::process::Command;

const PAGE: &str = "import { decode } from std/json\n\
                    import { local_start, local_store } from web/storage\n\
                    \n\
                    Site = { title: str }\n\
                    \n\
                    Links = \"links.json\"\n\
                    \n\
                    site: Site = data(Links)\n\
                    locked = stored(\"page.locked\", true)\n\
                    \n\
                    main() -> str => site.title\n";

fn scratch(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join("maca-host-forms").join(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("scratch dir");
    dir
}

fn check(file: &std::path::Path) -> (bool, String) {
    let out = Command::new(maca())
        .args(["check", &file.to_string_lossy()])
        .output()
        .expect("spawn maca check");
    (
        out.status.success(),
        format!(
            "{}{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        ),
    )
}

/// `data` and `stored` are rewritten before a program is compiled, so the call that builds must not be a defect to the call that checks.
#[test]
fn check_accepts_the_two_forms_the_driver_rewrites() {
    let dir = scratch("clean");
    std::fs::write(dir.join("links.json"), "{\"title\": \"home\"}\n").expect("write data");
    let page = dir.join("page.maca");
    std::fs::write(&page, PAGE).expect("write page");

    let (ok, said) = check(&page);
    assert!(ok, "the page checks clean:\n{said}");
    assert!(
        !said.contains("M0006"),
        "no name is undefined here:\n{said}"
    );
}

/// The file is read while checking, exactly as it is while building, so a path that is not there is named rather than believed.
#[test]
fn check_names_the_file_data_cannot_read() {
    let dir = scratch("missing");
    let page = dir.join("page.maca");
    std::fs::write(&page, PAGE).expect("write page");

    let (ok, said) = check(&page);
    assert!(!ok, "a missing file is a failure:\n{said}");
    assert!(
        said.contains("links.json"),
        "the message names the file it could not read:\n{said}"
    );
}

/// An editor holds no filesystem, so the checker has to know the names without reading anything.
#[test]
fn the_forms_are_not_undefined_names_to_the_checker() {
    let parsed = maca_parser::parse(PAGE);
    assert!(parsed.errors.is_empty(), "{:?}", parsed.errors);
    let diags = maca_core::check(&parsed.module, maca_core::Mode::Program);
    let undefined: Vec<&str> = diags
        .iter()
        .filter(|d| d.kind == maca_core::DiagKind::UndefinedName)
        .map(|d| d.msg.as_str())
        .collect();
    assert!(undefined.is_empty(), "{undefined:?}");
}
