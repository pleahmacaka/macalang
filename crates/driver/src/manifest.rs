use maca_parser::modules::{MANIFEST, declares_workspace, manifest_chain, workspace_root};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

/// One manifest on a chain: the directory it governs and what it says.
pub struct Entry {
    pub dir: PathBuf,
    pub text: String,
}

impl Entry {
    /// The manifest file itself, for a message that has to name it.
    pub fn file(&self) -> PathBuf {
        self.dir.join(MANIFEST)
    }
}

/// A `[[bin]]` block: what to build when no file is named.
pub struct Bin {
    pub name: String,
    pub path: PathBuf,
}

/// The manifests that answer for one path, nearest first, ending at the workspace root.
pub struct Chain {
    entries: Vec<Entry>,
}

impl Chain {
    /// The chain covering a source file.
    pub fn for_source(src: &Path) -> Chain {
        let dir = src
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .map(Path::to_path_buf)
            .unwrap_or_else(|| PathBuf::from("."));
        Chain::for_dir(&dir)
    }

    /// The chain covering the directory the command was run in.
    pub fn here() -> Chain {
        Chain::for_dir(Path::new("."))
    }

    pub fn for_dir(dir: &Path) -> Chain {
        let entries = manifest_chain(dir)
            .into_iter()
            .filter_map(|dir| {
                let text = std::fs::read_to_string(dir.join(MANIFEST)).ok()?;
                Some(Entry { dir, text })
            })
            .collect();
        Chain { entries }
    }

    /// The manifest of the directory itself, which is where identity is written.
    pub fn own(&self) -> Option<&Entry> {
        self.entries.first()
    }

    /// The workspace root's manifest, which is where the members are listed.
    pub fn root(&self) -> Option<&Entry> {
        self.entries.last()
    }

    /// What this package calls itself, or the name of the directory it sits in.
    pub fn package_name(&self) -> String {
        self.value("[package]", "name")
            .map(|(_, v)| unquote(&v).to_string())
            .or_else(|| {
                let dir = self.own()?.dir.canonicalize().ok()?;
                Some(dir.file_name()?.to_string_lossy().into_owned())
            })
            .unwrap_or_else(|| "this package".to_string())
    }

    /// What this package calls its release, which a workspace member inherits from the root.
    pub fn package_version(&self) -> Option<String> {
        self.value("[package]", "version")
            .map(|(_, v)| unquote(&v).to_string())
    }

    /// Every key of one table, the nearest manifest that states a key answering for it.
    pub fn table(&self, name: &str) -> Vec<(String, String)> {
        let mut out: Vec<(String, String)> = Vec::new();
        for entry in &self.entries {
            for (k, v) in table_of(&entry.text, name) {
                if !out.iter().any(|(seen, _)| seen == &k) {
                    out.push((k, v));
                }
            }
        }
        out
    }

    /// One key, and the directory of the manifest that answered, because a path in a manifest is relative to it.
    pub fn value(&self, table: &str, key: &str) -> Option<(&Path, String)> {
        self.entries.iter().find_map(|e| {
            table_of(&e.text, table)
                .into_iter()
                .find(|(k, _)| k == key)
                .map(|(_, v)| (e.dir.as_path(), v))
        })
    }

    /// The `[[bin]]` blocks of the directory's own manifest, with their paths resolved against it.
    pub fn bins(&self) -> Vec<Bin> {
        let Some(entry) = self.own() else {
            return Vec::new();
        };
        blocks_of(&entry.text, "[[bin]]")
            .into_iter()
            .filter_map(|kv| {
                let get = |k: &str| {
                    kv.iter()
                        .find(|(key, _)| key == k)
                        .map(|(_, v)| unquote(v).to_string())
                };
                let path = get("path")?;
                let name = get("name").unwrap_or_else(|| path.clone());
                Some(Bin {
                    name,
                    path: entry.dir.join(path),
                })
            })
            .collect()
    }

    /// What the workspace root says is a member, against what the tree holds.
    pub fn check_workspace(&self) -> Result<(), String> {
        let Some(root) = self.root() else {
            return Ok(());
        };
        for entry in &self.entries {
            if entry.dir != root.dir && declares_workspace(&entry.text) {
                return Err(format!(
                    "{}: [workspace] inside a workspace; only {} declares one",
                    entry.file().display(),
                    root.file().display()
                ));
            }
        }
        check_members(&root.dir, &root.text)
    }
}

/// The members the root lists, against the directories that hold a manifest beside them.
pub fn check_members(root: &Path, text: &str) -> Result<(), String> {
    let members = array_of(text, "[workspace]", "members");
    if members.is_empty() {
        return Ok(());
    }
    for m in &members {
        let file = root.join(m).join(MANIFEST);
        let Ok(text) = std::fs::read_to_string(&file) else {
            return Err(format!(
                "{}: [workspace] member `{m}` has no {MANIFEST}",
                root.join(MANIFEST).display()
            ));
        };
        if !table_of(&text, "[package]")
            .iter()
            .any(|(k, _)| k == "name")
        {
            return Err(format!(
                "{}: a workspace member states its own [package] name",
                file.display()
            ));
        }
    }
    let listed: BTreeSet<&str> = members.iter().map(String::as_str).collect();
    let mut parents: BTreeSet<PathBuf> = BTreeSet::new();
    for m in &members {
        parents.insert(Path::new(m).parent().unwrap_or(Path::new("")).to_path_buf());
    }
    for parent in parents {
        let Ok(dir) = std::fs::read_dir(root.join(&parent)) else {
            continue;
        };
        for child in dir.flatten() {
            if !child.path().join(MANIFEST).is_file() {
                continue;
            }
            let rel = parent.join(child.file_name());
            let rel = rel.to_string_lossy().replace('\\', "/");
            if !listed.contains(rel.as_str()) {
                return Err(format!(
                    "{}: `{rel}` holds a {MANIFEST} but is not a [workspace] member; \
                     list it or delete it",
                    root.join(MANIFEST).display()
                ));
            }
        }
    }
    Ok(())
}

