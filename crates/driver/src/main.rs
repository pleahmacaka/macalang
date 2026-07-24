//! `maca` — the Maca toolchain CLI.
//!
//! Commands: `init` (scaffold), `build`/`run` (compile a `.maca` program to C
//! and link a native binary), `watch` (rebuild+rerun on change), `fmt` (indent
//! normalization, style from `maca.toml [format]`), `lint` (style + type
//! diagnostics). Codegen prefers `zig cc` (static musl) through WSL when
//! present; otherwise it falls back to the host's native `cc`/`clang`.

use std::path::{Path, PathBuf};
use std::process::Command;

use maca_profile as profile;

mod bindgen;
mod build_cache;
mod deps;

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
        Some("profile") => cmd_profile(&args[1..]),
        Some("dev") => cmd_dev(&args[1..]),
        Some("add") => deps::cmd_add(&args[1..]),
        Some("update") => deps::cmd_update(&args[1..]),
        Some("upgrade") => deps::cmd_upgrade(&args[1..]),
        Some("bindgen") => bindgen::cmd_bindgen(&args[1..]),
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
            if in_fmt && let Some((k, v)) = t.split_once('=') {
                let v = v.trim().trim_matches('"');
                match k.trim() {
                    "indent_style" => style = v.to_string(),
                    "indent_size" => size = v.parse().unwrap_or(4),
                    _ => {}
                }
            }
        }
    }
    let unit = if style == "tab" {
        "\t".to_string()
    } else {
        " ".repeat(size)
    };
    FmtStyle { unit }
}

fn cmd_fmt(args: &[String]) {
    let check = args.iter().any(|a| a == "--check");
    let files: Vec<PathBuf> = args
        .iter()
        .filter(|a| !a.starts_with("--"))
        .map(PathBuf::from)
        .collect();
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
            die(&format!(
                "fmt: {}: parse errors:\n  {}",
                src.display(),
                parsed.errors.join("\n  ")
            ));
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
        std::fs::create_dir_all(d)
            .unwrap_or_else(|e| die(&format!("cannot create {}: {e}", d.display())));
    }
    let name = root
        .canonicalize()
        .ok()
        .and_then(|p| p.file_name().map(|s| s.to_string_lossy().into_owned()))
        .unwrap_or_else(|| "app".into());

    let toml = format!(
        "[package]\nname = \"{name}\"\nversion = \"0.1.0\"\n\n\
         [[bin]]\nname = \"{name}\"\npath = \"main.maca\"\n\n\
         # Dependencies — `maca add npm:pkg | git+url | name@ver`.\n\
         [dependencies]\n\n\
         # Formatting — `maca fmt` reads these (defaults shown).\n\
         [format]\nindent_style = \"space\"\nindent_size = 4\n\n\
         # `maca <name>` runs a script alias.\n\
         [scripts]\nstart = \"maca run main.maca\"\n"
    );
    let main = "main() -> int {\n    info(\"Hello from Maca\")\n    0\n}\n";
    let gitignore = "/target\n/build\n*.o\n/maca_modules\n";

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
    std::fs::write(path, contents)
        .unwrap_or_else(|e| die(&format!("cannot write {}: {e}", path.display())));
    println!("  create {}", path.display());
}

/// `maca profile <file> [-o out.svg]` — run under Callgrind and render a flame
/// graph SVG (+ a text profile on stdout). Native-only (needs `cc` + `valgrind`).
fn cmd_profile(args: &[String]) {
    let mut src = None;
    let mut out_svg = None;
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "-o" => out_svg = it.next().map(PathBuf::from),
            _ => src = Some(PathBuf::from(a)),
        }
    }
    let Some(src) = src else {
        die("profile: expected a .maca file")
    };
    if !have("cc") {
        die("profile: needs a native C compiler (cc) on PATH");
    }
    if !have("valgrind") {
        die("profile: needs valgrind on PATH (uses --tool=callgrind)");
    }

    // front-end + emit C
    let source =
        std::fs::read_to_string(&src).unwrap_or_else(|e| die(&format!("cannot read: {e}")));
    let parsed = maca_parser::parse(&source);
    if !parsed.errors.is_empty() {
        die(&format!(
            "profile: parse errors:\n  {}",
            parsed.errors.join("\n  ")
        ));
    }
    let diags = maca_core::check(&parsed.module, maca_core::Mode::Program);
    if !diags.is_empty() {
        let msgs: Vec<_> = diags
            .iter()
            .map(|d| format!("{:?}: {}", d.kind, d.msg))
            .collect();
        die(&format!("profile: type errors:\n  {}", msgs.join("\n  ")));
    }
    let c_src = maca_backend_c::emit(&parsed.module);
    let dir = build_dir(&src);
    std::fs::create_dir_all(&dir).unwrap_or_else(|e| die(&e.to_string()));
    std::fs::write(dir.join("main.c"), &c_src).unwrap_or_else(|e| die(&e.to_string()));
    maca_runtime::write_to(&dir).unwrap_or_else(|e| die(&e.to_string()));

    // build with debug info (no strip), instrumentation-friendly
    let bin = dir.join(stem(&src));
    let build = Command::new("cc")
        .args(["-O2", "-g", "-fno-omit-frame-pointer"])
        .arg(dir.join("main.c"))
        .arg(dir.join("maca_runtime.c"))
        .arg("-I")
        .arg(&dir)
        .arg("-o")
        .arg(&bin)
        .output()
        .unwrap_or_else(|e| die(&format!("cc: {e}")));
    if !build.status.success() {
        die(&format!(
            "profile build failed:\n{}",
            String::from_utf8_lossy(&build.stderr)
        ));
    }

    // run under callgrind
    let cg = dir.join("callgrind.out");
    eprintln!("profiling under callgrind (this is slower than a normal run)…");
    let run = Command::new("valgrind")
        .arg("--tool=callgrind")
        .arg(format!("--callgrind-out-file={}", cg.display()))
        .arg(&bin)
        .output()
        .unwrap_or_else(|e| die(&format!("valgrind: {e}")));
    if !run.status.success() && !cg.exists() {
        die(&format!(
            "callgrind failed:\n{}",
            String::from_utf8_lossy(&run.stderr)
        ));
    }
    let cg_text =
        std::fs::read_to_string(&cg).unwrap_or_else(|e| die(&format!("read callgrind: {e}")));

    // text profile + flame graph
    print!("{}", profile::text_profile(&cg_text));
    let svg = profile::flamegraph_svg(&cg_text);
    let out = out_svg.unwrap_or_else(|| PathBuf::from(format!("{}.svg", stem(&src))));
    std::fs::write(&out, &svg).unwrap_or_else(|e| die(&format!("write svg: {e}")));
    println!("\nflame graph → {}", out.display());
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
                Ok(s) => println!(
                    "\x1b[2m── exited {} ───────────────────────────────\x1b[0m",
                    s.code().unwrap_or(-1)
                ),
                Err(e) => eprintln!("watch: {e}"),
            }
        }
        std::thread::sleep(std::time::Duration::from_millis(300));
    }
}

