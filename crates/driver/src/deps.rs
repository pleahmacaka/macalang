use std::path::{Path, PathBuf};
use std::process::Command;

const NPM_REGISTRY: &str = "https://registry.npmjs.org";
const DEFAULT_REGISTRY: &str = "https://registry.maca.dev";
const MODULES_DIR: &str = "maca_modules";
const LOCK_FILE: &str = "maca.lock";
const MANIFEST: &str = "maca.toml";
const REPO: &str = "pleahmacaka/macalang";

/// A dependency source parsed from a spec string.
#[derive(Debug, Clone, PartialEq)]
pub enum Source {
    /// `npm:<pkg>[@<req>]`: the public npm registry.
    Npm { pkg: String, req: String },
    /// `git+<url>[#<ref>]`: any git remote.
    Git { url: String, reff: Option<String> },
    /// `<name>[@<req>]`: the maca registry.
    Registry { name: String, req: String },
}

/// Parse a dependency spec into its `(name, source)`.
pub fn parse_spec(spec: &str) -> Result<(String, Source), String> {
    let spec = spec.trim();
    if spec.is_empty() {
        return Err("empty dependency spec".into());
    }
    if let Some(rest) = spec.strip_prefix("npm:") {
        let (pkg, req) = split_version(rest);
        let name = pkg.rsplit('/').next().unwrap_or(pkg).to_string();
        return Ok((
            name,
            Source::Npm {
                pkg: pkg.to_string(),
                req,
            },
        ));
    }
    if let Some(rest) = spec.strip_prefix("git+") {
        let (url, reff) = match rest.split_once('#') {
            Some((u, r)) => (u.to_string(), Some(r.to_string())),
            None => (rest.to_string(), None),
        };
        return Ok((git_name(&url), Source::Git { url, reff }));
    }
    let (name, req) = split_version(spec);
    Ok((
        name.to_string(),
        Source::Registry {
            name: name.to_string(),
            req,
        },
    ))
}

/// Split `name[@version]`, honouring a leading `@scope`.
fn split_version(s: &str) -> (&str, String) {
    if let Some(idx) = s.rfind('@')
        && idx > 0
    {
        return (&s[..idx], s[idx + 1..].to_string());
    }
    (s, "latest".to_string())
}

/// The bare package name for a git url (last path segment, minus `.git`).
fn git_name(url: &str) -> String {
    url.trim_end_matches('/')
        .rsplit(['/', ':'])
        .next()
        .unwrap_or("dep")
        .trim_end_matches(".git")
        .to_string()
}

/// The prefix an asset import writes to name an installed package rather than a file beside it.
pub const PACKAGE_PREFIX: &str = "npm:";

/// The `package.json` keys a package states an entry point under, best first.
const ENTRY_KEYS: &[&str] = &["style", "browser", "module", "main"];

/// The file extensions an asset import of one language will accept an entry point at.
fn asset_extensions(lang: &str) -> &'static [&'static str] {
    match lang {
        "css" => &[".css"],
        "wasm" => &[".wasm"],
        _ => &[".js", ".mjs", ".cjs"],
    }
}

/// What an asset import of one language is called in a sentence.
fn asset_noun(lang: &str) -> &'static str {
    match lang {
        "css" => "stylesheet",
        "wasm" => "WebAssembly",
        _ => "script",
    }
}

