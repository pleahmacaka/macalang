use std::path::{Path, PathBuf};
use std::process::Command;

const REPO: &str = "pleahmacaka/macalang";

/// The release asset for the platform this installer was built for, which is the platform it is running on.
fn asset() -> &'static str {
    match (std::env::consts::OS, std::env::consts::ARCH) {
        ("linux", "x86_64") => "maca-linux-x86_64.tar.gz",
        ("linux", "aarch64") => "maca-linux-aarch64.tar.gz",
        ("macos", "x86_64") => "maca-macos-x86_64.tar.gz",
        ("macos", "aarch64") => "maca-macos-aarch64.tar.gz",
        ("windows", "x86_64") => "maca-windows-x86_64.zip",
        _ => "",
    }
}

fn binaries() -> [&'static str; 2] {
    if cfg!(windows) {
        ["maca.exe", "maca-lsp.exe"]
    } else {
        ["maca", "maca-lsp"]
    }
}

#[derive(Debug)]
struct Options {
    version: String,
    prefix: PathBuf,
}

fn home() -> PathBuf {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
}

fn parse(args: &[String]) -> Result<Options, String> {
    let mut version = std::env::var("MACA_VERSION").unwrap_or_else(|_| "latest".into());
    let mut prefix = std::env::var_os("PREFIX")
        .map(PathBuf::from)
        .unwrap_or_else(|| home().join(".local"));
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--version" | "-v" => {
                version = it
                    .next()
                    .ok_or("--version wants a value, e.g. 9.9.9")?
                    .clone()
            }
            "--prefix" | "-p" => {
                prefix = PathBuf::from(it.next().ok_or("--prefix wants a directory")?)
            }
            "--help" | "-h" => return Err(usage()),
            other => return Err(format!("unknown option `{other}`\n\n{}", usage())),
        }
    }
    Ok(Options { version, prefix })
}

fn usage() -> String {
    format!(
        "maca-install: put the Maca toolchain on this machine\n\n\
         usage: maca-install [--version <x.y.z>] [--prefix <dir>]\n\n\
         \x20 --version   which release, e.g. 9.9.9 (default: latest)\n\
         \x20 --prefix    where to install; the binaries go in <prefix>/bin\n\
         \x20             (default: {})\n\n\
         MACA_VERSION and PREFIX set the same two things.",
        home().join(".local").display()
    )
}

fn download_url(version: &str, asset: &str) -> String {
    if version == "latest" {
        format!("https://github.com/{REPO}/releases/latest/download/{asset}")
    } else {
        format!("https://github.com/{REPO}/releases/download/{version}/{asset}")
    }
}

fn run(cmd: &str, args: &[&str]) -> Result<(), String> {
    let out = Command::new(cmd)
        .args(args)
        .output()
        .map_err(|e| format!("{cmd}: {e}"))?;
    if out.status.success() {
        return Ok(());
    }
    Err(format!(
        "{cmd} failed: {}",
        String::from_utf8_lossy(&out.stderr).trim()
    ))
}

fn download(url: &str, to: &Path) -> Result<(), String> {
    run("curl", &["-fsSL", "-o", &to.to_string_lossy(), url])
        .map_err(|e| format!("could not download {url}\n  {e}"))
}

fn unpack(archive: &Path, into: &Path) -> Result<(), String> {
    run(
        "tar",
        &[
            "-xf",
            &archive.to_string_lossy(),
            "-C",
            &into.to_string_lossy(),
        ],
    )
}