/// Warn on a Capitalized *local* binding (`A = 1`) — it's a constant by
/// convention, but `const a = …` is clearer. Recurses into nested blocks.
fn lint_capital_consts(stmts: &[maca_parser::Stmt], src: &Path, issues: &mut Vec<String>) {
    use maca_parser::{Expr, Stmt};
    for s in stmts {
        match s {
            Stmt::Bind(b) => {
                if let Expr::Ident(n) = &b.target
                    && n.chars().next().is_some_and(|c| c.is_uppercase())
                {
                    issues.push(format!(
                            "{}: style: `{n}` is a Capitalized constant — prefer `const {} = …` (or lowercase for a variable)",
                            src.display(),
                            n.to_lowercase()
                        ));
                }
            }
            Stmt::Expr(e) => lint_capital_consts_expr(e, src, issues),
            _ => {}
        }
    }
}

fn lint_capital_consts_expr(e: &maca_parser::Expr, src: &Path, issues: &mut Vec<String>) {
    use maca_parser::Expr;
    match e {
        Expr::If { then, els, .. } => {
            lint_capital_consts(then, src, issues);
            if let Some(e) = els {
                lint_capital_consts(e, src, issues);
            }
        }
        Expr::For { body, .. } | Expr::While { body, .. } | Expr::Block(body) => {
            lint_capital_consts(body, src, issues)
        }
        Expr::Match { arms, .. } => {
            for a in arms {
                lint_capital_consts_expr(&a.body, src, issues);
            }
        }
        _ => {}
    }
}