/// The file `import <lang> "npm:<spec>"` names: the entry point the installed package states, or the file inside it the spec asked for.
pub fn package_asset(importer: &Path, lang: &str, spec: &str) -> Result<PathBuf, String> {
    let wrote = |why: String| format!("import {lang} \"{PACKAGE_PREFIX}{spec}\": {why}");
    let (name, sub) = maca_parser::modules::split_package(spec);
    if name.is_empty() {
        return Err(wrote("no package named".into()));
    }
    let Some(dir) = maca_parser::modules::installed_dir(&name, importer) else {
        return Err(wrote(format!(
            "`{name}` is not installed; run `maca add {PACKAGE_PREFIX}{name}`"
        )));
    };
    if let Some(sub) = sub {
        let file = dir.join(&sub);
        return file
            .is_file()
            .then_some(file)
            .ok_or_else(|| wrote(format!("`{name}` has no `{sub}`")));
    }
    let manifest = dir.join("package.json");
    let text = std::fs::read_to_string(&manifest)
        .map_err(|e| wrote(format!("{}: {e}", manifest.display())))?;
    let json: serde_json::Value =
        serde_json::from_str(&text).map_err(|e| wrote(format!("bad package.json: {e}")))?;
    let exts = asset_extensions(lang);
    let entry = ENTRY_KEYS.iter().find_map(|k| {
        let v = json.get(*k)?.as_str()?;
        exts.iter().any(|e| v.ends_with(e)).then_some((*k, v))
    });
    let Some((key, rel)) = entry else {
        return Err(wrote(format!(
            "`{name}` states no {} entry point ({} names none of {}); name the file, as in \
             `import {lang} \"{PACKAGE_PREFIX}{name}/dist/…\"`",
            asset_noun(lang),
            ENTRY_KEYS.join("/"),
            exts.join("/")
        )));
    };
    let file = dir.join(rel.trim_start_matches("./"));
    file.is_file().then_some(file).ok_or_else(|| {
        wrote(format!(
            "`{name}` states {key} = \"{rel}\", which is not there"
        ))
    })
}

/// A resolved dependency: a concrete version plus how to fetch it.
struct Resolved {
    version: String,
    tarball: Option<String>,
    integrity: Option<String>,
    git_sha: Option<String>,
}

/// The registry base for maca-registry deps, overridable via `[registry] url`.
fn registry_url() -> String {
    crate::manifest::Chain::here()
        .value("[registry]", "url")
        .map_or_else(
            || DEFAULT_REGISTRY.to_string(),
            |(_, v)| crate::manifest::unquote(&v).to_string(),
        )
}

/// `maca add <spec>…`: add and fetch one or more dependencies.
pub fn cmd_add(args: &[String]) {
    let specs: Vec<&String> = args.iter().filter(|a| !a.starts_with('-')).collect();
    if specs.is_empty() {
        eprintln!(
            "usage: maca add <spec>…\n  e.g. maca add npm:axios\n       maca add git+https://github.com/u/lib#main\n       maca add utils@^1.2.0"
        );
        std::process::exit(2);
    }
    ensure_manifest();
    let registry = registry_url();
    let mut failed = false;
    for spec in specs {
        match add_one(spec, &registry) {
            Ok(v) => println!("added {v}"),
            Err(e) => {
                eprintln!("maca add {spec}: {e}");
                failed = true;
            }
        }
    }
    if failed {
        std::process::exit(1);
    }
}

fn add_one(spec: &str, registry: &str) -> Result<String, String> {
    let (name, src) = parse_spec(spec)?;
    let r = resolve(&src, registry)?;
    fetch(&name, &src, &r)?;
    manifest_put(&name, spec)?;
    lock_put(&name, spec, &src, &r)?;
    Ok(format!("{name}@{} → {}/{name}", r.version, MODULES_DIR))
}

/// `maca update`: re-resolve every dependency to its latest matching version.
pub fn cmd_update(_args: &[String]) {
    let deps = manifest_deps();
    if deps.is_empty() {
        println!("no dependencies in {MANIFEST}");
        return;
    }
    let registry = registry_url();
    let mut failed = false;
    for (name, spec) in deps {
        match (|| -> Result<String, String> {
            let (_, src) = parse_spec(&spec)?;
            let r = resolve(&src, &registry)?;
            fetch(&name, &src, &r)?;
            lock_put(&name, &spec, &src, &r)?;
            Ok(r.version)
        })() {
            Ok(v) => println!("{name} → {v}"),
            Err(e) => {
                eprintln!("maca update {name}: {e}");
                failed = true;
            }
        }
    }
    if failed {
        std::process::exit(1);
    }
}

