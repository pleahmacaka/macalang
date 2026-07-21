//! Phase 5 memory tests: the reuse free-list and valgrind-cleanliness. Compiles
//! small C drivers against the runtime via `zig cc` in WSL; skips without it.

use std::path::{Path, PathBuf};
use std::process::Command;

fn wsl_ready() -> bool {
    Command::new("wsl").arg("true").status().map(|s| s.success()).unwrap_or(false)
}

/// Cross-process build lock (nix/zig can't be hammered concurrently).
struct BuildLock(PathBuf);
impl BuildLock {
    fn acquire() -> Self {
        let p = std::env::temp_dir().join("maca-it-build.lock");
        for _ in 0..1200 {
            if let Ok(m) = std::fs::metadata(&p) {
                if m.modified().ok().and_then(|t| t.elapsed().ok()).map(|e| e.as_secs() > 300).unwrap_or(false) {
                    let _ = std::fs::remove_file(&p);
                }
            }
            if std::fs::OpenOptions::new().write(true).create_new(true).open(&p).is_ok() {
                return BuildLock(p);
            }
            std::thread::sleep(std::time::Duration::from_millis(300));
        }
        BuildLock(p)
    }
}
impl Drop for BuildLock {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

fn to_wsl(p: &Path) -> String {
    let s = p.to_string_lossy().replace('\\', "/");
    let b = s.as_bytes();
    if b.len() >= 2 && b[1] == b':' {
        format!("/mnt/{}{}", (b[0] as char).to_ascii_lowercase(), &s[2..])
    } else {
        s
    }
}

/// Write runtime + a C main, compile to a static binary, return its path.
/// `with_async` also links the concurrency runtime.
fn build_opt(main_c: &str, dirname: &str, with_async: bool) -> PathBuf {
    let _lk = BuildLock::acquire();
    let dir = std::env::temp_dir().join(dirname);
    std::fs::create_dir_all(&dir).unwrap();
    maca_runtime::write_to(&dir).unwrap();
    if with_async {
        maca_runtime::write_async(&dir).unwrap();
    }
    std::fs::write(dir.join("main.c"), main_c).unwrap();
    let out = dir.join("prog");
    let mut args: Vec<String> =
        ["nix", "shell", "nixpkgs#zig", "-c", "zig", "cc"].iter().map(|s| s.to_string()).collect();
    args.push(to_wsl(&dir.join("main.c")));
    args.push(to_wsl(&dir.join("maca_runtime.c")));
    if with_async {
        args.push(to_wsl(&dir.join("maca_async.c")));
        args.push("-pthread".into());
    }
    args.push("-I".into());
    args.push(to_wsl(&dir));
    args.push("-o".into());
    args.push(to_wsl(&out));
    for f in ["-O2", "-static", "-target", "x86_64-linux-musl"] {
        args.push(f.into());
    }
    let r = Command::new("wsl").args(&args).output().expect("zig via wsl");
    assert!(r.status.success(), "compile failed:\n{}", String::from_utf8_lossy(&r.stderr));
    out
}
fn build(main_c: &str, dirname: &str) -> PathBuf {
    build_opt(main_c, dirname, false)
}

const REUSE_C: &str = r#"
#include "maca_runtime.h"
#include <stdio.h>
int main(void) {
    maca_init();
    /* 1000 same-size alloc/drop cycles must malloc once and reuse the rest */
    for (int i = 0; i < 1000; i++) { void* p = maca_alloc(64); maca_drop(p); }
    printf("%llu %llu\n",
        (unsigned long long)maca_alloc_count(),
        (unsigned long long)maca_reuse_count());
    return 0;
}
"#;

#[test]
fn reuse_freelist() {
    if !wsl_ready() {
        eprintln!("skipping reuse_freelist: wsl not available");
        return;
    }
    let bin = build(REUSE_C, "maca-mem-reuse");
    let out = Command::new("wsl").arg(to_wsl(&bin)).output().unwrap();
    let text = String::from_utf8_lossy(&out.stdout);
    let n: Vec<u64> = text.split_whitespace().filter_map(|x| x.parse().ok()).collect();
    assert_eq!(n.len(), 2, "output: {text}");
    assert_eq!(n[0], 1, "expected exactly one real allocation, got {} ({text})", n[0]);
    assert_eq!(n[1], 999, "expected 999 free-list reuses, got {} ({text})", n[1]);
}

const CHURN_C: &str = r#"
#include "maca_runtime.h"
int main(void) {
    maca_init();
    /* build/append/discard arrays and strings; atexit(shutdown) must free all */
    maca_str s = "";
    for (int i = 0; i < 200; i++) s = maca_concat(s, maca_from_int(i));
    maca_str p = maca_path_join("/tmp/maca-test-dir", "x.txt");
    maca_write(p, s);
    (void)maca_read(p);
    return 0;
}
"#;

#[test]
fn valgrind_clean() {
    if !wsl_ready() {
        eprintln!("skipping valgrind_clean: wsl not available");
        return;
    }
    let bin = build(CHURN_C, "maca-mem-churn");
    // valgrind via nix; skip if it cannot be provisioned
    let probe = Command::new("wsl")
        .args(["sh", "-c", "nix shell nixpkgs#valgrind -c valgrind --version"])
        .output();
    if probe.as_ref().map(|o| !o.status.success()).unwrap_or(true) {
        eprintln!("skipping valgrind_clean: valgrind unavailable");
        return;
    }
    let cmd = format!(
        "nix shell nixpkgs#valgrind -c valgrind --leak-check=full {} 2>&1",
        to_wsl(&bin)
    );
    let out = Command::new("wsl").args(["sh", "-c", &cmd]).output().unwrap();
    let log = String::from_utf8_lossy(&out.stdout);
    assert!(
        log.contains("no leaks are possible")
            || log.contains("definitely lost: 0 bytes in 0 blocks"),
        "valgrind reported leaks:\n{log}"
    );
}

const CANCEL_C: &str = r#"
#include "maca_async.h"
#include "maca_runtime.h"
#include <stdio.h>
int main(void) {
    maca_init();
    /* workers loop until cancelled; if cancellation is broken this hangs forever */
    int64_t total = maca_cancel_demo(4);
    printf("%lld\n", (long long)total);
    return total > 0 ? 0 : 1;
}
"#;

#[test]
fn cancellation_stops_workers() {
    if !wsl_ready() {
        eprintln!("skipping cancellation_stops_workers: wsl not available");
        return;
    }
    let bin = build_opt(CANCEL_C, "maca-cancel", true);
    // The program terminating at all proves cancellation works (workers spin
    // in `while (!cancelled)`); a positive count proves they actually ran.
    let out = Command::new("wsl").arg(to_wsl(&bin)).output().unwrap();
    assert!(out.status.success(), "cancel demo failed/hung: {}", String::from_utf8_lossy(&out.stderr));
    let n: i64 = String::from_utf8_lossy(&out.stdout).trim().parse().unwrap_or(0);
    assert!(n > 0, "workers should have run before cancellation; got {n}");
}
