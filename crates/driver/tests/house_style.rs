mod common;
use common::*;

use std::fs;
use std::path::{Path, PathBuf};

/// Written as escapes rather than as themselves, because this file is one of the files the rule reads and a table of literals would report itself.
const BANNED: [(char, &str); 3] = [
    ('\u{2014}', "em dash"),
    ('\u{2013}', "en dash"),
    ('\u{00b7}', "middle dot"),
];

/// Directories that are not the repository's own text.
fn is_skipped(name: &str) -> bool {
    matches!(
        name,
        ".git"
            | "target"
            | "node_modules"
            | "site"
            | "maca_modules"
            | ".maca"
            | "fonts"
            | "graphify-out"
    )
}

/// A file whose bytes are not prose.
fn is_text(path: &Path) -> bool {
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
    !matches!(
        ext,
        "png" | "jpg" | "jpeg" | "gif" | "ico" | "wasm" | "woff" | "woff2" | "pdf" | "zip"
    )
}

fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for e in entries.flatten() {
        let path = e.path();
        let name = e.file_name().to_string_lossy().to_string();
        if path.is_dir() {
            if !is_skipped(&name) {
                walk(&path, out);
            }
        } else if is_text(&path) {
            out.push(path);
        }
    }
}

#[test]
fn no_em_dash_en_dash_or_middle_dot_anywhere() {
    let root = repo();
    let mut files = Vec::new();
    walk(&root, &mut files);
    assert!(
        files.len() > 200,
        "the walk found only {} files, so it is not reaching the tree",
        files.len()
    );

    let mut found: Vec<String> = Vec::new();
    for path in &files {
        let Ok(text) = fs::read_to_string(path) else {
            continue;
        };
        for (no, line) in text.lines().enumerate() {
            for (ch, what) in BANNED {
                if line.contains(ch) {
                    let rel = path.strip_prefix(&root).unwrap_or(path);
                    found.push(format!("{}:{}: {what}", rel.display(), no + 1));
                }
            }
        }
    }
    assert!(
        found.is_empty(),
        "write a colon, a comma pair or a full stop instead:\n{}",
        found.join("\n")
    );
}

/// The walk has to actually read files, and a `read_dir` that silently fails would leave the rule passing over an empty list forever.
#[test]
fn the_scan_would_notice_a_dash() {
    let dir = std::env::temp_dir().join(format!("maca_house_style_{}", std::process::id()));
    fs::create_dir_all(&dir).expect("make the probe directory");
    let file = dir.join("prose.md");
    fs::write(&file, "a line with an em dash \u{2014} in it\n").expect("write the probe");

    let mut files = Vec::new();
    walk(&dir, &mut files);
    assert_eq!(files, vec![file.clone()], "the walk found the probe");

    let text = fs::read_to_string(&file).expect("read the probe");
    assert!(
        text.lines().any(|l| l.contains('\u{2014}')),
        "the same test the rule runs did not see the dash it was given"
    );
    fs::remove_dir_all(&dir).ok();
}