/// `maca upgrade`: self-update the `maca` toolchain from GitHub releases.
pub fn cmd_upgrade(_args: &[String]) {
    let cur = env!("CARGO_PKG_VERSION");
    let body = match http_get(
        "https://api.github.com/repos/".to_string() + REPO + "/releases/latest",
    ) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("maca upgrade: cannot reach GitHub releases: {e}");
            eprintln!(
                "  install manually: curl -fsSL https://raw.githubusercontent.com/{REPO}/main/install.sh | bash"
            );
            std::process::exit(1);
        }
    };
    let json: serde_json::Value = serde_json::from_str(&body).unwrap_or(serde_json::Value::Null);
    let tag = json.get("tag_name").and_then(|v| v.as_str()).unwrap_or("");
    if tag.is_empty() {
        eprintln!("maca upgrade: no published release yet");
        eprintln!(
            "  install from source: curl -fsSL https://raw.githubusercontent.com/{REPO}/main/install.sh | bash"
        );
        std::process::exit(1);
    }
    let want = tag.trim_start_matches('v');
    if want == cur {
        println!("maca {cur} is already the latest release");
        return;
    }
    let asset = format!("maca-{}", target_triple());
    let url = json
        .get("assets")
        .and_then(|a| a.as_array())
        .and_then(|arr| {
            arr.iter().find(|x| {
                x.get("name")
                    .and_then(|n| n.as_str())
                    .map(|n| n.starts_with(&asset))
                    .unwrap_or(false)
            })
        })
        .and_then(|x| x.get("browser_download_url"))
        .and_then(|u| u.as_str());
    let Some(url) = url else {
        eprintln!(
            "maca upgrade: release {tag} has no asset for {}; build from source:",
            target_triple()
        );
        eprintln!("  curl -fsSL https://raw.githubusercontent.com/{REPO}/main/install.sh | bash");
        std::process::exit(1);
    };
    match self_replace(url) {
        Ok(()) => println!("upgraded maca {cur} → {want}"),
        Err(e) => {
            eprintln!("maca upgrade: {e}");
            std::process::exit(1);
        }
    }
}

fn resolve(src: &Source, registry: &str) -> Result<Resolved, String> {
    match src {
        Source::Npm { pkg, req } => resolve_registry(NPM_REGISTRY, pkg, req),
        Source::Registry { name, req } => resolve_registry(registry, name, req),
        Source::Git { url, reff } => {
            let sha = git_ls_remote(url, reff.as_deref())?;
            Ok(Resolved {
                version: reff.clone().unwrap_or_else(|| short(&sha)),
                tarball: None,
                integrity: None,
                git_sha: Some(sha),
            })
        }
    }
}

/// Resolve against an npm-compatible registry.
fn resolve_registry(base: &str, pkg: &str, req: &str) -> Result<Resolved, String> {
    let exact = req == "latest" || req.is_empty() || parse_semver(req).is_some();
    if exact {
        let tag = if req.is_empty() { "latest" } else { req };
        let url = format!("{base}/{}/{tag}", enc(pkg));
        let body = http_get(url).map_err(|e| format!("registry fetch failed: {e}"))?;
        let v: serde_json::Value =
            serde_json::from_str(&body).map_err(|e| format!("bad registry JSON: {e}"))?;
        return resolved_from_manifest(&v);
    }
    let url = format!("{base}/{}", enc(pkg));
    let body = http_get(url).map_err(|e| format!("registry fetch failed: {e}"))?;
    let doc: serde_json::Value =
        serde_json::from_str(&body).map_err(|e| format!("bad registry JSON: {e}"))?;
    let versions = doc
        .get("versions")
        .and_then(|v| v.as_object())
        .ok_or("no `versions` in package document")?;
    let best = versions
        .keys()
        .filter(|v| semver_satisfies(v, req))
        .max_by(|a, b| cmp_semver(a, b))
        .ok_or_else(|| format!("no published version of `{pkg}` satisfies `{req}`"))?;
    resolved_from_manifest(&versions[best])
}

