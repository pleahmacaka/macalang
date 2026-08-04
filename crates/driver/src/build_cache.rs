use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

/// A per-build-unique temp suffix.
static TMP_SEQ: AtomicU64 = AtomicU64::new(0);
fn unique_tmp(dst: &Path, ext: &str) -> PathBuf {
    let n = TMP_SEQ.fetch_add(1, Ordering::Relaxed);
    dst.with_extension(format!("{}.{n}.{ext}", std::process::id()))
}

/// Whether the build cache is enabled (default on; `MACA_NO_CACHE=1` disables).
pub fn enabled() -> bool {
    !matches!(
        std::env::var("MACA_NO_CACHE").as_deref(),
        Ok("1") | Ok("true")
    )
}

/// The cache root directory, created on demand.
pub fn root() -> PathBuf {
    maca_stdlib::cache_root().join("build")
}

/// A stable 64-bit hex hash over the given byte slices.
pub fn hash(parts: &[&[u8]]) -> String {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    for p in parts {
        p.len().hash(&mut h);
        p.hash(&mut h);
    }
    format!("{:016x}", h.finish())
}

/// The artifact key for a native build of `source`, given the compiler `version` and a `target` label.
pub fn artifact_key(source: &str, version: &str, target: &str) -> String {
    hash(&[source.as_bytes(), version.as_bytes(), target.as_bytes()])
}

fn artifact_path(key: &str) -> PathBuf {
    root().join("bin").join(key)
}

/// A cached artifact for `key`, if present.
pub fn get(key: &str) -> Option<PathBuf> {
    if !enabled() {
        return None;
    }
    let p = artifact_path(key);
    p.exists().then_some(p)
}

/// Store `artifact` under `key` (best-effort; cache failures never fail a build).
pub fn put(key: &str, artifact: &Path) {
    if !enabled() {
        return;
    }
    let dst = artifact_path(key);
    if let Some(parent) = dst.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let tmp = unique_tmp(&dst, "tmp");
    if std::fs::copy(artifact, &tmp).is_ok() {
        let _ = std::fs::rename(&tmp, &dst);
    }
    let _ = std::fs::remove_file(&tmp);
}

/// Copy a cached artifact to `out` and mark it executable.
pub fn place(cached: &Path, out: &Path) -> Result<(), String> {
    if let Some(parent) = out.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let tmp = unique_tmp(out, "place");
    std::fs::copy(cached, &tmp).map_err(|e| format!("cache copy failed: {e}"))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(0o755));
    }
    if let Err(e) = std::fs::rename(&tmp, out) {
        let _ = std::fs::remove_file(&tmp);
        return Err(format!("cache install failed: {e}"));
    }
    Ok(())
}

/// A cached object path for `key`, compiling it via `build` on a miss.
pub fn object<F>(key: &str, fallback: &Path, build: F) -> Result<PathBuf, String>
where
    F: FnOnce(&Path) -> Result<(), String>,
{
    if !enabled() {
        build(fallback)?;
        return Ok(fallback.to_path_buf());
    }
    let dst = root().join("obj").join(format!("{key}.o"));
    if dst.exists() {
        return Ok(dst);
    }
    if let Some(parent) = dst.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let tmp = unique_tmp(&dst, "building.o");
    build(&tmp)?;
    if std::fs::rename(&tmp, &dst).is_err() {
        if dst.exists() {
            let _ = std::fs::remove_file(&tmp);
            return Ok(dst);
        }
        return Ok(tmp);
    }
    Ok(dst)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hash_is_stable_and_distinct() {
        let a = hash(&[b"main() -> int { 0 }", b"0.1.0", b"native"]);
        let b = hash(&[b"main() -> int { 0 }", b"0.1.0", b"native"]);
        assert_eq!(a, b, "same inputs → same hash");
        let c = hash(&[b"main() -> int { 1 }", b"0.1.0", b"native"]);
        assert_ne!(a, c, "different source → different hash");
        let d = hash(&[b"main() -> int { 0 }", b"0.2.0", b"native"]);
        assert_ne!(a, d, "different version → different hash");
    }

    #[test]
    fn length_prefix_avoids_concatenation_collision() {
        assert_ne!(hash(&[b"ab", b"c"]), hash(&[b"a", b"bc"]));
    }
}
