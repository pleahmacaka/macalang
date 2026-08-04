use std::process::Command;

const HEADER: &str = r#"
/* sample */
#ifndef MYLIB_H
#define MYLIB_H
#include <stddef.h>

// open a database
int mylib_open(const char* path);
void mylib_close(int handle);
const char* mylib_version(void);
double mylib_scale(double x, int factor);
int mylib_exec(int db, const char* sql, size_t len);
typedef struct mylib_ctx mylib_ctx;
struct mylib_ctx { int x; };
#endif
"#;

#[test]
fn generates_bindings_from_a_header() {
    let dir = std::env::temp_dir().join("maca-bindgen-test");
    std::fs::create_dir_all(&dir).unwrap();
    let header = dir.join("mylib.h");
    std::fs::write(&header, HEADER).unwrap();

    let out = Command::new(env!("CARGO_BIN_EXE_maca"))
        .args(["bindgen", &header.to_string_lossy()])
        .output()
        .expect("spawn maca bindgen");
    assert!(out.status.success(), "bindgen failed");
    let maca = String::from_utf8_lossy(&out.stdout);

    for want in [
        "import c \"mylib.h\"",
        "mylib_open(path: str) -> int",
        "mylib_close(handle: int) -> int",
        "mylib_version() -> str",
        "mylib_scale(x: float, factor: int) -> float",
        "mylib_exec(db: int, sql: str, len: int) -> int",
    ] {
        assert!(maca.contains(want), "missing {want:?}:\n{maca}");
    }
    assert!(
        !maca.contains("mylib_ctx("),
        "struct/typedef leaked into output:\n{maca}"
    );
}

#[test]
fn generated_bindings_parse_and_typecheck() {
    let dir = std::env::temp_dir().join("maca-bindgen-test");
    std::fs::create_dir_all(&dir).unwrap();
    let header = dir.join("mylib.h");
    std::fs::write(&header, HEADER).unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_maca"))
        .args(["bindgen", &header.to_string_lossy()])
        .output()
        .expect("spawn maca bindgen");
    let mut maca = String::from_utf8_lossy(&out.stdout).to_string();
    maca.push_str("\nmain() -> int {\n    info(mylib_version())\n    mylib_close(0)\n    0\n}\n");

    let parsed = maca_parser::parse(&maca);
    assert!(
        parsed.errors.is_empty(),
        "generated bindings don't parse: {:?}\n{maca}",
        parsed.errors
    );
    let diags = maca_core::check(&parsed.module, maca_core::Mode::Program);
    assert!(
        diags.is_empty(),
        "generated bindings don't type-check: {diags:?}\n{maca}"
    );
}