fn resolved_from_manifest(v: &serde_json::Value) -> Result<Resolved, String> {
    let version = v
        .get("version")
        .and_then(|x| x.as_str())
        .ok_or("no `version` in manifest")?;
    let tarball = v
        .get("dist")
        .and_then(|d| d.get("tarball"))
        .and_then(|t| t.as_str());
    let integrity = v
        .get("dist")
        .and_then(|d| d.get("integrity"))
        .and_then(|t| t.as_str());
    Ok(Resolved {
        version: version.to_string(),
        tarball: tarball.map(str::to_string),
        integrity: integrity.map(str::to_string),
        git_sha: None,
    })
}

/// Parse `major.minor.patch` (a leading `v` and any `-prerelease`/`+build` suffix are ignored) into a comparable tuple.
fn parse_semver(s: &str) -> Option<(u64, u64, u64)> {
    let s = s.trim().trim_start_matches('v');
    let core = s.split(['-', '+']).next().unwrap_or(s);
    let mut it = core.split('.');
    let major = it.next()?.parse().ok()?;
    let minor = it.next().unwrap_or("0").parse().ok()?;
    let patch = it.next().unwrap_or("0").parse().ok()?;
    if it.next().is_some() {
        return None;
    }
    Some((major, minor, patch))
}

fn cmp_semver(a: &str, b: &str) -> std::cmp::Ordering {
    parse_semver(a)
        .unwrap_or((0, 0, 0))
        .cmp(&parse_semver(b).unwrap_or((0, 0, 0)))
}

/// Does version `ver` satisfy the semver range `range`?
fn semver_satisfies(ver: &str, range: &str) -> bool {
    let Some(v) = parse_semver(ver) else {
        return false;
    };
    range.split("||").any(|clause| {
        let clause = clause.trim();
        clause.is_empty()
            || clause == "*"
            || clause.split_whitespace().all(|c| comparator_matches(v, c))
    })
}

fn comparator_matches(v: (u64, u64, u64), c: &str) -> bool {
    let c = c.trim();
    if let Some(rest) = c.strip_prefix("^")
        && let Some(lo) = parse_semver(rest)
    {
        let hi = if lo.0 > 0 {
            (lo.0 + 1, 0, 0)
        } else if lo.1 > 0 {
            (0, lo.1 + 1, 0)
        } else {
            (0, 0, lo.2 + 1)
        };
        return v >= lo && v < hi;
    }
    if let Some(rest) = c.strip_prefix("~")
        && let Some(lo) = parse_semver(rest)
    {
        let hi = (lo.0, lo.1 + 1, 0);
        return v >= lo && v < hi;
    }
    for (op, len) in [(">=", 2), ("<=", 2), (">", 1), ("<", 1), ("=", 1)] {
        if let Some(rest) = c.strip_prefix(op) {
            let _ = len;
            if let Some(w) = parse_semver(rest) {
                return match op {
                    ">=" => v >= w,
                    "<=" => v <= w,
                    ">" => v > w,
                    "<" => v < w,
                    _ => v == w,
                };
            }
        }
    }
    if c == "*" || c == "x" {
        return true;
    }
    if c.contains('x') || c.contains('*') {
        let parts: Vec<&str> = c.split('.').collect();
        let get = |i: usize| parts.get(i).copied().unwrap_or("*");
        let wild = |s: &str| s == "x" || s == "*";
        if !wild(get(0)) && get(0).parse::<u64>() != Ok(v.0) {
            return false;
        }
        if parts.len() >= 2 && !wild(get(1)) && get(1).parse::<u64>() != Ok(v.1) {
            return false;
        }
        return true;
    }
    parse_semver(c) == Some(v)
}

