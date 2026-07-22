//! `maca` — the Maca toolchain CLI.
//!
//! Commands: `init` (scaffold), `build`/`run` (compile a `.maca` program to C
//! and link a native binary), `watch` (rebuild+rerun on change), `fmt` (indent
//! normalization, style from `maca.toml [format]`), `lint` (style + type
//! diagnostics). Codegen prefers `zig cc` (static musl) through WSL when
//! present; otherwise it falls back to the host's native `cc`/`clang`.

use std::path::{Path, PathBuf};
use std::process::Command;

const VERSION: &str = env!("CARGO_PKG_VERSION");

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("--version" | "-V" | "version") => println!("maca {VERSION}"),
        Some("build") => cmd_build(&args[1..]),
        Some("run") => cmd_run(&args[1..]),
        Some("init") => cmd_init(&args[1..]),
        Some("fmt") => cmd_fmt(&args[1..]),
        Some("lint") => cmd_lint(&args[1..]),
        Some("watch") => cmd_watch(&args[1..]),
        Some("--help" | "-h" | "help") | None => usage(),
        Some(other) => {
            // maca.toml [scripts] alias?
            if let Some(cmd) = script_alias(other) {
                run_script(&cmd);
            } else {
                eprintln!("maca: unknown command `{other}`\n");
                usage();
                std::process::exit(2);
            }
        }
    }
}

/// Formatting style. Defaults match the codebase: 4-space soft indent.
struct FmtStyle {
    unit: String, // one indent level's whitespace
}

/// Read `[format]` from `maca.toml` (indent_style = "space"|"tab", indent_size = N).
fn format_style() -> FmtStyle {
    let mut style = "space".to_string();
    let mut size = 4usize;
    if let Ok(toml) = std::fs::read_to_string("maca.toml") {
        let mut in_fmt = false;
        for line in toml.lines() {
            let t = line.trim();
            if t.starts_with('[') {
                in_fmt = t == "[format]";
                continue;
            }
            if in_fmt {
                if let Some((k, v)) = t.split_once('=') {
                    let v = v.trim().trim_matches('"');
                    match k.trim() {
                        "indent_style" => style = v.to_string(),
                        "indent_size" => size = v.parse().unwrap_or(4),
                        _ => {}
                    }
                }
            }
        }
    }
    let unit = if style == "tab" { "\t".to_string() } else { " ".repeat(size) };
    FmtStyle { unit }
}

fn cmd_fmt(args: &[String]) {
    let check = args.iter().any(|a| a == "--check");
    let files: Vec<PathBuf> =
        args.iter().filter(|a| !a.starts_with("--")).map(PathBuf::from).collect();
    if files.is_empty() {
        die("fmt: expected one or more .maca files (use --check to verify only)");
    }
    let style = format_style();
    let mut unformatted = Vec::new();
    for src in &files {
        let source =
            std::fs::read_to_string(src).unwrap_or_else(|e| die(&format!("cannot read: {e}")));
        // Parse only to reject broken files — never reformat through the AST
        // (the lexer drops comments, so a print-based `fmt` would delete them).
        // `fmt` re-indents the *original* text by bracket depth, preserving all
        // content: comments, blank lines, and intra-line spacing.
        let parsed = maca_parser::parse(&source);
        if !parsed.errors.is_empty() {
            die(&format!("fmt: {}: parse errors:\n  {}", src.display(), parsed.errors.join("\n  ")));
        }
        let formatted = reindent(&source, &style.unit);
        if check {
            if formatted != source {
                unformatted.push(src.display().to_string());
            }
        } else if formatted != source {
            std::fs::write(src, &formatted).unwrap_or_else(|e| die(&format!("cannot write: {e}")));
            println!("formatted {}", src.display());
        }
    }
    if check && !unformatted.is_empty() {
        eprintln!("not formatted:\n  {}", unformatted.join("\n  "));
        std::process::exit(1);
    }
}