fn cmd_lint(args: &[String]) {
    let Some(src) = args.first().map(PathBuf::from) else {
        die("lint: expected a .maca file");
    };
    let source =
        std::fs::read_to_string(&src).unwrap_or_else(|e| die(&format!("cannot read: {e}")));
    let mut issues: Vec<String> = Vec::new();

    // style: line width (strings excluded from the count is future; flag raw >80)
    for (i, line) in source.lines().enumerate() {
        if line.chars().count() > 80 && !line.trim_start().starts_with("//") {
            issues.push(format!(
                "{}:{}: line exceeds 80 columns",
                src.display(),
                i + 1
            ));
        }
        // forced block breaks: no single-line statement block `if c { .. }`
        if line.contains("if ") && line.contains('{') && line.contains('}') && !line.contains("? ")
        {
            issues.push(format!(
                "{}:{}: single-line `if` block; break it across lines",
                src.display(),
                i + 1
            ));
        }
    }
    // semantic diagnostics
    let parsed = maca_parser::parse(&source);
    for d in maca_core::check(&parsed.module, maca_core::Mode::Program) {
        issues.push(format!("{}: {:?}: {}", src.display(), d.kind, d.msg));
    }

    // style: a Capitalized local binding is a constant by convention, but the
    // explicit `const` reads better. (Type/constructor names stay capitalized.)
    for item in &parsed.module.items {
        if let maca_parser::Stmt::Fn(f) = item
            && let Some(maca_parser::FnBody::Block(stmts)) = &f.body
        {
            lint_capital_consts(stmts, &src, &mut issues);
        }
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
    let step = widths
        .iter()
        .copied()
        .reduce(gcd)
        .filter(|&s| s > 0)
        .unwrap_or(4);

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
        if in_scripts
            && let Some((k, v)) = t.split_once('=')
            && k.trim() == name
        {
            return Some(v.trim().trim_matches('"').to_string());
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
         \x20 build <file.maca> [-o out]   compile (native | --target nix|js|jvm|embedded|tauri)\n\
         \x20 run   <file.maca> [args..]   compile and run\n\
         \x20 dev   [dev.maca] [-o flake]  generate a dev-shell flake.nix from Maca\n\
         \x20 watch <file.maca> [args..]   rebuild & rerun on change (hot reload)\n\
         \x20 fmt   <file.maca>… [--check] format in place (style from maca.toml [format])\n\
         \x20 lint  <file.maca>            style + type/effect diagnostics\n\
         \x20 profile <file.maca> [-o svg] run under callgrind, render a flame graph\n\
         \x20 add   <spec>…               add a dependency (npm:pkg | git+url | name@ver)\n\
         \x20 update                      re-resolve dependencies to latest\n\
         \x20 upgrade                     self-update the maca toolchain\n\
         \x20 bindgen <header.h> [-o f]   generate Maca FFI declarations from a C header\n\
         \x20 --version                    print the toolchain version\n\
         \n\
         build targets: native (default), --target nix | js | jvm | embedded | tauri\n\
         \x20 embedded also takes --mcu cortex-m0|m3|m4|riscv32; jvm takes --cp <jars>"
    );
}

fn cmd_build(args: &[String]) {
    let mut src = None;
    let mut out = None;
    let mut target = "native".to_string();
    let mut explicit_target = false;
    let mut classpath = None;
    let mut mcu = String::new();
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "-o" => out = it.next().map(PathBuf::from),
            "--target" => {
                target = it.next().cloned().unwrap_or_else(|| "native".into());
                explicit_target = true;
            }
            "--cp" | "--classpath" => classpath = it.next().cloned(),
            "--mcu" => mcu = it.next().cloned().unwrap_or_default(),
            _ => src = Some(PathBuf::from(a)),
        }
    }
    let Some(src) = src else {
        die("build: expected a .maca file");
    };
    // Auto-detect config (nix) / UI (js) sources so a bare `maca build` doesn't
    // fall through to the native path and emit confusing cc/linker errors.
    if !explicit_target && let Ok(source) = std::fs::read_to_string(&src) {
        let parsed = maca_parser::parse(&source);
        if let Some((detected, why)) = detect_target(&parsed.module) {
            eprintln!("note: {why}; building --target {detected} (pass --target to override)");
            target = detected.to_string();
        }
    }
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
    if target == "tauri" {
        let out = out.unwrap_or_else(|| PathBuf::from(format!("{}-tauri", stem(&src))));
        match build_tauri(&src, &out) {
            Ok(msg) => println!("{msg}"),
            Err(e) => die(&e),
        }
        return;
    }
    if target == "jvm" || target == "java" {
        match build_jvm(&src, out.as_deref(), classpath.as_deref()) {
            Ok(msg) => println!("{msg}"),
            Err(e) => die(&e),
        }
        return;
    }
    if target == "embedded" || target == "baremetal" || target == "mcu" {
        match build_embedded(&src, out.as_deref(), &mcu) {
            Ok(msg) => println!("{msg}"),
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

/// Infer a non-native build target from the source shape, so a bare
/// `maca build` on a config or UI file does the right thing instead of failing
/// on the native path. Only fires on unambiguous signals.
fn detect_target(m: &maca_parser::Module) -> Option<(&'static str, &'static str)> {
    use maca_parser::{Import, Stmt, Type};
    // config mode: imports nixpkgs
    let imports_nixpkgs = m.items.iter().any(|it| match it {
        Stmt::Import(Import::Module(segs)) => segs.last().map(String::as_str) == Some("nixpkgs"),
        Stmt::Import(Import::Bare(n)) => n == "nixpkgs",
        _ => false,
    });
    if imports_nixpkgs {
        return Some(("nix", "source imports nixpkgs (config mode)"));
    }
    // UI mode: a function returns `Element`
    let returns_element = m.items.iter().any(|it| match it {
        Stmt::Fn(f) => matches!(&f.ret, Some(Type::Name(segs)) if segs.last().map(String::as_str) == Some("Element")),
        _ => false,
    });
    if returns_element {
        return Some(("js", "a view returns Element (reactive-UI mode)"));
    }
    None
}

/// Config mode → a NixOS module. Checked in the pure `<>` config context.
fn build_nix(src: &Path, out: &Path) -> Result<(), String> {
    let source =
        std::fs::read_to_string(src).map_err(|e| format!("cannot read {}: {e}", src.display()))?;
    let parsed = maca_parser::parse(&source);
    if !parsed.errors.is_empty() {
        return Err(format!("parse errors:\n  {}", parsed.errors.join("\n  ")));
    }
    let diags = maca_core::check(&parsed.module, maca_core::Mode::Config);
    if !diags.is_empty() {
        let msgs: Vec<_> = diags
            .iter()
            .map(|d| format!("{:?}: {}", d.kind, d.msg))
            .collect();
        return Err(format!("config errors:\n  {}", msgs.join("\n  ")));
    }
    let nix = maca_backend_nix::emit(&parsed.module);
    std::fs::write(out, nix).map_err(|e| e.to_string())?;
    Ok(())
}

/// JVM target → Java source (and `javac` to `.class` when a JDK is present).
/// The class name is the file stem, capitalized. `out` names the output dir.
fn build_jvm(src: &Path, out: Option<&Path>, classpath: Option<&str>) -> Result<String, String> {
    let source =
        std::fs::read_to_string(src).map_err(|e| format!("cannot read {}: {e}", src.display()))?;
    let parsed = maca_parser::parse(&source);
    if !parsed.errors.is_empty() {
        return Err(format!("parse errors:\n  {}", parsed.errors.join("\n  ")));
    }
    let diags = maca_core::check(&parsed.module, maca_core::Mode::Program);
    if !diags.is_empty() {
        let msgs: Vec<_> = diags
            .iter()
            .map(|d| format!("{:?}: {}", d.kind, d.msg))
            .collect();
        return Err(format!("type errors:\n  {}", msgs.join("\n  ")));
    }
    let class = capitalize(&stem(src));
    let java = maca_backend_jvm::emit(&parsed.module, &class, None);

    let out_dir = out
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(format!("{}-jvm", stem(src))));
    std::fs::create_dir_all(&out_dir).map_err(|e| e.to_string())?;
    let java_path = out_dir.join(format!("{class}.java"));
    std::fs::write(&java_path, &java).map_err(|e| e.to_string())?;

    if have("javac") {
        let mut cmd = Command::new("javac");
        cmd.arg(&java_path).arg("-d").arg(&out_dir);
        if let Some(cp) = classpath {
            cmd.arg("-cp").arg(cp);
        }
        let o = cmd.output().map_err(|e| format!("javac: {e}"))?;
        if !o.status.success() {
            // Interop code often needs external jars (e.g. the Minecraft/Fabric
            // API) that aren't on the CLI classpath — keep the emitted .java and
            // warn rather than fail; a Gradle build compiles it with its deps.
            return Ok(format!(
                "emitted {} (javac could not resolve all types — pass --cp, or build via Gradle):\n{}",
                java_path.display(),
                String::from_utf8_lossy(&o.stderr).trim()
            ));
        }
        Ok(format!(
            "built {} and compiled to {}/{class}.class\n  run: java -cp {} {class}",
            java_path.display(),
            out_dir.display(),
            out_dir.display()
        ))
    } else {
        Ok(format!(
            "emitted {} (no javac on PATH to compile)",
            java_path.display()
        ))
    }
}

fn capitalize(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        Some(f) => f.to_ascii_uppercase().to_string() + c.as_str(),
        None => "Main".into(),
    }
}

