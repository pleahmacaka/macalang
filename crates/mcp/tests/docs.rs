use maca_mcp::check;
use std::fs;
use std::path::PathBuf;

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

/// Every `maca` example in the documentation must pass `maca.check`.
#[test]
fn llm_docs_examples_check_clean() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    for file in ["llms.txt", ".claude/skills/macalang/SKILL.md", "README.md"] {
        let md = fs::read_to_string(root.join(file)).unwrap_or_else(|e| panic!("{file}: {e}"));
        let blocks = maca_blocks(&md);
        assert!(!blocks.is_empty(), "{file}: no ```maca blocks found");
        for (i, b) in blocks.iter().enumerate() {
            let d = check(b, false);
            assert!(
                d.is_empty(),
                "{file} block #{i} failed maca.check: {d:?}\n---\n{b}"
            );
        }
    }
}