/// Copy one binary into place and make it executable, replacing a running one by unlinking first.
fn place(from: &Path, to: &Path) -> Result<(), String> {
    let _ = std::fs::remove_file(to);
    std::fs::copy(from, to).map_err(|e| format!("{}: {e}", to.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(to, std::fs::Permissions::from_mode(0o755))
            .map_err(|e| format!("{}: {e}", to.display()))?;
    }
    Ok(())
}

/// Run the installed compiler on a program that imports the standard library, from a directory that is not a checkout.
fn verify(maca: &Path) -> Result<String, String> {
    let out = Command::new(maca)
        .arg("--version")
        .output()
        .map_err(|e| format!("the installed binary does not run: {e}"))?;
    let version = String::from_utf8_lossy(&out.stdout).trim().to_string();

    let probe = std::env::temp_dir().join(format!("maca-install-probe-{}", std::process::id()));
    std::fs::create_dir_all(&probe).map_err(|e| e.to_string())?;
    let file = probe.join("probe.maca");
    std::fs::write(
        &file,
        "import { lines } from std/text\n\n\
         main() -> int {\n    lines(\"a\\nb\").length() == 2 ? 0 : 1\n}\n",
    )
    .map_err(|e| e.to_string())?;
    let ran = Command::new(maca)
        .arg("run")
        .arg(&file)
        .output()
        .map_err(|e| e.to_string())?;
    let _ = std::fs::remove_dir_all(&probe);
    if !ran.status.success() {
        return Err(format!(
            "{version} installed, but compiling a program that imports std failed.\n\
             `maca build` and `maca run` need a C compiler (cc or clang) on PATH.\n  {}",
            String::from_utf8_lossy(&ran.stderr).trim()
        ));
    }
    Ok(version)
}

fn on_path(dir: &Path) -> bool {
    std::env::var_os("PATH")
        .map(|p| std::env::split_paths(&p).any(|d| d == dir))
        .unwrap_or(false)
}

fn install(o: &Options) -> Result<(), String> {
    let asset = asset();
    if asset.is_empty() {
        return Err(format!(
            "no release is built for {}-{}",
            std::env::consts::OS,
            std::env::consts::ARCH
        ));
    }
    let bin = o.prefix.join("bin");
    std::fs::create_dir_all(&bin).map_err(|e| format!("{}: {e}", bin.display()))?;

    let work = std::env::temp_dir().join(format!("maca-install-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&work);
    std::fs::create_dir_all(&work).map_err(|e| e.to_string())?;

    println!("downloading {asset} ({} release)", o.version);
    let archive = work.join(asset);
    download(&download_url(&o.version, asset), &archive)?;
    unpack(&archive, &work)?;

    let mut placed = Vec::new();
    for name in binaries() {
        let from = work.join(name);
        if from.is_file() {
            place(&from, &bin.join(name))?;
            placed.push(name);
        }
    }
    let _ = std::fs::remove_dir_all(&work);
    if placed.is_empty() {
        return Err(format!("{asset} contained none of {:?}", binaries()));
    }
    for name in &placed {
        println!("installed {name} -> {}", bin.join(name).display());
    }

    let version = verify(&bin.join(binaries()[0]))?;
    println!("verified: {version} compiled and ran a program that imports std");

    if !on_path(&bin) {
        println!(
            "\nadd it to your PATH:\n  {}",
            if cfg!(windows) {
                format!("setx PATH \"{};%PATH%\"", bin.display())
            } else {
                format!(
                    "echo 'export PATH=\"{}:$PATH\"' >> ~/.profile",
                    bin.display()
                )
            }
        );
    }
    println!("\ndone. Try: maca init myapp");
    Ok(())
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let options = match parse(&args) {
        Ok(o) => o,
        Err(message) => {
            println!("{message}");
            std::process::exit(if args.iter().any(|a| a == "--help" || a == "-h") {
                0
            } else {
                2
            });
        }
    };
    if let Err(e) = install(&options) {
        eprintln!("maca-install: {e}");
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn latest_and_a_named_version_are_different_urls() {
        assert!(download_url("latest", "a.tar.gz").ends_with("releases/latest/download/a.tar.gz"));
        assert!(download_url("9.9.9", "a.tar.gz").ends_with("releases/download/9.9.9/a.tar.gz"));
    }

    #[test]
    fn this_platform_has_an_asset_to_ask_for() {
        assert!(
            !asset().is_empty(),
            "the installer is built per platform, so it is built for one that has a release"
        );
    }

    #[test]
    fn a_flag_that_wants_a_value_and_does_not_get_one_says_so() {
        let e = parse(&["--version".to_string()]).expect_err("no value follows");
        assert!(e.contains("--version"), "{e}");
        let e = parse(&["--nope".to_string()]).expect_err("not an option");
        assert!(e.contains("--nope") && e.contains("usage"), "{e}");
    }

    #[test]
    fn the_flags_and_the_environment_set_the_same_two_things() {
        let o = parse(&[
            "--version".into(),
            "9.9.9".into(),
            "--prefix".into(),
            "/opt/maca".into(),
        ])
        .expect("both flags");
        assert_eq!(o.version, "9.9.9");
        assert_eq!(o.prefix, PathBuf::from("/opt/maca"));
    }
}
