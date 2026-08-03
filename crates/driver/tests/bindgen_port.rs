//! `tools/bindgen.maca`: the Maca port of the stage-0 Rust bindgen.
//!
//! The point of the port is that Maca's own tooling should be written in Maca.
//! The point of this test is that the port must stay *equivalent*: it runs the
//! Maca implementation and the Rust one over the same header and requires the
//! generated declarations to match exactly. A divergence in either direction
//! fails here rather than silently producing different FFI bindings.

mod common;
use common::*;

use std::path::PathBuf;
use std::process::Command;

/// The declaration lines only, dropping the generated banner (which names the
/// header) and blank lines, so the two implementations are compared on the
/// output that actually matters.
fn decls(text: &str) -> Vec<String> {
    text.lines()
        .map(str::trim_end)
        .filter(|l| !l.is_empty() && !l.starts_with("//") && !l.starts_with("import c"))
        .map(str::to_string)
        .collect()
}

#[test]
fn maca_bindgen_matches_the_rust_implementation() {
    if have_wsl() || !have("cc") {
        eprintln!("skipping bindgen port test: needs a host cc and no wsl");
        return;
    }
    let repo = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let dir = std::env::temp_dir().join("maca-bindgen-port");
    let _ = std::fs::create_dir_all(&dir);

    // the same header the Maca tool embeds as its sample
    let header = dir.join("sqlite3.h");
    std::fs::write(
        &header,
        "/* a sample C header */\n\
         #include <stddef.h>\n\
         typedef struct sqlite3 sqlite3;\n\
         const char* sqlite3_libversion(void);\n\
         int sqlite3_open(const char *filename, sqlite3 **ppDb);\n\
         double sqlite3_column_double(sqlite3_stmt*, int iCol);\n\
         int sqlite3_close(sqlite3*);\n",
    )
    .unwrap();

    // stage-0 (Rust)
    let rust = Command::new(env!("CARGO_BIN_EXE_maca"))
        .args(["bindgen", &header.to_string_lossy()])
        .output()
        .expect("spawn maca bindgen");
    assert!(rust.status.success(), "rust bindgen failed");
    let rust_decls = decls(&String::from_utf8_lossy(&rust.stdout));

    // the Maca port, reading the same header off disk
    let maca = Command::new(env!("CARGO_BIN_EXE_maca"))
        .args([
            "run",
            &repo.join("tools/bindgen.maca").to_string_lossy(),
            &header.to_string_lossy(),
        ])
        .output()
        .expect("spawn maca run tools/bindgen.maca");
    assert!(
        maca.status.success(),
        "maca bindgen failed:\n{}",
        String::from_utf8_lossy(&maca.stderr)
    );
    let maca_decls = decls(&String::from_utf8_lossy(&maca.stdout));

    assert_eq!(
        rust_decls, maca_decls,
        "the Maca port and the Rust implementation disagree"
    );

    // and the declarations are actually right, not merely equal
    assert_eq!(
        rust_decls,
        vec![
            "sqlite3_libversion() -> str",
            "sqlite3_open(filename: str, ppDb: int) -> int",
            "sqlite3_column_double(sqlite3_stmt: int, iCol: int) -> float",
            "sqlite3_close(sqlite3: int) -> int",
        ],
        "bindgen output changed"
    );
}

/// The port is a usable CLI, not just a library: it writes to a file when given
/// a second argument, and reports a missing header rather than emitting an
/// empty module.
#[test]
fn maca_bindgen_cli_writes_and_reports_errors() {
    if have_wsl() || !have("cc") {
        eprintln!("skipping bindgen CLI test: needs a host cc and no wsl");
        return;
    }
    let repo = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let tool = repo.join("tools/bindgen.maca");
    let dir = std::env::temp_dir().join("maca-bindgen-cli");
    let _ = std::fs::create_dir_all(&dir);

    let header = dir.join("api.h");
    std::fs::write(&header, "int api_open(const char *path);\n").unwrap();
    let out = dir.join("api.maca");
    let _ = std::fs::remove_file(&out);

    let run = Command::new(env!("CARGO_BIN_EXE_maca"))
        .args([
            "run",
            &tool.to_string_lossy(),
            &header.to_string_lossy(),
            &out.to_string_lossy(),
        ])
        .output()
        .expect("spawn bindgen");
    assert!(run.status.success(), "bindgen CLI failed");
    let written = std::fs::read_to_string(&out).expect("output file not written");
    assert!(
        written.contains("api_open(path: str) -> int") && written.contains("import c \"api.h\""),
        "written module wrong:\n{written}"
    );

    // a header that doesn't exist is an error, not an empty module
    let miss = Command::new(env!("CARGO_BIN_EXE_maca"))
        .args([
            "run",
            &tool.to_string_lossy(),
            &dir.join("absent.h").to_string_lossy(),
        ])
        .output()
        .expect("spawn bindgen");
    assert!(
        !miss.status.success(),
        "a missing header should exit non-zero"
    );
    assert!(
        String::from_utf8_lossy(&miss.stderr).contains("cannot read"),
        "missing header should be reported: {}",
        String::from_utf8_lossy(&miss.stderr)
    );
}