/// `maca dev [dev.maca] [-o flake.nix]` — compile a Maca-defined dev
/// environment to a `flake.nix` devShell, replacing a hand-written flake.
fn cmd_dev(args: &[String]) {
    let mut src = PathBuf::from("dev.maca");
    let mut out = PathBuf::from("flake.nix");
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "-o" => out = it.next().map(PathBuf::from).unwrap_or(out),
            _ => src = PathBuf::from(a),
        }
    }
    let source = match std::fs::read_to_string(&src) {
        Ok(s) => s,
        Err(e) => die(&format!("cannot read {}: {e}", src.display())),
    };
    let parsed = maca_parser::parse(&source);
    if !parsed.errors.is_empty() {
        die(&format!("parse errors:\n  {}", parsed.errors.join("\n  ")));
    }
    // dev config is pure (no effects) — check in Config mode
    let diags = maca_core::check(&parsed.module, maca_core::Mode::Config);
    let real: Vec<_> = diags
        .iter()
        // `dev.*` isn't a NixOS option namespace; that diagnostic is expected here
        .filter(|d| !matches!(d.kind, maca_core::DiagKind::UnknownOption))
        .collect();
    if !real.is_empty() {
        let msgs: Vec<_> = real
            .iter()
            .map(|d| format!("{:?}: {}", d.kind, d.msg))
            .collect();
        die(&format!("config errors:\n  {}", msgs.join("\n  ")));
    }
    let flake = maca_backend_nix::emit_flake(&parsed.module);
    if let Err(e) = std::fs::write(&out, flake) {
        die(&e.to_string());
    }
    println!(
        "wrote {} — run `nix develop` to enter the shell",
        out.display()
    );

    // Windows: also emit native dev-env scripts under .maca/dev/ (scoop/choco/
    // winget). The flake above ignores scoop.*/choco.*/winget.*, so Nix hosts
    // are unaffected; Windows hosts get a portable, project-local toolchain.
    if let Some(win) = maca_backend_nix::emit_windows_dev(&parsed.module) {
        let dir = PathBuf::from(".maca").join("dev");
        if let Err(e) = std::fs::create_dir_all(&dir) {
            die(&format!("cannot create {}: {e}", dir.display()));
        }
        let setup = dir.join("setup.ps1");
        let activate = dir.join("activate.ps1");
        if let Err(e) = std::fs::write(&setup, &win.setup) {
            die(&e.to_string());
        }
        if let Err(e) = std::fs::write(&activate, &win.activate) {
            die(&e.to_string());
        }
        println!(
            "wrote {} + {} — Windows native dev env ({})",
            setup.display(),
            activate.display(),
            win.managers.join(", ")
        );
        // On Windows, provision the toolchain immediately (portable scoop into
        // .maca\dev); elsewhere the scripts are emitted for the target host.
        if cfg!(windows) {
            println!("provisioning… (running setup.ps1)");
            let status = std::process::Command::new("powershell")
                .args(["-NoProfile", "-ExecutionPolicy", "Bypass", "-File"])
                .arg(&setup)
                .status();
            match status {
                Ok(s) if s.success() => {
                    println!("done — activate with:  . .\\.maca\\dev\\activate.ps1");
                }
                Ok(s) => eprintln!("setup.ps1 exited with {s} — inspect {}", setup.display()),
                Err(e) => eprintln!("could not run powershell: {e}"),
            }
        } else {
            println!("on Windows, run:  powershell -File {}", setup.display());
        }
    }
}

/// Embedded target → freestanding C + startup + linker script, cross-compiled
/// to a bare-metal firmware image (ELF + raw .bin) with clang/lld.
fn build_embedded(src: &Path, out: Option<&Path>, mcu_name: &str) -> Result<String, String> {
    let source =
        std::fs::read_to_string(src).map_err(|e| format!("cannot read {}: {e}", src.display()))?;
    let parsed = maca_parser::parse(&source);
    if !parsed.errors.is_empty() {
        return Err(format!("parse errors:\n  {}", parsed.errors.join("\n  ")));
    }
    let diags = maca_core::check(&parsed.module, maca_core::Mode::Program);
    if !diags.is_empty() {
        let msgs: Vec<_> = diags
            .iter()
            .map(|d| format!("{:?}: {}", d.kind, d.msg))
            .collect();
        return Err(format!("type errors:\n  {}", msgs.join("\n  ")));
    }
    let mcu = maca_backend_embedded::Mcu::resolve(mcu_name)
        .ok_or_else(|| format!("unknown --mcu {mcu_name:?} (try cortex-m0/m3/m4, riscv32)"))?;

    let out_dir = out
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(format!("{}-fw", stem(src))));
    std::fs::create_dir_all(&out_dir).map_err(|e| e.to_string())?;
    let c_path = out_dir.join("firmware.c");
    let ld_path = out_dir.join("link.ld");
    std::fs::write(&c_path, maca_backend_embedded::emit_c(&parsed.module))
        .map_err(|e| e.to_string())?;
    std::fs::write(&ld_path, maca_backend_embedded::linker_script(&mcu))
        .map_err(|e| e.to_string())?;

    if !have("clang") {
        return Ok(format!(
            "emitted {} + {} for {} (no clang on PATH to cross-compile)",
            c_path.display(),
            ld_path.display(),
            mcu.name
        ));
    }
    let elf = out_dir.join("firmware.elf");
    let o = Command::new("clang")
        .args([
            &format!("--target={}", mcu.triple),
            &format!("-mcpu={}", mcu.cpu),
        ])
        .args([
            "-ffreestanding",
            "-nostdlib",
            "-Os",
            "-ffunction-sections",
            "-fdata-sections",
        ])
        .arg("-fuse-ld=lld")
        .arg(format!("-Wl,-T,{}", ld_path.display()))
        .arg("-Wl,--gc-sections")
        .arg("-o")
        .arg(&elf)
        .arg(&c_path)
        .output()
        .map_err(|e| format!("clang: {e}"))?;
    if !o.status.success() {
        return Err(format!(
            "cross-compile failed:\n{}",
            String::from_utf8_lossy(&o.stderr)
        ));
    }
    // raw binary + size report
    let bin = out_dir.join("firmware.bin");
    if have("llvm-objcopy") {
        let _ = Command::new("llvm-objcopy")
            .args(["-O", "binary"])
            .arg(&elf)
            .arg(&bin)
            .status();
    }
    let size = if have("llvm-size") {
        String::from_utf8_lossy(
            &Command::new("llvm-size")
                .arg(&elf)
                .output()
                .map(|o| o.stdout)
                .unwrap_or_default(),
        )
        .trim()
        .to_string()
    } else {
        String::new()
    };
    Ok(format!(
        "built firmware for {} → {}\n{}\n  flash: 0x{:08X}  ram: 0x{:08X}",
        mcu.name,
        elf.display(),
        size,
        mcu.flash_origin,
        mcu.ram_origin
    ))
}