fn fetch(name: &str, src: &Source, r: &Resolved) -> Result<(), String> {
    let dir = PathBuf::from(MODULES_DIR).join(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    match src {
        Source::Git { url, .. } => git_fetch_into(url, r.git_sha.as_deref(), &dir),
        _ => {
            let tarball = r.tarball.as_deref().ok_or("registry gave no tarball url")?;
            download_tgz(tarball, &dir)
        }
    }
}

/// Download a `.tgz` and extract it, stripping the archive's top-level dir (npm tarballs wrap everything in `package/`).
fn download_tgz(url: &str, dir: &Path) -> Result<(), String> {
    let tmp = dir.join(".pkg.tgz");
    run("curl", &["-fsSL", "-o", &tmp.to_string_lossy(), url])?;
    run(
        "tar",
        &[
            "-xzf",
            &tmp.to_string_lossy(),
            "-C",
            &dir.to_string_lossy(),
            "--strip-components=1",
        ],
    )?;
    let _ = std::fs::remove_file(&tmp);
    Ok(())
}

fn git_fetch_into(url: &str, sha: Option<&str>, dir: &Path) -> Result<(), String> {
    run("git", &["clone", "--quiet", url, &dir.to_string_lossy()])?;
    if let Some(sha) = sha {
        run(
            "git",
            &["-C", &dir.to_string_lossy(), "checkout", "--quiet", sha],
        )?;
    }
    let _ = std::fs::remove_dir_all(dir.join(".git"));
    Ok(())
}

fn git_ls_remote(url: &str, reff: Option<&str>) -> Result<String, String> {
    let out = Command::new("git")
        .args(["ls-remote", url, reff.unwrap_or("HEAD")])
        .output()
        .map_err(|e| format!("git: {e}"))?;
    if !out.status.success() {
        return Err(format!(
            "git ls-remote failed: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .next()
        .and_then(|l| l.split_whitespace().next())
        .map(str::to_string)
        .ok_or_else(|| format!("no ref `{}` in {url}", reff.unwrap_or("HEAD")))
}

fn self_replace(url: &str) -> Result<(), String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let tmp = exe.with_extension("new");
    run("curl", &["-fsSL", "-o", &tmp.to_string_lossy(), url])?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(0o755));
    }
    std::fs::rename(&tmp, &exe).map_err(|e| format!("cannot replace {}: {e}", exe.display()))?;
    Ok(())
}

fn target_triple() -> &'static str {
    if cfg!(all(target_os = "linux", target_arch = "x86_64")) {
        "x86_64-linux"
    } else if cfg!(all(target_os = "linux", target_arch = "aarch64")) {
        "aarch64-linux"
    } else if cfg!(all(target_os = "macos", target_arch = "x86_64")) {
        "x86_64-macos"
    } else if cfg!(all(target_os = "macos", target_arch = "aarch64")) {
        "aarch64-macos"
    } else if cfg!(target_os = "windows") {
        "x86_64-windows"
    } else {
        "unknown"
    }
}

fn ensure_manifest() {
    if !Path::new(MANIFEST).exists() {
        let _ = std::fs::write(MANIFEST, "[package]\nname = \"app\"\n\n[dependencies]\n");
    }
}

/// Read `[dependencies]` off the manifest chain as `(name, spec)` pairs.
pub fn manifest_deps() -> Vec<(String, String)> {
    crate::manifest::Chain::here()
        .table("[dependencies]")
        .into_iter()
        .map(|(k, v)| (k, crate::manifest::unquote(&v).to_string()))
        .filter(|(_, v)| !v.is_empty())
        .collect()
}

/// Insert or replace `name = "spec"` inside `[dependencies]`, creating the section if needed.
fn manifest_put(name: &str, spec: &str) -> Result<(), String> {
    let existing = std::fs::read_to_string(MANIFEST).unwrap_or_default();
    let line = format!("{name} = \"{spec}\"");
    let mut lines: Vec<String> = existing.lines().map(str::to_string).collect();

    let deps_at = lines.iter().position(|l| l.trim() == "[dependencies]");
    match deps_at {
        Some(start) => {
            let end = lines[start + 1..]
                .iter()
                .position(|l| l.trim().starts_with('['))
                .map(|p| start + 1 + p)
                .unwrap_or(lines.len());
            let key = format!("{name} =");
            if let Some(i) = lines[start + 1..end]
                .iter()
                .position(|l| l.trim().starts_with(&key))
            {
                lines[start + 1 + i] = line;
            } else {
                lines.insert(end, line);
            }
        }
        None => {
            if !existing.is_empty() && !existing.ends_with('\n') {
                lines.push(String::new());
            }
            lines.push("[dependencies]".into());
            lines.push(line);
        }
    }
    let mut out = lines.join("\n");
    out.push('\n');
    std::fs::write(MANIFEST, out).map_err(|e| e.to_string())
}