/// Scaffold a new Maca project: `maca.toml`, `main.maca`, `.gitignore`.
fn cmd_init(args: &[String]) {
    let dir = args.iter().find(|a| !a.starts_with('-')).map(PathBuf::from);
    let root = dir.clone().unwrap_or_else(|| PathBuf::from("."));
    if let Some(d) = &dir {
        std::fs::create_dir_all(d).unwrap_or_else(|e| die(&format!("cannot create {}: {e}", d.display())));
    }
    let name = root
        .canonicalize()
        .ok()
        .and_then(|p| p.file_name().map(|s| s.to_string_lossy().into_owned()))
        .unwrap_or_else(|| "app".into());

    let toml = format!(
        "[package]\nname = \"{name}\"\nversion = \"0.1.0\"\n\n\
         [[bin]]\nname = \"{name}\"\npath = \"main.maca\"\n\n\
         # Formatting — `maca fmt` reads these (defaults shown).\n\
         [format]\nindent_style = \"space\"\nindent_size = 4\n\n\
         # `maca <name>` runs a script alias.\n\
         [scripts]\nstart = \"maca run main.maca\"\n"
    );
    let main = "main() -> int {\n    info(\"Hello from Maca\")\n    0\n}\n";
    let gitignore = "/target\n/build\n*.o\n";

    write_if_absent(&root.join("maca.toml"), &toml);
    write_if_absent(&root.join("main.maca"), main);
    write_if_absent(&root.join(".gitignore"), gitignore);
    println!("initialized Maca project `{name}` in {}", root.display());
    println!("  maca run main.maca   # build & run");
}

fn write_if_absent(path: &Path, contents: &str) {
    if path.exists() {
        println!("  skip {} (exists)", path.display());
        return;
    }
    std::fs::write(path, contents).unwrap_or_else(|e| die(&format!("cannot write {}: {e}", path.display())));
    println!("  create {}", path.display());
}

/// Hot reload: rebuild + rerun whenever the source (or its directory) changes.
/// Polls mtimes — no extra deps, works anywhere `maca run` does.
fn cmd_watch(args: &[String]) {
    let Some(src) = args.first().map(PathBuf::from) else {
        die("watch: expected a .maca file");
    };
    let prog_args: Vec<String> = args[1..].to_vec();
    println!("watching {} — Ctrl-C to stop", src.display());
    let mut last = std::time::SystemTime::UNIX_EPOCH;
    loop {
        let changed = std::fs::metadata(&src)
            .and_then(|m| m.modified())
            .map(|m| m > last)
            .unwrap_or(false);
        if changed {
            last = std::fs::metadata(&src).and_then(|m| m.modified()).unwrap();
            println!("\x1b[2m── change detected, rebuilding ─────────────\x1b[0m");
            let mut run = vec!["run".to_string(), src.to_string_lossy().into_owned()];
            run.extend(prog_args.iter().cloned());
            let status = Command::new(std::env::current_exe().unwrap())
                .args(&run)
                .status();
            match status {
                Ok(s) => println!("\x1b[2m── exited {} ───────────────────────────────\x1b[0m", s.code().unwrap_or(-1)),
                Err(e) => eprintln!("watch: {e}"),
            }
        }
        std::thread::sleep(std::time::Duration::from_millis(300));
    }
}