/// UI mode → one self-contained, deployable `index.html`.
///
/// Styles and the transpiled app are inlined; any `import wasm "path"` asset the
/// program declares is read and embedded as a base64 `<script>` (with id
/// `wasm-b64`) before the app, so a browser playground ships as a single file
/// with no external requests. `maca build --target js app.maca` → deploy it.
fn build_js(src: &Path, out_dir: &Path) -> Result<(), String> {
    let source =
        std::fs::read_to_string(src).map_err(|e| format!("cannot read {}: {e}", src.display()))?;
    let parsed = maca_parser::parse(&source);
    if !parsed.errors.is_empty() {
        return Err(format!("parse errors:\n  {}", parsed.errors.join("\n  ")));
    }
    let out = maca_backend_js::emit(&parsed.module);

    // embed declared binary assets: `import wasm "relpath"` → a base64 script.
    let base = src.parent().unwrap_or(Path::new("."));
    let mut assets = String::new();
    for item in &parsed.module.items {
        if let maca_parser::Stmt::Import(maca_parser::Import::Foreign { lang, spec }) = item
            && lang == "wasm"
        {
            let path = base.join(spec);
            let bytes =
                std::fs::read(&path).map_err(|e| format!("import wasm {}: {e}", path.display()))?;
            assets.push_str(&format!(
                "<script id=\"wasm-b64\" type=\"application/octet-stream\">{}</script>\n",
                base64(&bytes)
            ));
        }
    }

    let title = stem(src);
    let page = format!(
        "<!doctype html>\n<html>\n<head>\n<meta charset=\"utf-8\">\n\
         <meta name=\"viewport\" content=\"width=device-width,initial-scale=1\">\n\
         <title>{title}</title>\n<style>\n{}\n</style>\n</head>\n\
         <body>\n<div id=\"app\"></div>\n{assets}<script>\n{}\n</script>\n</body>\n</html>\n",
        out.css, out.js
    );

    std::fs::create_dir_all(out_dir).map_err(|e| e.to_string())?;
    std::fs::write(out_dir.join("index.html"), page).map_err(|e| e.to_string())?;
    // also emit the modules separately (for non-inline hosting / tooling); the
    // self-contained index.html above is the single-file deployable.
    std::fs::write(out_dir.join("app.js"), &out.js).map_err(|e| e.to_string())?;
    std::fs::write(out_dir.join("app.css"), &out.css).map_err(|e| e.to_string())?;
    Ok(())
}

/// `maca build --target tauri app.maca -o out` — scaffold a complete,
/// `cargo tauri build`-able Tauri v2 desktop app from a Maca UI. Emits:
///   out/dist/         the compiled Maca UI (index.html, app.js, app.css) + a
///                     bridge that exposes `macaInvoke(arg)` to the frontend
///   out/src-tauri/    a Tauri v2 Rust shell (Cargo.toml, tauri.conf.json,
///                     build.rs, src/main.rs) registering a `maca_run` command
///   out/src-tauri/bin/backend   the native `backend.maca` (if present), run by
///                     the command — so the whole app is Maca, Tauri just the shell.
fn build_tauri(src: &Path, out: &Path) -> Result<String, String> {
    let name = sanitize_ident(&stem(src));
    let title = stem(src);
    let dist = out.join("dist");

    // 1. the UI (reuses the JS backend), plus the invoke bridge
    build_js(src, &dist)?;
    let bridge = "// Tauri bridge — call a Maca native command from the UI.\n\
        // `macaInvoke(arg)` runs the bundled `backend` binary with `arg` and\n\
        // resolves to its stdout. Works under Tauri v2; a no-op stub otherwise.\n\
        globalThis.macaInvoke = async (arg) => {\n\
        \x20 const t = globalThis.__TAURI__;\n\
        \x20 if (t && t.core && t.core.invoke) return t.core.invoke('maca_run', { arg: String(arg) });\n\
        \x20 if (t && t.invoke) return t.invoke('maca_run', { arg: String(arg) });\n\
        \x20 return '(no tauri runtime)';\n\
        };\n";
    std::fs::write(dist.join("bridge.js"), bridge).map_err(|e| e.to_string())?;
    // reference the bridge from index.html
    let index = dist.join("index.html");
    if let Ok(html) = std::fs::read_to_string(&index) {
        let html = html.replace("</body>", "<script src=\"bridge.js\"></script>\n</body>");
        std::fs::write(&index, html).map_err(|e| e.to_string())?;
    }

    // 2. the native backend command (sibling backend.maca, if any)
    let backend = src.parent().unwrap_or(Path::new(".")).join("backend.maca");
    let bin_dir = out.join("src-tauri").join("bin");
    let mut has_backend = false;
    if backend.exists() {
        std::fs::create_dir_all(&bin_dir).map_err(|e| e.to_string())?;
        compile(&backend, &bin_dir.join("backend"))?;
        has_backend = true;
    }

    // 3. the Tauri v2 Rust shell
    let st = out.join("src-tauri");
    std::fs::create_dir_all(st.join("src")).map_err(|e| e.to_string())?;
    std::fs::write(st.join("Cargo.toml"), tauri_cargo_toml(&name)).map_err(|e| e.to_string())?;
    std::fs::write(
        st.join("build.rs"),
        "fn main() {\n    tauri_build::build()\n}\n",
    )
    .map_err(|e| e.to_string())?;
    std::fs::write(st.join("tauri.conf.json"), tauri_conf(&name, &title))
        .map_err(|e| e.to_string())?;
    std::fs::write(st.join("src").join("main.rs"), tauri_main_rs()).map_err(|e| e.to_string())?;

    let note = if has_backend {
        ""
    } else {
        " (no backend.maca — add one for `maca_run`)"
    };
    Ok(format!(
        "scaffolded Tauri app in {}{note}\n  cd {} && cargo tauri build   # needs the Tauri CLI + a system webview",
        out.display(),
        st.display()
    ))
}