/// Append or replace a package entry in `maca.lock`.
fn lock_put(name: &str, spec: &str, src: &Source, r: &Resolved) -> Result<(), String> {
    let mut entries = read_lock();
    let source = match src {
        Source::Npm { .. } => format!("npm:{name}"),
        Source::Git { url, .. } => format!("git+{url}"),
        Source::Registry { .. } => "registry".to_string(),
    };
    entries.retain(|(n, _)| n != name);
    let mut block = String::new();
    block.push_str(&format!("name = \"{name}\"\n"));
    block.push_str(&format!("version = \"{}\"\n", r.version));
    block.push_str(&format!("request = \"{spec}\"\n"));
    block.push_str(&format!("source = \"{source}\"\n"));
    if let Some(t) = &r.tarball {
        block.push_str(&format!("resolved = \"{t}\"\n"));
    }
    if let Some(i) = &r.integrity {
        block.push_str(&format!("integrity = \"{i}\"\n"));
    }
    if let Some(s) = &r.git_sha {
        block.push_str(&format!("commit = \"{s}\"\n"));
    }
    entries.push((name.to_string(), block));
    entries.sort_by(|a, b| a.0.cmp(&b.0));

    let mut out =
        String::from("# maca.lock: generated by `maca add`/`maca update`; do not edit.\n");
    for (_, block) in &entries {
        out.push_str("\n[[package]]\n");
        out.push_str(block);
    }
    std::fs::write(LOCK_FILE, out).map_err(|e| e.to_string())
}

/// Parse `maca.lock` into `(name, raw-block)` entries.
fn read_lock() -> Vec<(String, String)> {
    let Ok(text) = std::fs::read_to_string(LOCK_FILE) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut name = String::new();
    let flush = |name: &mut String, cur: &mut String, out: &mut Vec<(String, String)>| {
        if !name.is_empty() {
            out.push((std::mem::take(name), std::mem::take(cur)));
        } else {
            cur.clear();
        }
    };
    for line in text.lines() {
        let t = line.trim();
        if t == "[[package]]" {
            flush(&mut name, &mut cur, &mut out);
            continue;
        }
        if t.starts_with('#') || t.is_empty() {
            continue;
        }
        if let Some(v) = t.strip_prefix("name =") {
            name = v.trim().trim_matches('"').to_string();
        }
        cur.push_str(line);
        cur.push('\n');
    }
    flush(&mut name, &mut cur, &mut out);
    out
}

