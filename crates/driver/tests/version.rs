use std::path::{Path, PathBuf};

fn repo() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn read(rel: &str) -> String {
    let p = repo().join(rel);
    std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("{}: {e}", p.display()))
}

/// The value of the first `key`-ish line, with quotes stripped.
fn field(src: &str, key: &str) -> String {
    for line in src.lines() {
        let line = line.trim();
        let Some(rest) = line.strip_prefix(key) else {
            continue;
        };
        let rest = rest.trim_start();
        let Some(rest) = rest.strip_prefix(['=', ':']) else {
            continue;
        };
        return rest
            .trim()
            .trim_end_matches(',')
            .trim()
            .trim_matches('"')
            .to_string();
    }
    panic!("no `{key}` line found");
}

/// The newest `## x.y.z` heading in the changelog.
fn newest_release(changelog: &str) -> String {
    for line in changelog.lines() {
        if let Some(rest) = line.strip_prefix("## ") {
            let v = rest.trim();
            if v.chars().next().is_some_and(|c| c.is_ascii_digit()) {
                return v.to_string();
            }
        }
    }
    panic!("no `## <version>` heading in CHANGELOG.md");
}

#[test]
fn every_file_that_states_the_version_agrees() {
    let want = field(&read("Cargo.toml"), "version");
    assert!(
        want.split('.').count() == 3 && want.chars().next().is_some_and(|c| c.is_ascii_digit()),
        "Cargo.toml version is not a bare semver: {want}"
    );

    let others: Vec<(&str, String)> = vec![
        ("maca.toml", field(&read("maca.toml"), "version")),
        (
            "packages/macalang/package.json",
            field(&read("packages/macalang/package.json"), "\"version\""),
        ),
        (
            "editor/zed-maca/extension.toml",
            field(&read("editor/zed-maca/extension.toml"), "version"),
        ),
        (
            "editor/zed-maca/Cargo.toml",
            field(&read("editor/zed-maca/Cargo.toml"), "version"),
        ),
        ("CHANGELOG.md", newest_release(&read("CHANGELOG.md"))),
    ];

    let wrong: Vec<String> = others
        .iter()
        .filter(|(_, got)| *got != want)
        .map(|(f, got)| format!("{f}: {got}"))
        .collect();
    assert!(
        wrong.is_empty(),
        "Cargo.toml says {want}, but:\n  {}",
        wrong.join("\n  ")
    );
}

/// The Zed extension's two files are one release of one thing, and its README documents that pairing.
#[test]
fn the_zed_readme_shows_the_version_it_documents() {
    let want = field(&read("editor/zed-maca/extension.toml"), "version");
    let readme = read("editor/zed-maca/README.md");
    assert!(
        readme.contains(&format!("version = \"{want}\"")),
        "editor/zed-maca/README.md does not show `version = \"{want}\"`"
    );
}

/// A guard on the guard: the reader above has to be able to say "no".
#[test]
fn the_field_reader_reads_the_field() {
    assert_eq!(field("version = \"1.2.3\"", "version"), "1.2.3");
    assert_eq!(field("  \"version\": \"1.2.3\",", "\"version\""), "1.2.3");
    assert_eq!(
        field("name = \"x\"\nversion = \"9.9.9\"", "version"),
        "9.9.9"
    );
    assert_eq!(
        newest_release("# Changelog\n\n## 1.2.3\n\n## 1.2.2\n"),
        "1.2.3"
    );
    assert_eq!(
        newest_release("# Changelog\n\n## Unreleased\n\n## 1.2.3\n"),
        "1.2.3"
    );
}

/// Nothing outside the files above should be hard-coding the version, because a copy nobody compares is a copy that goes stale.
#[test]
fn no_other_source_file_hard_codes_the_version() {
    let want = field(&read("Cargo.toml"), "version");
    let mut found = Vec::new();
    walk(&repo().join("crates"), &mut found, &want);
    found.retain(|p| !p.ends_with("build_cache.rs") && !p.ends_with("version.rs"));
    assert!(found.is_empty(), "version literal in: {found:?}");
}

fn walk(dir: &Path, out: &mut Vec<String>, want: &str) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for e in entries.flatten() {
        let p = e.path();
        if p.is_dir() {
            walk(&p, out, want);
        } else if p.extension().is_some_and(|x| x == "rs")
            && std::fs::read_to_string(&p).is_ok_and(|s| s.contains(&format!("\"{want}\"")))
        {
            out.push(p.display().to_string());
        }
    }
}