fn cmd_lint(args: &[String]) {
    let Some(src) = args.first().map(PathBuf::from) else {
        die("lint: expected a .maca file");
    };
    let source = std::fs::read_to_string(&src).unwrap_or_else(|e| die(&format!("cannot read: {e}")));
    let mut issues: Vec<String> = Vec::new();

    // style: line width (strings excluded from the count is future; flag raw >80)
    for (i, line) in source.lines().enumerate() {
        if line.chars().count() > 80 && !line.trim_start().starts_with("//") {
            issues.push(format!("{}:{}: line exceeds 80 columns", src.display(), i + 1));
        }
        // forced block breaks: no single-line statement block `if c { .. }`
        if line.contains("if ") && line.contains('{') && line.contains('}') && !line.contains("? ") {
            issues.push(format!("{}:{}: single-line `if` block; break it across lines", src.display(), i + 1));
        }
    }
    // semantic diagnostics
    let parsed = maca_parser::parse(&source);
    for d in maca_core::check(&parsed.module, maca_core::Mode::Program) {
        issues.push(format!("{}: {:?}: {}", src.display(), d.kind, d.msg));
    }

    if issues.is_empty() {
        println!("{}: no issues", src.display());
    } else {
        for i in &issues {
            eprintln!("{i}");
        }
        std::process::exit(1);
    }
}

/// Brace/paren-depth re-indenter (4-space), string-aware.
/// Normalize indentation to `unit` without reflowing. Each line's existing
/// indent depth (in levels, inferred from the file's own indent step) is
/// re-emitted with `unit` — so a 4-space file re-formatted with the default
/// 4-space style is unchanged (idempotent), while tab↔space / size changes
/// convert cleanly. This preserves every author choice that isn't pure
/// leading whitespace: comments, blank lines, and expression-continuation
/// alignment (`=>` bodies, ternary chains) all survive intact.
fn reindent(src: &str, unit: &str) -> String {
    // Detect the file's indent step as the gcd of all leading-whitespace widths
    // (a tab counts as one column here — enough to recover level counts).
    let widths: Vec<usize> = src
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| l.len() - l.trim_start().len())
        .filter(|&w| w > 0)
        .collect();
    let step = widths.iter().copied().reduce(gcd).filter(|&s| s > 0).unwrap_or(4);

    let mut out = String::new();
    for raw in src.lines() {
        if raw.trim().is_empty() {
            out.push('\n');
            continue;
        }
        let lead = raw.len() - raw.trim_start().len();
        let levels = lead / step;
        for _ in 0..levels {
            out.push_str(unit);
        }
        out.push_str(raw.trim_start());
        out.push('\n');
    }
    out
}

fn gcd(a: usize, b: usize) -> usize {
    if b == 0 { a } else { gcd(b, a % b) }
}

/// Look up a `[scripts]` alias in ./maca.toml.
fn script_alias(name: &str) -> Option<String> {
    let toml = std::fs::read_to_string("maca.toml").ok()?;
    let mut in_scripts = false;
    for line in toml.lines() {
        let t = line.trim();
        if t.starts_with('[') {
            in_scripts = t == "[scripts]";
            continue;
        }
        if in_scripts {
            if let Some((k, v)) = t.split_once('=') {
                if k.trim() == name {
                    return Some(v.trim().trim_matches('"').to_string());
                }
            }
        }
    }
    None
}

fn run_script(cmd: &str) {
    // `maca <alias>` runs its command through the shell
    let status = if cfg!(windows) {
        Command::new("cmd").args(["/C", cmd]).status()
    } else {
        Command::new("sh").args(["-c", cmd]).status()
    };
    let code = status.map(|s| s.code().unwrap_or(1)).unwrap_or(1);
    std::process::exit(code);
}

fn usage() {
    println!(
        "maca {VERSION}\n\
         \n\
         usage: maca <command> [args]\n\
         \n\
         commands:\n\
         \x20 init  [dir]                  scaffold a new project (maca.toml, main.maca)\n\
         \x20 build <file.maca> [-o out]   compile to a native binary\n\
         \x20 run   <file.maca> [args..]   compile and run\n\
         \x20 watch <file.maca> [args..]   rebuild & rerun on change (hot reload)\n\
         \x20 fmt   <file.maca>… [--check] format in place (style from maca.toml [format])\n\
         \x20 lint  <file.maca>            style + type/effect diagnostics\n\
         \x20 --version                    print the toolchain version"
    );
}

