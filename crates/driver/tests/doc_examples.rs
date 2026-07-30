//! The documentation's examples, compiled against the real tree.
//!
//! `crates/mcp/tests/docs.rs` already runs `maca.check` over every ```maca
//! block in `llms.txt`, `SKILL.md` and `README.md`, which catches a block that
//! does not parse or calls something it never defines. It cannot catch a bad
//! *import*: `check` reads source text and never touches the filesystem, so
//! `import { nosuchfn } from http/server` passes it.
//!
//! That is the failure this file exists for. README advertised
//! `import { serve, text } from http/server` — `http/server` defines neither
//! name — and every check in the repository was green, because the example had
//! only ever been verified by pasting it into a file with the missing pieces
//! added. An example is a claim about the packages as they are, and the only
//! way to check that claim is to build it where they live.
//!
//! A block naming no import is left to the `maca.check` pass; building all of
//! them would cost a C compile each for no extra coverage.

mod common;
use common::*;

use std::process::Command;

/// The ```maca blocks in `md`.
fn maca_blocks(md: &str) -> Vec<String> {
    let mut blocks = Vec::new();
    let mut cur = String::new();
    let mut inside = false;
    for line in md.lines() {
        if !inside && line.trim_start().starts_with("```maca") {
            inside = true;
            cur.clear();
        } else if inside && line.trim_start().starts_with("```") {
            inside = false;
            blocks.push(cur.clone());
        } else if inside {
            cur.push_str(line);
            cur.push('\n');
        }
    }
    blocks
}

/// Does this block import something the repository is supposed to provide?
fn imports_a_package(block: &str) -> bool {
    block
        .lines()
        .any(|l| l.trim_start().starts_with("import ") && !l.contains('"'))
}

/// A block is an excerpt, not a program: give it an entry point when it has
/// none, so what is being tested is the import rather than the block's shape.
fn as_program(block: &str) -> String {
    if block.contains("main(") {
        block.to_string()
    } else {
        format!("{block}\nmain() -> int => 0\n")
    }
}

/// Every documented example that imports a package must build against it.
#[test]
fn documented_imports_name_things_that_exist() {
    if have_wsl() || !have("cc") {
        eprintln!("skipping doc example build: needs a host cc and no wsl");
        return;
    }

    let mut built = 0;
    for file in ["llms.txt", "SKILL.md", "README.md"] {
        let md =
            std::fs::read_to_string(repo().join(file)).unwrap_or_else(|e| panic!("{file}: {e}"));

        for (i, block) in maca_blocks(&md).iter().enumerate() {
            if !imports_a_package(block) {
                continue;
            }
            // Written at the repository root, because that is where `modules/`
            // resolves from — the same reason a reader's own project has to be
            // a project.
            let src = repo().join(format!(".maca-doc-{}-{i}.maca", file.replace('.', "-")));
            std::fs::write(&src, as_program(block)).expect("write the block");

            let out = std::env::temp_dir().join(format!("maca-doc-{i}"));
            let result = Command::new(maca())
                .current_dir(repo())
                .args([
                    "build",
                    &src.to_string_lossy(),
                    "-o",
                    &out.to_string_lossy(),
                ])
                .output()
                .expect("spawn maca build");
            let _ = std::fs::remove_file(&src);

            assert!(
                result.status.success(),
                "{file} block #{i} does not build:\n{block}\n{}\n{}",
                String::from_utf8_lossy(&result.stdout),
                String::from_utf8_lossy(&result.stderr),
            );
            built += 1;
        }
    }

    assert!(
        built > 0,
        "no documented example imports a package — either the docs stopped \
         showing one or the detection above stopped recognising it"
    );
}