fn http_get(url: impl AsRef<str>) -> Result<String, String> {
    let out = Command::new("curl")
        .args(["-fsSL", "-A", "maca-cli", url.as_ref()])
        .output()
        .map_err(|e| format!("curl: {e}"))?;
    if !out.status.success() {
        return Err(format!("HTTP request failed ({})", url.as_ref()));
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

fn run(cmd: &str, args: &[&str]) -> Result<(), String> {
    let out = Command::new(cmd)
        .args(args)
        .output()
        .map_err(|e| format!("{cmd}: {e}"))?;
    if !out.status.success() {
        return Err(format!(
            "{cmd} failed: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    Ok(())
}

/// URL-encode a package path (keep `/` and `@` for scoped npm names).
fn enc(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            'A'..='Z' | 'a'..='z' | '0'..='9' | '-' | '_' | '.' | '~' | '/' | '@' => c.to_string(),
            _ => format!("%{:02X}", c as u32),
        })
        .collect()
}

fn short(sha: &str) -> String {
    sha.chars().take(12).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn semver_ranges_match() {
        assert!(semver_satisfies("1.4.2", "^1.2.0"));
        assert!(!semver_satisfies("2.0.0", "^1.2.0"));
        assert!(semver_satisfies("0.9.9", "^0.9.1"));
        assert!(!semver_satisfies("0.10.0", "^0.9.1"));
        assert!(semver_satisfies("1.2.9", "~1.2.3"));
        assert!(!semver_satisfies("1.3.0", "~1.2.3"));
        assert!(semver_satisfies("2.5.0", ">=2 <3"));
        assert!(!semver_satisfies("3.0.0", ">=2 <3"));
        assert!(semver_satisfies("4.1.0", "^1.0.0 || ^4.0.0"));
        assert!(semver_satisfies("1.9.9", "1.x"));
        assert!(!semver_satisfies("2.0.0", "1.x"));
        assert!(semver_satisfies("1.2.3", "1.2.3"));
        assert!(!semver_satisfies("1.2.4", "1.2.3"));
    }

    #[test]
    fn semver_picks_the_highest() {
        let vers = ["1.0.0", "1.2.0", "1.4.9", "2.0.0"];
        let best = vers
            .iter()
            .filter(|v| semver_satisfies(v, "^1.1"))
            .max_by(|a, b| cmp_semver(a, b));
        assert_eq!(best, Some(&"1.4.9"));
    }

    #[test]
    fn parse_npm_specs() {
        assert_eq!(
            parse_spec("npm:axios").unwrap(),
            (
                "axios".into(),
                Source::Npm {
                    pkg: "axios".into(),
                    req: "latest".into()
                }
            )
        );
        assert_eq!(
            parse_spec("npm:axios@1.6.0").unwrap(),
            (
                "axios".into(),
                Source::Npm {
                    pkg: "axios".into(),
                    req: "1.6.0".into()
                }
            )
        );
        assert_eq!(
            parse_spec("npm:@scope/pkg@2.0.0").unwrap(),
            (
                "pkg".into(),
                Source::Npm {
                    pkg: "@scope/pkg".into(),
                    req: "2.0.0".into()
                }
            )
        );
    }

    #[test]
    fn parse_git_specs() {
        assert_eq!(
            parse_spec("git+https://github.com/u/lib.git").unwrap(),
            (
                "lib".into(),
                Source::Git {
                    url: "https://github.com/u/lib.git".into(),
                    reff: None
                }
            )
        );
        assert_eq!(
            parse_spec("git+https://github.com/u/lib#main").unwrap(),
            (
                "lib".into(),
                Source::Git {
                    url: "https://github.com/u/lib".into(),
                    reff: Some("main".into())
                }
            )
        );
    }

    #[test]
    fn parse_registry_specs() {
        assert_eq!(
            parse_spec("utils").unwrap(),
            (
                "utils".into(),
                Source::Registry {
                    name: "utils".into(),
                    req: "latest".into()
                }
            )
        );
        assert_eq!(
            parse_spec("utils@^1.2.0").unwrap(),
            (
                "utils".into(),
                Source::Registry {
                    name: "utils".into(),
                    req: "^1.2.0".into()
                }
            )
        );
    }

    #[test]
    fn manifest_put_creates_and_updates() {
        let dir = std::env::temp_dir().join(format!("maca-dep-test-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let prev = std::env::current_dir().unwrap();
        std::env::set_current_dir(&dir).unwrap();

        std::fs::write(MANIFEST, "[package]\nname = \"x\"\n").unwrap();
        manifest_put("axios", "npm:axios").unwrap();
        let t = std::fs::read_to_string(MANIFEST).unwrap();
        assert!(t.contains("[dependencies]"), "{t}");
        assert!(t.contains("axios = \"npm:axios\""), "{t}");

        manifest_put("axios", "npm:axios@2.0.0").unwrap();
        let t = std::fs::read_to_string(MANIFEST).unwrap();
        assert_eq!(t.matches("axios =").count(), 1, "{t}");
        assert!(t.contains("npm:axios@2.0.0"), "{t}");

        let deps = manifest_deps();
        assert_eq!(deps, vec![("axios".into(), "npm:axios@2.0.0".into())]);

        std::env::set_current_dir(prev).unwrap();
        let _ = std::fs::remove_dir_all(&dir);
    }
}
