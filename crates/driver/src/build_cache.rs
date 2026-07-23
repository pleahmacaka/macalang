//! Content-addressed build cache — the core of `maca`'s incremental builds.
//!
//! A native build is a pure function of `(source, compiler version, target)`, so
//! the finished binary is cached under a hash of exactly those. An unchanged
//! `maca build`/`run` then skips the whole pipeline (parse → check → emit → cc)
//! and just copies the cached artifact — effectively instant.
//!
//! The invariant C runtime is cached separately as a compiled object, so even a
//! *changed* program need not recompile the runtime — only its own `main.c`.
//!
//! Cache root: `$MACA_CACHE`, else `$XDG_CACHE_HOME/maca`, else
//! `~/.cache/maca`, else a temp dir. Set `MACA_NO_CACHE=1` to disable.

use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

/// A per-build-unique temp suffix — several builds (even threads of one process,
/// e.g. the test suite) can target the same cache key concurrently, so each
/// writes to its own temp file before the atomic rename into place.
static TMP_SEQ: AtomicU64 = AtomicU64::new(0);
fn unique_tmp(dst: &Path, ext: &str) -> PathBuf {
    let n = TMP_SEQ.fetch_add(1, Ordering::Relaxed);
    dst.with_extension(format!("{}.{n}.{ext}", std::process::id()))
}

/// Whether the build cache is enabled (default on; `MACA_NO_CACHE=1` disables).
pub fn enabled() -> bool {
    !matches!(std::env::var("MACA_NO_CACHE").as_deref(), Ok("1") | Ok("true"))
}

/// The cache root directory, created on demand.
pub fn root() -> PathBuf {
    let base = std::env::var_os("MACA_CACHE")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("XDG_CACHE_HOME").map(|c| PathBuf::from(c).join("maca")))
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".cache/maca")))
        .unwrap_or_else(std::env::temp_dir);
    base.join("build")
}

/// A stable 64-bit hex hash over the given byte slices.
pub fn hash(parts: &[&[u8]]) -> String {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    for p in parts {
        p.len().hash(&mut h); // length-prefix so concatenations don't collide
        p.hash(&mut h);
    }
    format!("{:016x}", h.finish())
}

/// The artifact key for a native build of `source`, given the compiler
/// `version` and a `target` label.
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
    // write to a unique temp sibling then rename, so a concurrent reader never
    // sees a half-copied binary and concurrent writers don't clobber each other.
    let tmp = unique_tmp(&dst, "tmp");
    if std::fs::copy(artifact, &tmp).is_ok() {
        let _ = std::fs::rename(&tmp, &dst);
    }
    let _ = std::fs::remove_file(&tmp);
}

/// Copy a cached artifact to `out` and mark it executable.
pub fn place(cached: &Path, out: &Path) -> Result<(), String> {
    std::fs::copy(cached, out).map_err(|e| format!("cache copy failed: {e}"))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(out, std::fs::Permissions::from_mode(0o755));
    }
    Ok(())
}

/// A cached object path for `key`, compiling it via `build` on a miss. Returns
/// the path to the (now-present) `.o`. On any cache trouble it falls back to
/// building straight into `fallback` and returning that.
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
    // rename into place; if another build won the race, dst is already valid and
    // we drop our temp. Either way the caller links against a complete object.
    if std::fs::rename(&tmp, &dst).is_err() {
        if dst.exists() {
            let _ = std::fs::remove_file(&tmp);
            return Ok(dst);
        }
        return Ok(tmp); // no winner yet — use our own complete object
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
        // ("ab","c") vs ("a","bc") must not collide
        assert_ne!(hash(&[b"ab", b"c"]), hash(&[b"a", b"bc"]));
    }
}