fn cmd_build(args: &[String]) {
    let mut src = None;
    let mut out = None;
    let mut target = "native".to_string();
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "-o" => out = it.next().map(PathBuf::from),
            "--target" => target = it.next().cloned().unwrap_or_else(|| "native".into()),
            _ => src = Some(PathBuf::from(a)),
        }
    }
    let Some(src) = src else {
        die("build: expected a .maca file");
    };
    if target == "nix" {
        let out = out.unwrap_or_else(|| PathBuf::from(format!("{}.nix", stem(&src))));
        match build_nix(&src, &out) {
            Ok(()) => println!("built {}", out.display()),
            Err(e) => die(&e),
        }
        return;
    }
    if target == "js" {
        let out = out.unwrap_or_else(|| PathBuf::from(format!("{}-web", stem(&src))));
        match build_js(&src, &out) {
            Ok(()) => println!("built {}/", out.display()),
            Err(e) => die(&e),
        }
        return;
    }
    let out = out.unwrap_or_else(|| PathBuf::from(stem(&src)));
    match compile(&src, &out) {
        Ok(()) => println!("built {}", out.display()),
        Err(e) => die(&e),
    }
}

/// Config mode → a NixOS module. Checked in the pure `<>` config context.
fn build_nix(src: &Path, out: &Path) -> Result<(), String> {
    let source = std::fs::read_to_string(src).map_err(|e| format!("cannot read {}: {e}", src.display()))?;
    let parsed = maca_parser::parse(&source);
    if !parsed.errors.is_empty() {
        return Err(format!("parse errors:\n  {}", parsed.errors.join("\n  ")));
    }
    let diags = maca_core::check(&parsed.module, maca_core::Mode::Config);
    if !diags.is_empty() {
        let msgs: Vec<_> = diags.iter().map(|d| format!("{:?}: {}", d.kind, d.msg)).collect();
        return Err(format!("config errors:\n  {}", msgs.join("\n  ")));
    }
    let nix = maca_backend_nix::emit(&parsed.module);
    std::fs::write(out, nix).map_err(|e| e.to_string())?;
    Ok(())
}

/// UI mode → JS + HTML + CSS in an output directory.
fn build_js(src: &Path, out_dir: &Path) -> Result<(), String> {
    let source = std::fs::read_to_string(src).map_err(|e| format!("cannot read {}: {e}", src.display()))?;
    let parsed = maca_parser::parse(&source);
    if !parsed.errors.is_empty() {
        return Err(format!("parse errors:\n  {}", parsed.errors.join("\n  ")));
    }
    let out = maca_backend_js::emit(&parsed.module);
    std::fs::create_dir_all(out_dir).map_err(|e| e.to_string())?;
    std::fs::write(out_dir.join("app.js"), out.js).map_err(|e| e.to_string())?;
    std::fs::write(out_dir.join("index.html"), out.html).map_err(|e| e.to_string())?;
    std::fs::write(out_dir.join("app.css"), out.css).map_err(|e| e.to_string())?;
    Ok(())
}

fn cmd_run(args: &[String]) {
    let Some(src) = args.first().map(PathBuf::from) else {
        die("run: expected a .maca file");
    };
    let prog_args = &args[1..];
    let dir = build_dir(&src);
    let out = dir.join(stem(&src));
    if let Err(e) = compile(&src, &out) {
        die(&e);
    }
    // Run the produced binary — natively when there's no WSL, else through WSL.
    let status = if have_wsl() {
        Command::new("wsl").arg(to_wsl(&out)).args(prog_args).status()
    } else {
        Command::new(&out).args(prog_args).status()
    }
    .unwrap_or_else(|e| die(&format!("failed to launch binary: {e}")));
    std::process::exit(status.code().unwrap_or(1));
}