fn tauri_cargo_toml(name: &str) -> String {
    format!(
        "[package]\nname = \"{name}\"\nversion = \"0.1.0\"\nedition = \"2021\"\n\n\
         [build-dependencies]\ntauri-build = {{ version = \"2\", features = [] }}\n\n\
         [dependencies]\ntauri = {{ version = \"2\", features = [] }}\n\n\
         [[bin]]\nname = \"{name}\"\npath = \"src/main.rs\"\n"
    )
}

fn tauri_conf(name: &str, title: &str) -> String {
    format!(
        "{{\n  \"productName\": \"{title}\",\n  \"version\": \"0.1.0\",\n  \
         \"identifier\": \"dev.maca.{name}\",\n  \
         \"build\": {{ \"frontendDist\": \"../dist\" }},\n  \
         \"app\": {{\n    \"windows\": [{{ \"title\": \"{title}\", \"width\": 900, \"height\": 640, \"resizable\": true }}],\n    \
         \"security\": {{ \"csp\": null }}\n  }},\n  \
         \"bundle\": {{ \"active\": true, \"targets\": \"all\" }}\n}}\n"
    )
}

fn tauri_main_rs() -> String {
    "#![cfg_attr(not(debug_assertions), windows_subsystem = \"windows\")]\n\
     use std::process::Command;\n\n\
     // Run the bundled Maca `backend` binary with `arg`; return its stdout. The\n\
     // whole app is Maca — this Rust shell only hosts the webview and the bridge.\n\
     #[tauri::command]\n\
     fn maca_run(arg: String) -> String {\n\
     \x20   let exe = std::env::current_exe()\n\
     \x20       .ok()\n\
     \x20       .and_then(|p| p.parent().map(|d| d.join(\"bin\").join(\"backend\")))\n\
     \x20       .unwrap_or_else(|| std::path::PathBuf::from(\"backend\"));\n\
     \x20   match Command::new(&exe).arg(&arg).output() {\n\
     \x20       Ok(o) => String::from_utf8_lossy(&o.stdout).trim().to_string(),\n\
     \x20       Err(e) => format!(\"error: {e}\"),\n\
     \x20   }\n\
     }\n\n\
     fn main() {\n\
     \x20   tauri::Builder::default()\n\
     \x20       .invoke_handler(tauri::generate_handler![maca_run])\n\
     \x20       .run(tauri::generate_context!())\n\
     \x20       .expect(\"error while running tauri application\");\n\
     }\n"
    .to_string()
}

/// A safe Rust/crate identifier from a file stem (lowercase, `_`-joined).
fn sanitize_ident(s: &str) -> String {
    let mut out: String = s
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_lowercase()
            } else {
                '_'
            }
        })
        .collect();
    if out.chars().next().is_some_and(|c| c.is_ascii_digit()) {
        out.insert(0, '_');
    }
    if out.is_empty() {
        out.push_str("app");
    }
    out
}

/// Minimal base64 (standard alphabet, padded) — no external dep.
fn base64(data: &[u8]) -> String {
    const A: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let b = [
            chunk[0],
            *chunk.get(1).unwrap_or(&0),
            *chunk.get(2).unwrap_or(&0),
        ];
        let n = ((b[0] as u32) << 16) | ((b[1] as u32) << 8) | b[2] as u32;
        out.push(A[(n >> 18 & 63) as usize] as char);
        out.push(A[(n >> 12 & 63) as usize] as char);
        out.push(if chunk.len() > 1 {
            A[(n >> 6 & 63) as usize] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            A[(n & 63) as usize] as char
        } else {
            '='
        });
    }
    out
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
        Command::new("wsl")
            .arg(to_wsl(&out))
            .args(prog_args)
            .status()
    } else {
        Command::new(&out).args(prog_args).status()
    }
    .unwrap_or_else(|e| die(&format!("failed to launch binary: {e}")));
    std::process::exit(status.code().unwrap_or(1));
}

/// Read → parse → typecheck → emit C → `zig cc` → binary at `out`.
/// Compile `src` to a native binary at `out`, backed by the content-addressed
/// build cache: an unchanged source is copied straight from the cache (skipping
/// the whole pipeline), and a fresh build is stored back.
/// A fingerprint that changes whenever the compiler itself does — its version
/// plus its binary's mtime — so a rebuilt/upgraded `maca` invalidates the cache
/// even though the crate version string is unchanged during development.
fn compiler_fingerprint() -> String {
    let mtime = std::env::current_exe()
        .ok()
        .and_then(|p| std::fs::metadata(p).ok())
        .and_then(|m| m.modified().ok())
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    format!("{VERSION}-{mtime}")
}

/// Resolve `import a/b` to a sibling `.maca` file. `import std/foo` and other
/// module imports that don't name a local file (the string/list/math stdlib is
/// compiler builtins) resolve to nothing and are left for the backend. Two
/// candidates are tried: a file named by the import's last segment next to the
/// importer (`import selfhost/token` from `selfhost/main.maca` →
/// `selfhost/token.maca`), and the whole dotted path from the working directory.
fn resolve_module_path(segs: &[String], importer: &Path) -> Option<PathBuf> {
    let last = segs.last()?;
    let by_sibling = importer
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join(format!("{last}.maca"));
    if by_sibling.is_file() {
        return Some(by_sibling);
    }
    let by_path = PathBuf::from(format!("{}.maca", segs.join("/")));
    by_path.is_file().then_some(by_path)
}

