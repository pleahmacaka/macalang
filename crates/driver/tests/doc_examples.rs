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

/// A block is an excerpt, not a program.
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
    for file in ["llms.txt", ".claude/skills/macalang/SKILL.md", "README.md"] {
        let md =
            std::fs::read_to_string(repo().join(file)).unwrap_or_else(|e| panic!("{file}: {e}"));

        for (i, block) in maca_blocks(&md).iter().enumerate() {
            if !imports_a_package(block) {
                continue;
            }
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
        "no documented example imports a package: either the docs stopped \
         showing one or the detection above stopped recognising it"
    );
}