/// Read → parse → typecheck → emit C → `zig cc` → binary at `out`.
fn compile(src: &Path, out: &Path) -> Result<(), String> {
    let source = std::fs::read_to_string(src).map_err(|e| format!("cannot read {}: {e}", src.display()))?;

    let mut parsed = maca_parser::parse(&source);
    if !parsed.errors.is_empty() {
        return Err(format!("parse errors:\n  {}", parsed.errors.join("\n  ")));
    }
    inject_nix_imports(&mut parsed.module, src)?;
    let diags = maca_core::check(&parsed.module, maca_core::Mode::Program);
    if !diags.is_empty() {
        let msgs: Vec<_> = diags.iter().map(|d| format!("{:?}: {}", d.kind, d.msg)).collect();
        return Err(format!("type errors:\n  {}", msgs.join("\n  ")));
    }

    let c_src = maca_backend_c::emit(&parsed.module);
    let use_async = maca_backend_c::needs_async(&c_src);
    let llvm = maca_backend_llvm::emit(&parsed.module);
    let use_simd = !llvm.simd_fns.is_empty();
    let dir = build_dir(src);
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    std::fs::write(dir.join("main.c"), &c_src).map_err(|e| e.to_string())?;
    maca_runtime::write_to(&dir).map_err(|e| e.to_string())?;

    // C FFI: link a real library (native dynamic build with rpath, since a
    // glibc lib can't static-link into a musl binary).
    let c_imports = maca_backend_c::c_imports(&parsed.module);
    if c_imports.iter().any(|h| h.contains("sqlite")) {
        maca_runtime::write_sqlite_glue(&dir).map_err(|e| e.to_string())?;
        let dev = nix_out("nixpkgs#sqlite.dev")?;
        let lib = nix_out("nixpkgs#sqlite.out")?;
        let mut args: Vec<String> =
            ["nix", "shell", "nixpkgs#zig", "-c", "zig", "cc"].iter().map(|s| s.to_string()).collect();
        args.push(to_wsl(&dir.join("main.c")));
        args.push(to_wsl(&dir.join("maca_runtime.c")));
        args.push(to_wsl(&dir.join("maca_ffi_sqlite.c")));
        args.push(format!("-I{dev}/include"));
        args.push(format!("-L{lib}/lib"));
        args.push("-lsqlite3".into());
        args.push(format!("-Wl,-rpath,{lib}/lib"));
        args.push("-O2".into());
        args.push("-o".into());
        args.push(to_wsl(out));
        let o = Command::new("wsl")
            .args(&args)
            .output()
            .map_err(|e| format!("failed to run zig via wsl: {e}"))?;
        if !o.status.success() {
            return Err(format!("ffi build failed:\n{}", String::from_utf8_lossy(&o.stderr)));
        }
        return Ok(());
    }

    // Python FFI (feature-gated): embeds CPython.
    if parsed.module.items.iter().any(|it| {
        matches!(it, maca_parser::Stmt::Import(maca_parser::Import::Foreign { lang, .. }) if lang == "py")
    }) {
        maca_runtime::write_py_glue(&dir).map_err(|e| e.to_string())?;
        let py = nix_out("nixpkgs#python3")?;
        let inc = wsl_capture(&format!("ls -d {py}/include/python3* | head -1"))?;
        let lname = wsl_capture(&format!(
            "basename $(ls {py}/lib/libpython3*.so | head -1) .so | sed 's/^lib//'"
        ))?;
        let mut args: Vec<String> =
            ["nix", "shell", "nixpkgs#zig", "-c", "zig", "cc"].iter().map(|s| s.to_string()).collect();
        args.push(to_wsl(&dir.join("main.c")));
        args.push(to_wsl(&dir.join("maca_runtime.c")));
        args.push(to_wsl(&dir.join("maca_ffi_py.c")));
        args.push(format!("-I{inc}"));
        args.push(format!("-L{py}/lib"));
        args.push(format!("-l{lname}"));
        args.push(format!("-Wl,-rpath,{py}/lib"));
        args.push("-O2".into());
        args.push("-o".into());
        args.push(to_wsl(out));
        let o = Command::new("wsl")
            .args(&args)
            .output()
            .map_err(|e| format!("failed to run zig via wsl: {e}"))?;
        if !o.status.success() {
            return Err(format!("py ffi build failed:\n{}", String::from_utf8_lossy(&o.stderr)));
        }
        return Ok(());
    }

    // std/mqtt (import c "mqtt.h"): libc sockets only → links into the static
    // musl build alongside the runtime.
    let use_mqtt = c_imports.iter().any(|h| h.contains("mqtt"));
    if use_mqtt {
        maca_runtime::write_mqtt_glue(&dir).map_err(|e| e.to_string())?;
    }
    if use_async {
        maca_runtime::write_async(&dir).map_err(|e| e.to_string())?;
    }
    if use_simd {
        std::fs::write(dir.join("simd.ll"), &llvm.ir).map_err(|e| e.to_string())?;
    }

    // No WSL/zig? Link with the host's native cc (plain builds only).
    if !have_wsl() {
        return link_native(&dir, out, use_async, use_mqtt, use_simd);
    }

    // Only the async translation unit is linked when needed, so a sequential
    // binary carries no scheduler symbols.
    let mut args: Vec<String> = ["nix", "shell", "nixpkgs#zig", "-c", "zig", "cc"]
        .iter()
        .map(|s| s.to_string())
        .collect();
    args.push(to_wsl(&dir.join("main.c")));
    args.push(to_wsl(&dir.join("maca_runtime.c")));
    if use_async {
        args.push(to_wsl(&dir.join("maca_async.c")));
    }
    if use_mqtt {
        args.push(to_wsl(&dir.join("maca_ffi_mqtt.c")));
    }
    if use_async || use_mqtt {
        args.push("-pthread".into());
    }
    if use_simd {
        // the LLVM IR object (clang compiles .ll directly); enable AVX
        args.push(to_wsl(&dir.join("simd.ll")));
        args.push("-mavx2".into());
    }
    args.push("-I".into());
    args.push(to_wsl(&dir));
    args.push("-o".into());
    args.push(to_wsl(out));
    for f in ["-O2", "-static", "-target", "x86_64-linux-musl", "-s"] {
        args.push(f.into());
    }
    let output = Command::new("wsl")
        .args(&args)
        .output()
        .map_err(|e| format!("failed to run zig via wsl: {e}"))?;
    if !output.status.success() {
        return Err(format!("zig cc failed:\n{}", String::from_utf8_lossy(&output.stderr)));
    }
    Ok(())
}