/// Depth-first post-order over local `import`s: every module is emitted after
/// the modules it depends on, each exactly once. Cycles terminate (a module is
/// marked seen before its imports are walked).
fn collect_module(
    path: &Path,
    seen: &mut std::collections::HashSet<PathBuf>,
    order: &mut Vec<PathBuf>,
) -> Result<(), String> {
    let canon = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    if !seen.insert(canon.clone()) {
        return Ok(());
    }
    let src = std::fs::read_to_string(path)
        .map_err(|e| format!("cannot read {}: {e}", path.display()))?;
    let parsed = maca_parser::parse(&src);
    for item in &parsed.module.items {
        // `import a/b` (Module) and a single-word `import a` (Bare) both name a
        // local module when a matching `.maca` file exists; anything else (a
        // foreign header, `nixpkgs`, a stdlib builtin) resolves to no file and
        // is left for the backend.
        let segs = match item {
            maca_parser::ast::Stmt::Import(maca_parser::ast::Import::Module(segs)) => segs.clone(),
            maca_parser::ast::Stmt::Import(maca_parser::ast::Import::Bare(name)) => {
                vec![name.clone()]
            }
            _ => continue,
        };
        if let Some(dep) = resolve_module_path(&segs, path) {
            collect_module(&dep, seen, order)?;
        }
    }
    order.push(canon);
    Ok(())
}

/// Read a program and inline every local module it imports (transitively), in
/// dependency order, so `maca build a.maca` sees a single translation unit. A
/// program with no local imports is just its own text.
fn load_with_imports(entry: &Path) -> Result<String, String> {
    let mut seen = std::collections::HashSet::new();
    let mut order = Vec::new();
    collect_module(entry, &mut seen, &mut order)?;
    let mut combined = String::new();
    for p in &order {
        combined.push_str(
            &std::fs::read_to_string(p).map_err(|e| format!("cannot read {}: {e}", p.display()))?,
        );
        combined.push('\n');
    }
    Ok(combined)
}

fn compile(src: &Path, out: &Path) -> Result<(), String> {
    let source = load_with_imports(src)?;
    let key = build_cache::artifact_key(&source, &compiler_fingerprint(), "native");
    if let Some(cached) = build_cache::get(&key) {
        // transparent: `run` must stay silent, and stderr clean for callers that
        // check it. Set MACA_VERBOSE=1 to see cache hits.
        build_cache::place(&cached, out)?;
        if std::env::var("MACA_VERBOSE").is_ok() {
            eprintln!("  reused cached build [{key}]");
        }
        return Ok(());
    }
    compile_inner(src, &source, out)?;
    build_cache::put(&key, out);
    Ok(())
}

fn compile_inner(src: &Path, source: &str, out: &Path) -> Result<(), String> {
    let mut parsed = maca_parser::parse(source);
    if !parsed.errors.is_empty() {
        return Err(format!("parse errors:\n  {}", parsed.errors.join("\n  ")));
    }
    inject_nix_imports(&mut parsed.module, src)?;
    let diags = maca_core::check(&parsed.module, maca_core::Mode::Program);
    if !diags.is_empty() {
        let msgs: Vec<_> = diags
            .iter()
            .map(|d| format!("{:?}: {}", d.kind, d.msg))
            .collect();
        return Err(format!("type errors:\n  {}", msgs.join("\n  ")));
    }

    let c_src = maca_backend_c::emit_checked(&parsed.module).map_err(|probs| {
        format!(
            "unsupported by the native backend:\n  {}",
            probs.join("\n  ")
        )
    })?;
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
        // Plain Linux host (no WSL/nix): link the system sqlite with host cc.
        if !have_wsl() {
            let o = Command::new("cc")
                .arg(dir.join("main.c"))
                .arg(dir.join("maca_runtime.c"))
                .arg(dir.join("maca_ffi_sqlite.c"))
                .args(["-O2", "-lsqlite3", "-lpthread"])
                .arg("-I")
                .arg(&dir)
                .arg("-o")
                .arg(out)
                .output()
                .map_err(|e| format!("failed to run cc: {e}"))?;
            if !o.status.success() {
                return Err(format!(
                    "ffi build (host cc) failed:\n{}",
                    String::from_utf8_lossy(&o.stderr)
                ));
            }
            return Ok(());
        }
        let dev = nix_out("nixpkgs#sqlite.dev")?;
        let lib = nix_out("nixpkgs#sqlite.out")?;
        let mut args: Vec<String> = ["nix", "shell", "nixpkgs#zig", "-c", "zig", "cc"]
            .iter()
            .map(|s| s.to_string())
            .collect();
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
            return Err(format!(
                "ffi build failed:\n{}",
                String::from_utf8_lossy(&o.stderr)
            ));
        }
        return Ok(());
    }

    // PostgreSQL FFI (`import c "libpq-fe.h"`): link libpq.
    if c_imports
        .iter()
        .any(|h| h.contains("libpq") || h.contains("postgres"))
    {
        maca_runtime::write_pg_glue(&dir).map_err(|e| e.to_string())?;
        // Plain Linux host: locate libpq via pg_config.
        if !have_wsl() {
            let inc = capture_cmd("pg_config", &["--includedir"])?;
            let libdir = capture_cmd("pg_config", &["--libdir"])?;
            let o = Command::new("cc")
                .arg(dir.join("main.c"))
                .arg(dir.join("maca_runtime.c"))
                .arg(dir.join("maca_ffi_pg.c"))
                .arg("-I")
                .arg(&dir)
                .arg(format!("-I{inc}"))
                .arg(format!("-L{libdir}"))
                .arg(format!("-Wl,-rpath,{libdir}"))
                .args(["-lpq", "-O2", "-o"])
                .arg(out)
                .output()
                .map_err(|e| format!("failed to run cc: {e}"))?;
            if !o.status.success() {
                return Err(format!(
                    "pg ffi build (host cc) failed:\n{}",
                    String::from_utf8_lossy(&o.stderr)
                ));
            }
            return Ok(());
        }
        let dev = nix_out("nixpkgs#postgresql.dev")?;
        let lib = nix_out("nixpkgs#postgresql.lib")?;
        let mut args: Vec<String> = ["nix", "shell", "nixpkgs#zig", "-c", "zig", "cc"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        args.push(to_wsl(&dir.join("main.c")));
        args.push(to_wsl(&dir.join("maca_runtime.c")));
        args.push(to_wsl(&dir.join("maca_ffi_pg.c")));
        args.push(format!("-I{dev}/include"));
        args.push(format!("-L{lib}/lib"));
        args.push("-lpq".into());
        args.push(format!("-Wl,-rpath,{lib}/lib"));
        args.push("-O2".into());
        args.push("-o".into());
        args.push(to_wsl(out));
        let o = Command::new("wsl")
            .args(&args)
            .output()
            .map_err(|e| format!("failed to run zig via wsl: {e}"))?;
        if !o.status.success() {
            return Err(format!(
                "pg ffi build failed:\n{}",
                String::from_utf8_lossy(&o.stderr)
            ));
        }
        return Ok(());
    }

    // Python FFI (feature-gated): embeds CPython.
    if parsed.module.items.iter().any(|it| {
        matches!(it, maca_parser::Stmt::Import(maca_parser::Import::Foreign { lang, .. }) if lang == "py")
    }) {
        maca_runtime::write_py_glue(&dir).map_err(|e| e.to_string())?;
        // Plain Linux host: use python3-config to find the embed flags.
        if !have_wsl() {
            let includes = capture_cmd("python3-config", &["--includes"])?;
            let ldflags = capture_cmd("python3-config", &["--ldflags", "--embed"])
                .or_else(|_| capture_cmd("python3-config", &["--ldflags"]))?;
            let mut cc = Command::new("cc");
            cc.arg(dir.join("main.c"))
                .arg(dir.join("maca_runtime.c"))
                .arg(dir.join("maca_ffi_py.c"))
                .arg("-I")
                .arg(&dir)
                .args(includes.split_whitespace())
                .args(ldflags.split_whitespace())
                .args(["-O2", "-o"])
                .arg(out);
            let o = cc.output().map_err(|e| format!("failed to run cc: {e}"))?;
            if !o.status.success() {
                return Err(format!("py ffi build (host cc) failed:\n{}", String::from_utf8_lossy(&o.stderr)));
            }
            return Ok(());
        }
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
        return Err(format!(
            "zig cc failed:\n{}",
            String::from_utf8_lossy(&output.stderr)
        ));
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
        if let Stmt::Import(Import::Foreign { lang, spec }) = item
            && lang == "nix"
        {
            let path = dir.join(spec);
            let out = Command::new("wsl")
                .args(["nix-instantiate", "--eval", &to_wsl(&path)])
                .output()
                .map_err(|e| format!("wsl nix-instantiate: {e}"))?;
            if !out.status.success() {
                return Err(format!(
                    "nix eval {spec} failed:\n{}",
                    String::from_utf8_lossy(&out.stderr)
                ));
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
                is_const: false,
                target: Expr::Ident(name),
                tys: vec![],
                value: expr,
            }));
        }
    }
    for (i, b) in injected.into_iter().enumerate() {
        m.items.insert(i, b);
    }
    Ok(())
}

