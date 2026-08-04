use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

#[cfg(not(target_family = "wasm"))]
mod embedded {
    include!(concat!(env!("OUT_DIR"), "/files.rs"));
}

#[cfg(not(target_family = "wasm"))]
pub use embedded::FILES;

/// A browser has no filesystem to unpack a standard library into, so the wasm front end carries none.
#[cfg(target_family = "wasm")]
pub static FILES: &[(&str, &str)] = &[];

/// The variable that points `import std/…` at a working copy instead of the one the compiler carries.
pub const OVERRIDE: &str = "MACA_STDLIB";

/// The file that marks an unpacked standard library as complete.
const STAMP: &str = ".stamp";

/// The manifest that stops an import walk at the unpacked standard library, so its own imports never reach past it.
const ROOT_MANIFEST: &str = "[workspace]\n";

static ROOT: OnceLock<Option<PathBuf>> = OnceLock::new();

/// Where this machine keeps what `maca` builds and unpacks.
pub fn cache_root() -> PathBuf {
    std::env::var_os("MACA_CACHE")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("XDG_CACHE_HOME").map(|c| PathBuf::from(c).join("maca")))
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".cache/maca")))
        .or_else(|| std::env::var_os("LOCALAPPDATA").map(|a| PathBuf::from(a).join("maca")))
        .unwrap_or_else(std::env::temp_dir)
}

/// A stable hex digest over every embedded file, so a compiler carrying a different standard library unpacks it somewhere of its own.
pub fn digest() -> String {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    for (rel, text) in FILES {
        rel.hash(&mut h);
        text.len().hash(&mut h);
        text.hash(&mut h);
    }
    format!("{:016x}", h.finish())
}

/// The packages the compiler carries, in the order an import writes them.
pub fn packages() -> Vec<&'static str> {
    let mut out: Vec<&'static str> = FILES
        .iter()
        .filter_map(|(rel, _)| rel.split('/').next())
        .collect();
    out.dedup();
    out
}

/// The directory an unpacked standard library is or lives under, answered without unpacking one.
pub fn holder() -> PathBuf {
    std::env::var_os(OVERRIDE)
        .map(PathBuf::from)
        .unwrap_or_else(|| cache_root().join("stdlib"))
}

/// The directory the standard library's source lives in: what `MACA_STDLIB` names, or the copy inside the compiler, unpacked once.
pub fn root() -> Option<&'static Path> {
    ROOT.get_or_init(install).as_deref()
}

fn install() -> Option<PathBuf> {
    let holder = holder();
    if std::env::var_os(OVERRIDE).is_some() {
        return holder.is_dir().then_some(holder);
    }
    if FILES.is_empty() {
        return None;
    }
    let dir = holder.join(format!("{}-{}", env!("CARGO_PKG_VERSION"), digest()));
    if dir.join(STAMP).is_file() {
        return Some(dir);
    }
    unpack(&dir).ok().map(|()| dir)
}

/// Write the embedded copy out beside its siblings, whole or not at all, because a half-written standard library is a compile error nobody can read.
fn unpack(dir: &Path) -> std::io::Result<()> {
    let parent = dir.parent().unwrap_or(Path::new("."));
    std::fs::create_dir_all(parent)?;
    let stem = dir.file_name().unwrap_or_default().to_string_lossy();
    let tmp = parent.join(format!(".{stem}.{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::create_dir_all(&tmp)?;

    for (rel, text) in FILES {
        let path = tmp.join(rel);
        if let Some(holder) = path.parent() {
            std::fs::create_dir_all(holder)?;
        }
        std::fs::write(&path, text)?;
    }
    std::fs::write(tmp.join("maca.toml"), ROOT_MANIFEST)?;
    std::fs::write(tmp.join(STAMP), digest())?;

    if std::fs::rename(&tmp, dir).is_err() {
        let _ = std::fs::remove_dir_all(&tmp);
        if !dir.join(STAMP).is_file() {
            return Err(std::io::Error::other(format!(
                "cannot unpack the standard library into {}",
                dir.display()
            )));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_standard_library_travels_with_the_compiler() {
        assert!(
            FILES.len() > 30,
            "the compiler carries {} files",
            FILES.len()
        );
        let names: Vec<&str> = packages();
        for want in [
            "std", "cli", "http", "bench", "profile", "signal", "tambo", "web",
        ] {
            assert!(names.contains(&want), "`{want}` is not carried: {names:?}");
        }
    }

    #[test]
    fn a_package_carries_its_source_and_its_manifest_and_not_its_suite() {
        let has = |rel: &str| FILES.iter().any(|(p, _)| *p == rel);
        assert!(has("std/json.maca"), "the source");
        assert!(has("std/maca.toml"), "the manifest that names the package");
        assert!(
            !FILES.iter().any(|(p, _)| p.contains("/tests/")),
            "a package's own suite is the repository's, not the user's"
        );
    }

    #[test]
    fn the_digest_answers_for_the_contents() {
        assert_eq!(digest(), digest());
        assert_eq!(digest().len(), 16);
    }
}