fn build_dir(src: &Path) -> PathBuf {
    std::env::temp_dir().join(format!("maca-build-{}", stem(src)))
}

/// `import nix "F"` evaluates F at build time and binds the result (the file
/// stem) as a constant the program can use — the Nix boundary value.
fn inject_nix_imports(m: &mut maca_parser::Module, src: &Path) -> Result<(), String> {
    use maca_parser::{Bind, Expr, Import, Stmt, StrPart};
    let dir = src.parent().unwrap_or(Path::new("."));
    let mut injected = Vec::new();
    for item in &m.items {
        if let Stmt::Import(Import::Foreign { lang, spec }) = item {
            if lang == "nix" {
                let path = dir.join(spec);
                let out = Command::new("wsl")
                    .args(["nix-instantiate", "--eval", &to_wsl(&path)])
                    .output()
                    .map_err(|e| format!("wsl nix-instantiate: {e}"))?;
                if !out.status.success() {
                    return Err(format!("nix eval {spec} failed:\n{}", String::from_utf8_lossy(&out.stderr)));
                }
                let val = String::from_utf8_lossy(&out.stdout).trim().to_string();
                let name = Path::new(spec)
                    .file_stem()
                    .map(|s| s.to_string_lossy().into_owned())
                    .unwrap_or_else(|| "nixval".into());
                let expr = if let Ok(n) = val.parse::<i64>() {
                    Expr::Int(n)
                } else {
                    let text = val.trim_matches('"').to_string();
                    Expr::Str(vec![StrPart::Text(text)])
                };
                injected.push(Stmt::Bind(Bind {
                    is_let: false,
                    target: Expr::Ident(name),
                    tys: vec![],
                    value: expr,
                }));
            }
        }
    }
    for (i, b) in injected.into_iter().enumerate() {
        m.items.insert(i, b);
    }
    Ok(())
}