/// Run a shell command in WSL and capture trimmed stdout.
fn wsl_capture(cmd: &str) -> Result<String, String> {
    let o = Command::new("wsl")
        .args(["sh", "-c", cmd])
        .output()
        .map_err(|e| format!("wsl: {e}"))?;
    if !o.status.success() {
        return Err(format!(
            "`{cmd}` failed:\n{}",
            String::from_utf8_lossy(&o.stderr)
        ));
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
        return Err(format!(
            "nix build {attr} failed:\n{}",
            String::from_utf8_lossy(&o.stderr)
        ));
    }
    Ok(String::from_utf8_lossy(&o.stdout).trim().to_string())
}

fn stem(p: &Path) -> String {
    p.file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "out".into())
}

/// Is a working WSL available? The default toolchain shells out to
/// `wsl … zig cc` (NixOS side); when there's no WSL we fall back to a native
/// `cc`/`clang` so `maca build`/`run` work on a plain Linux host too.
fn have_wsl() -> bool {
    Command::new("wsl")
        .arg("true")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Is a command available on PATH (responds to `--version`)?
/// Run a host command and capture its trimmed stdout (for `python3-config` etc.).
fn capture_cmd(cmd: &str, args: &[&str]) -> Result<String, String> {
    let o = Command::new(cmd)
        .args(args)
        .output()
        .map_err(|e| format!("failed to run {cmd}: {e}"))?;
    if !o.status.success() {
        return Err(format!(
            "{cmd} {}: {}",
            args.join(" "),
            String::from_utf8_lossy(&o.stderr)
        ));
    }
    Ok(String::from_utf8_lossy(&o.stdout).trim().to_string())
}

fn have(cmd: &str) -> bool {
    Command::new(cmd)
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
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

    // The C runtime is invariant per compiler version, so compile it once to a
    // cached object and reuse it — a changed program only recompiles its own
    // `main.c`, not the whole runtime.
    let rt_c = dir.join("maca_runtime.c");
    let rt_src = std::fs::read(&rt_c).unwrap_or_default();
    let rt_key = build_cache::hash(&[
        &rt_src,
        compiler_fingerprint().as_bytes(),
        cc.as_bytes(),
        b"native-O2",
    ]);
    let rt_o = build_cache::object(&rt_key, &dir.join("maca_runtime.o"), |o| {
        let out = Command::new(cc)
            .args(["-O2", "-c"])
            .arg(&rt_c)
            .arg("-I")
            .arg(dir)
            .arg("-o")
            .arg(o)
            .output()
            .map_err(|e| format!("failed to run {cc}: {e}"))?;
        if !out.status.success() {
            return Err(format!(
                "{cc} (runtime) failed:\n{}",
                String::from_utf8_lossy(&out.stderr)
            ));
        }
        Ok(())
    })?;

    let mut cmd = Command::new(cc);
    cmd.arg(dir.join("main.c")).arg(&rt_o);
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
    cmd.arg("-I")
        .arg(dir)
        .arg("-O2")
        .arg("-s")
        .arg("-o")
        .arg(out);
    let output = cmd
        .output()
        .map_err(|e| format!("failed to run {cc}: {e}"))?;
    if !output.status.success() {
        return Err(format!(
            "{cc} failed:\n{}",
            String::from_utf8_lossy(&output.stderr)
        ));
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
