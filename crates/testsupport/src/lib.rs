//! What the integration suites need in order to decide whether they can run.
//!
//! A crate rather than a `tests/common/mod.rs` because `crates/runtime`'s tests
//! need the same build lock as `crates/driver`'s, and a lock two crates spell
//! differently is a lock that protects nothing.
//!
//! These were forty-two copies across twenty-three files, and copies drift: one
//! probed `java` with `-version` where the rest used `--version`, and three
//! `wsl` probes used `.status()` and leaked WSL's output into the test log. A
//! probe that drifts the wrong way makes a suite skip and report success, which
//! is the failure this whole directory exists to prevent.

use std::path::{Path, PathBuf};
use std::process::Command;

/// Is `cmd` present and runnable? Probed with `--version`, which every tool
/// this repository shells out to answers.
pub fn have(cmd: &str) -> bool {
    ran(Command::new(cmd).arg("--version"))
}

/// The JVM tools are the exception: `javac --version` is a modern spelling that
/// older JDKs reject, and `-version` is what both accept.
pub fn have_jdk(cmd: &str) -> bool {
    ran(Command::new(cmd).arg("-version"))
}

/// Is WSL present? On a Windows host the native path goes through it, and these
/// suites take the host-`cc` path instead, so its presence means *skip*.
pub fn have_wsl() -> bool {
    ran(Command::new("wsl").arg("true"))
}

fn ran(cmd: &mut Command) -> bool {
    // `.output()` rather than `.status()`: status lets the child write straight
    // to the test's stdout, and a probe should be silent.
    cmd.output().map(|o| o.status.success()).unwrap_or(false)
}

/// The host cannot run a native-toolchain suite: no C compiler, or a WSL that
/// would take over the native path. Prints why, so a skipped suite says so
/// rather than passing quietly.
pub fn unsupported_host() -> bool {
    if have_wsl() {
        eprintln!("skipping: wsl is present, so the native path is not the host cc's");
        return true;
    }
    if !have("cc") {
        eprintln!("skipping: no host cc");
        return true;
    }
    false
}

/// A Windows path as WSL sees it: `C:\x` is `/mnt/c/x`.
pub fn to_wsl(p: &Path) -> String {
    let s = p.display().to_string().replace('\\', "/");
    match s.split_once(':') {
        Some((drive, rest)) if drive.len() == 1 => {
            format!("/mnt/{}{}", drive.to_lowercase(), rest)
        }
        _ => s,
    }
}

/// A cross-process lock around the native toolchain.
///
/// `zig` and `nix` cannot take a dozen concurrent invocations, so every suite
/// that shells out to them serializes here, across crates, which is why this
/// lives in one.
pub struct BuildLock {
    path: PathBuf,
}

impl BuildLock {
    pub fn acquire() -> Self {
        let path = std::env::temp_dir().join("maca-it-build.lock");
        loop {
            if std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&path)
                .is_ok()
            {
                return BuildLock { path };
            }
            // A lock left behind by a killed run would block every later one,
            // so a stale one is taken rather than waited on forever.
            if let Ok(m) = std::fs::metadata(&path)
                && m.modified()
                    .ok()
                    .and_then(|t| t.elapsed().ok())
                    .is_some_and(|d| d.as_secs() > 600)
            {
                let _ = std::fs::remove_file(&path);
                continue;
            }
            std::thread::sleep(std::time::Duration::from_millis(200));
        }
    }
}

impl Drop for BuildLock {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}