/// Run a shell command in WSL and capture trimmed stdout.
fn wsl_capture(cmd: &str) -> Result<String, String> {
    let o = Command::new("wsl").args(["sh", "-c", cmd]).output().map_err(|e| format!("wsl: {e}"))?;
    if !o.status.success() {
        return Err(format!("`{cmd}` failed:\n{}", String::from_utf8_lossy(&o.stderr)));
    }
    Ok(String::from_utf8_lossy(&o.stdout).trim().to_string())
}

/// Resolve a nixpkgs attribute to its store path (for FFI include/lib dirs).
fn nix_out(attr: &str) -> Result<String, String> {
    let o = Command::new("wsl")
        .args(["nix", "build", "--no-link", "--print-out-paths", attr])
        .output()
        .map_err(|e| format!("wsl nix: {e}"))?;
    if !o.status.success() {
        return Err(format!("nix build {attr} failed:\n{}", String::from_utf8_lossy(&o.stderr)));
    }
    Ok(String::from_utf8_lossy(&o.stdout).trim().to_string())
}

fn stem(p: &Path) -> String {
    p.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_else(|| "out".into())
}

/// Is a working WSL available? The default toolchain shells out to
/// `wsl … zig cc` (NixOS side); when there's no WSL we fall back to a native
/// `cc`/`clang` so `maca build`/`run` work on a plain Linux host too.
fn have_wsl() -> bool {
    Command::new("wsl").arg("true").output().map(|o| o.status.success()).unwrap_or(false)
}

/// Link the plain (no-FFI) build with the host's native C compiler. `clang` is
/// used when SIMD IR is present (it compiles `.ll`); otherwise `cc`.
fn link_native(
    dir: &Path,
    out: &Path,
    use_async: bool,
    use_mqtt: bool,
    use_simd: bool,
) -> Result<(), String> {
    let cc = if use_simd { "clang" } else { "cc" };
    let mut cmd = Command::new(cc);
    cmd.arg(dir.join("main.c")).arg(dir.join("maca_runtime.c"));
    if use_async {
        cmd.arg(dir.join("maca_async.c"));
    }
    if use_mqtt {
        cmd.arg(dir.join("maca_ffi_mqtt.c"));
    }
    if use_async || use_mqtt {
        cmd.arg("-pthread");
    }
    if use_simd {
        cmd.arg(dir.join("simd.ll")).arg("-mavx2");
    }
    cmd.arg("-I").arg(dir).arg("-O2").arg("-s").arg("-o").arg(out);
    let output = cmd.output().map_err(|e| format!("failed to run {cc}: {e}"))?;
    if !output.status.success() {
        return Err(format!("{cc} failed:\n{}", String::from_utf8_lossy(&output.stderr)));
    }
    Ok(())
}

/// `C:\a\b` → `/mnt/c/a/b` for passing paths across the WSL boundary.
fn to_wsl(p: &Path) -> String {
    let s = p.to_string_lossy().replace('\\', "/");
    let b = s.as_bytes();
    if b.len() >= 2 && b[1] == b':' {
        format!("/mnt/{}{}", (b[0] as char).to_ascii_lowercase(), &s[2..])
    } else {
        s
    }
}

fn die(msg: &str) -> ! {
    eprintln!("maca: {msg}");
    std::process::exit(1);
}