/// The workspace this path belongs to, checked, or the reason it does not hold together.
pub fn check_for(src: &Path) -> Result<(), String> {
    Chain::for_source(src).check_workspace()
}

/// `key = value` pairs of one table, comments and other tables ignored.
pub fn table_of(toml: &str, name: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let mut inside = false;
    for line in toml.lines() {
        let t = line.trim();
        if t.starts_with('#') {
            continue;
        }
        if t.starts_with('[') {
            inside = t == name;
            continue;
        }
        if inside && let Some((k, v)) = t.split_once('=') {
            let (k, v) = (k.trim(), v.trim());
            if !k.is_empty() && !v.is_empty() {
                out.push((k.to_string(), v.to_string()));
            }
        }
    }
    out
}

/// Each `[[name]]` block of a table array, in the order written.
fn blocks_of(toml: &str, name: &str) -> Vec<Vec<(String, String)>> {
    let mut out: Vec<Vec<(String, String)>> = Vec::new();
    let mut inside = false;
    for line in toml.lines() {
        let t = line.trim();
        if t.starts_with('#') {
            continue;
        }
        if t.starts_with('[') {
            inside = t == name;
            if inside {
                out.push(Vec::new());
            }
            continue;
        }
        if inside
            && let Some((k, v)) = t.split_once('=')
            && let Some(block) = out.last_mut()
        {
            let (k, v) = (k.trim(), v.trim());
            if !k.is_empty() && !v.is_empty() {
                block.push((k.to_string(), v.to_string()));
            }
        }
    }
    out
}

/// A `key = ["a", "b"]` list, which may span lines.
pub fn array_of(toml: &str, table: &str, key: &str) -> Vec<String> {
    let mut inside = false;
    let mut gathering: Option<String> = None;
    for line in toml.lines() {
        let t = line.trim();
        if t.starts_with('#') {
            continue;
        }
        if gathering.is_none() {
            if t.starts_with('[') && !t.starts_with("[[") {
                inside = t == table;
                continue;
            }
            if t.starts_with('[') {
                inside = false;
                continue;
            }
            if !inside {
                continue;
            }
            let Some((k, v)) = t.split_once('=') else {
                continue;
            };
            if k.trim() != key {
                continue;
            }
            gathering = Some(v.trim().trim_start_matches('[').to_string());
        } else if let Some(acc) = gathering.as_mut() {
            acc.push(' ');
            acc.push_str(t);
        }
        if let Some(acc) = &gathering
            && acc.contains(']')
        {
            let body = acc.split(']').next().unwrap_or("");
            return body
                .split(',')
                .map(|s| unquote(s.trim()).to_string())
                .filter(|s| !s.is_empty())
                .collect();
        }
    }
    Vec::new()
}

/// A scalar as the reader wants it: without its quotes.
pub fn unquote(v: &str) -> &str {
    let v = v.trim();
    match v.strip_prefix('"').and_then(|r| r.strip_suffix('"')) {
        Some(inner) => inner,
        None => v,
    }
}

/// The directory a bare `maca build`/`run`/`test` is about, for a message that has to name it.
pub fn here_or_root() -> PathBuf {
    workspace_root(Path::new(".")).unwrap_or_else(|| PathBuf::from("."))
}

#[cfg(test)]
mod tests {
    use super::*;

    const ROOT: &str = "[workspace]\nmembers = [\n    \"modules/std\",\n    \"apps/site\",\n]\n\n\
                        [format]\nindent_size = 4\n\n[[bin]]\nname = \"taskr\"\npath = \"t.maca\"\n";

    #[test]
    fn an_array_may_span_lines_and_keeps_its_order() {
        assert_eq!(
            array_of(ROOT, "[workspace]", "members"),
            vec!["modules/std".to_string(), "apps/site".to_string()]
        );
    }

    #[test]
    fn an_array_on_one_line_reads_the_same() {
        let toml = "[workspace]\nmembers = [\"a\", \"b\"]\n";
        assert_eq!(
            array_of(toml, "[workspace]", "members"),
            vec!["a".to_string(), "b".to_string()]
        );
    }

    #[test]
    fn a_table_array_does_not_leak_into_the_table_before_it() {
        assert_eq!(
            table_of(ROOT, "[format]"),
            vec![("indent_size".to_string(), "4".to_string())]
        );
        let bins = blocks_of(ROOT, "[[bin]]");
        assert_eq!(bins.len(), 1);
        assert_eq!(bins[0][0], ("name".to_string(), "\"taskr\"".to_string()));
    }

    #[test]
    fn a_key_of_another_table_is_not_this_table() {
        let toml = "[workspace]\nmembers = [\"a\"]\n\n[package]\nmembers = [\"b\"]\n";
        assert_eq!(
            array_of(toml, "[package]", "members"),
            vec!["b".to_string()]
        );
    }
}
