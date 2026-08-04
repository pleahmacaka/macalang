use std::path::{Path, PathBuf};
use std::process::Command;

/// Is `cmd` present and runnable?
pub fn have(cmd: &str) -> bool {
    ran(Command::new(cmd).arg("--version"))
}

/// The JVM tools are the exception.
pub fn have_jdk(cmd: &str) -> bool {
    ran(Command::new(cmd).arg("-version"))
}

/// Is WSL present?
pub fn have_wsl() -> bool {
    ran(Command::new("wsl").arg("true"))
}

fn ran(cmd: &mut Command) -> bool {
    cmd.output().map(|o| o.status.success()).unwrap_or(false)
}

/// The host cannot run a native-toolchain suite.
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
