use maca_parser::ast::*;

pub fn emit(m: &Module) -> String {
    emit_for_user(m, "alice")
}

thread_local! {
    /// Constructs config mode cannot express.
    static PROBLEMS: std::cell::RefCell<Vec<String>> = const { std::cell::RefCell::new(Vec::new()) };
}

fn problem(msg: impl Into<String>) {
    PROBLEMS.with(|p| p.borrow_mut().push(msg.into()));
}

/// Emit the NixOS module, or the list of constructs config mode cannot express.
pub fn emit_checked(m: &Module) -> Result<String, Vec<String>> {
    PROBLEMS.with(|p| p.borrow_mut().clear());
    let out = emit(m);
    let problems = PROBLEMS.with(|p| p.borrow().clone());
    if problems.is_empty() {
        Ok(out)
    } else {
        Err(problems)
    }
}

/// Config-mode → a **dev-environment flake** (`flake.nix`), replacing a hand-written flake.
pub fn emit_flake(m: &Module) -> String {
    let mut name = "dev".to_string();
    let mut packages = String::from("[ ]");
    let mut env: Vec<String> = Vec::new();
    let mut shell_hook: Option<String> = None;

    for item in &m.items {
        let Stmt::Bind(b) = item else { continue };
        let path = path_of(&b.target);
        match path
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>()
            .as_slice()
        {
            ["dev", "name"] => {
                if let Expr::Str(parts) = &b.value {
                    name = plain_text(parts);
                }
            }
            ["dev", "packages"] => packages = pkg_list(&b.value),
            ["dev", "env"] => {
                if let Expr::Record(fields) = &b.value {
                    for f in record_fields(fields) {
                        env.push(f);
                    }
                }
            }
            ["dev", "shellHook"] | ["dev", "shell_hook"] => {
                if let Expr::Str(parts) = &b.value {
                    shell_hook = Some(nix_string(parts));
                }
            }
            _ => {}
        }
    }

    let mut shell = String::new();
    shell.push_str(&format!("          packages = {packages};\n"));
    for e in &env {
        shell.push_str(&format!("          {e}\n"));
    }
    if let Some(h) = &shell_hook {
        shell.push_str(&format!("          shellHook = {h};\n"));
    }

    format!(
        "{{\n\
         \x20 description = \"{name} dev environment, generated from dev.maca by `maca dev`\";\n\n\
         \x20 inputs.nixpkgs.url = \"github:NixOS/nixpkgs/nixos-unstable\";\n\n\
         \x20 outputs = {{ self, nixpkgs }}:\n\
         \x20\x20\x20 let\n\
         \x20\x20\x20\x20\x20 systems = [ \"x86_64-linux\" \"aarch64-linux\" \"x86_64-darwin\" \"aarch64-darwin\" ];\n\
         \x20\x20\x20\x20\x20 forAllSystems = f: nixpkgs.lib.genAttrs systems (system: f nixpkgs.legacyPackages.${{system}});\n\
         \x20\x20\x20 in {{\n\
         \x20\x20\x20\x20\x20 devShells = forAllSystems (pkgs: {{\n\
         \x20\x20\x20\x20\x20\x20\x20 default = pkgs.mkShell {{\n\
         {shell}\
         \x20\x20\x20\x20\x20\x20\x20 }};\n\
         \x20\x20\x20\x20\x20 }});\n\
         \x20\x20\x20 }};\n\
         }}\n"
    )
}

/// A native Windows dev environment generated from `scoop.*` / `choco.*` / `winget.*` config.
pub struct WindowsDev {
    /// `setup.ps1`, which installs the configured tools (scoop is portable into `.maca/dev/scoop`; choco/winget install system-wide when present).
    pub setup: String,
    /// `activate.ps1`, sourced to enter the env.
    pub activate: String,
    /// Which package managers the config uses (for the CLI message).
    pub managers: Vec<String>,
}

/// Generate the Windows dev environment, or `None` if no `scoop`/`choco`/ `winget` packages are declared.
pub fn emit_windows_dev(m: &Module) -> Option<WindowsDev> {
    let mut name = "dev".to_string();
    let (mut scoop, mut buckets, mut choco, mut winget) =
        (Vec::new(), Vec::new(), Vec::new(), Vec::new());
    let mut env: Vec<(String, String)> = Vec::new();

    for item in &m.items {
        let Stmt::Bind(b) = item else { continue };
        match path_of(&b.target)
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>()
            .as_slice()
        {
            ["dev", "name"] => {
                if let Expr::Str(p) = &b.value {
                    name = plain_text(p);
                }
            }
            ["scoop", "packages"] => scoop = plain_list(&b.value),
            ["scoop", "buckets"] => buckets = plain_list(&b.value),
            ["choco", "packages"] => choco = plain_list(&b.value),
            ["winget", "packages"] => winget = plain_list(&b.value),
            ["dev", "env"] => {
                if let Expr::Record(fields) = &b.value {
                    env = env_pairs(fields);
                }
            }
            _ => {}
        }
    }
    if scoop.is_empty() && choco.is_empty() && winget.is_empty() {
        return None;
    }

    let mut managers = Vec::new();
    let mut s = String::new();
    s.push_str(&format!(
        "# Generated by `maca dev`: native Windows dev environment for {name}.\n\
         # Tools install under .maca\\dev\\ (scoop is project-local & portable).\n\
         $ErrorActionPreference = \"Stop\"\n\
         $dev = Join-Path (Resolve-Path .).Path \".maca\\dev\"\n\
         New-Item -ItemType Directory -Force -Path $dev | Out-Null\n\n"
    ));
    if !scoop.is_empty() {
        managers.push("scoop".to_string());
        s.push_str(
            "# ---- scoop (portable: everything under .maca\\dev\\scoop) ----\n\
             $env:SCOOP = Join-Path $dev \"scoop\"\n\
             New-Item -ItemType Directory -Force -Path $env:SCOOP | Out-Null\n\
             if (-not (Test-Path (Join-Path $env:SCOOP \"shims\\scoop.ps1\"))) {\n\
             \x20   Write-Host \"installing scoop into $env:SCOOP ...\"\n\
             \x20   Invoke-RestMethod -Uri https://get.scoop.sh | Invoke-Expression\n\
             }\n",
        );
        for b in &buckets {
            s.push_str(&format!("scoop bucket add {b} 2>$null\n"));
        }
        s.push_str(&format!("scoop install {}\n\n", scoop.join(" ")));
    }
    if !choco.is_empty() {
        managers.push("choco".to_string());
        s.push_str(&format!(
            "# ---- choco (system-wide; run this script elevated) ----\n\
             if (Get-Command choco -ErrorAction SilentlyContinue) {{\n\
             \x20   choco install {} -y\n\
             }} else {{ Write-Host \"choco not found, skipping choco packages\" }}\n\n",
            choco.join(" ")
        ));
    }
    if !winget.is_empty() {
        managers.push("winget".to_string());
        s.push_str("# ---- winget ----\nif (Get-Command winget -ErrorAction SilentlyContinue) {\n");
        for p in &winget {
            s.push_str(&format!(
                "    winget install --id {p} --accept-package-agreements --accept-source-agreements\n"
            ));
        }
        s.push_str("} else { Write-Host \"winget not found, skipping winget packages\" }\n\n");
    }
    s.push_str("Write-Host \"done. activate with:  . .\\.maca\\dev\\activate.ps1\"\n");

    let mut a = String::new();
    a.push_str(&format!(
        "# Source to enter the {name} dev env:  . .\\.maca\\dev\\activate.ps1\n\
         $dev = Join-Path (Resolve-Path .).Path \".maca\\dev\"\n"
    ));
    if !scoop.is_empty() {
        a.push_str(
            "$env:SCOOP = Join-Path $dev \"scoop\"\n\
             $env:PATH = \"$env:SCOOP\\shims;$env:PATH\"\n",
        );
    }
    for (k, v) in &env {
        a.push_str(&format!("$env:{k} = \"{}\"\n", ps_escape(v)));
    }
    if !scoop.is_empty() {
        a.push_str(
            "$jdk = Get-ChildItem -Path (Join-Path $env:SCOOP \"apps\") -Directory -ErrorAction SilentlyContinue |\n\
             \x20   Where-Object { $_.Name -match \"jdk|jre|temurin|openjdk|zulu|corretto|liberica\" } | Select-Object -First 1\n\
             if ($jdk) { $env:JAVA_HOME = Join-Path $jdk.FullName \"current\"; $env:PATH = \"$env:JAVA_HOME\\bin;$env:PATH\" }\n",
        );
    }
    a.push_str(&format!(
        "Write-Host \"maca dev: {name} (native, .maca\\dev)\"\n"
    ));

    Some(WindowsDev {
        setup: s,
        activate: a,
        managers,
    })
}

/// Bare package names from a `pkg, pkg, …` list (idents or strings).
fn plain_list(v: &Expr) -> Vec<String> {
    let items: Vec<&Expr> = match v {
        Expr::List(es) => es.iter().collect(),
        e => vec![e],
    };
    items
        .iter()
        .filter_map(|e| match e {
            Expr::Ident(n) => Some(n.clone()),
            Expr::Str(p) => Some(plain_text(p)),
            Expr::Field { .. } => {
                let p = path_of(e);
                (!p.is_empty()).then(|| p.join("/"))
            }
            _ => None,
        })
        .collect()
}

/// `{ K = "v", … }` → `(K, v)` pairs (string values only).
fn env_pairs(fields: &[Field]) -> Vec<(String, String)> {
    fields
        .iter()
        .filter_map(|f| match f {
            Field::Value {
                name,
                value: Expr::Str(p),
            } => Some((name.clone(), plain_text(p))),
            _ => None,
        })
        .collect()
}

/// Escape a value for a PowerShell double-quoted string.
fn ps_escape(s: &str) -> String {
    s.replace('`', "``").replace('"', "`\"").replace('$', "`$")
}

/// `user` is the home-manager username (from `maca.toml [hosts.X] user = …`).
fn emit_for_user(m: &Module, user: &str) -> String {
    let mut top: Vec<String> = Vec::new();
    let mut hm: Vec<String> = Vec::new();

    for item in &m.items {
        if let Stmt::Bind(b) = item {
            emit_bind(b, &mut top, &mut hm);
        }
    }

    let mut out = String::from("{ config, pkgs, lib, ... }:\n{\n");
    for l in &top {
        out.push_str(&indent(l, 1));
    }
    if !hm.is_empty() {
        out.push_str(&format!("  home-manager.users.{user} = {{\n"));
        for l in &hm {
            out.push_str(&indent(l, 2));
        }
        out.push_str("  };\n");
    }
    out.push_str("}\n");
    out
}

fn indent(s: &str, n: usize) -> String {
    let pad = "  ".repeat(n);
    s.lines().map(|l| format!("{pad}{l}\n")).collect()
}

fn emit_bind(b: &Bind, top: &mut Vec<String>, hm: &mut Vec<String>) {
    let path = path_of(&b.target);
    let p: Vec<&str> = path.iter().map(String::as_str).collect();

    if !b.tys.is_empty() && p.len() == 1 {
        hm.push(format!(
            "programs.{} = {};",
            p[0],
            record_with_enable(&b.value)
        ));
        return;
    }

    match p.as_slice() {
        ["system", "packages"] => top.push(format!(
            "environment.systemPackages = {};",
            pkg_list(&b.value)
        )),
        ["system", "fonts"] => top.push(format!("fonts.packages = {};", pkg_list(&b.value))),
        ["user", "packages"] => hm.push(format!("home.packages = {};", pkg_list(&b.value))),
        ["user", "home", "dirs"] => hm.push(xdg_user_dirs(&b.value)),
        ["services", svc] => top.push(format!(
            "services.{svc} = {};",
            record_with_enable(&b.value)
        )),
        _ => {
            top.push(format!("{} = {};", path.join("."), value(&b.value)));
        }
    }
}

fn pkg_list(v: &Expr) -> String {
    let items: Vec<&Expr> = match v {
        Expr::List(es) => es.iter().collect(),
        e => vec![e],
    };
    let parts: Vec<String> = items.iter().map(|e| pkg_ref(e)).collect();
    format!("[ {} ]", parts.join(" "))
}

fn pkg_ref(e: &Expr) -> String {
    match e {
        Expr::Ident(n) => format!("pkgs.{n}"),
        Expr::Field { name, .. } => format!("pkgs.{name}"),
        Expr::Str(parts) => nix_string(parts),
        _ => "pkgs.unknown".into(),
    }
}

/// A record with `enable = true` injected (implicit-enable).
fn record_with_enable(v: &Expr) -> String {
    let Expr::Record(fields) = v else {
        return value(v);
    };
    let mut out = String::from("{\n  enable = true;\n");
    for f in record_fields(fields) {
        out.push_str(&format!("  {f}\n"));
    }
    out.push('}');
    out
}

/// Coalesce `f = a, b` (Value followed by Bare entries) into `f = [ a b ];`, and render `key = value;` / `key = <nested>;` fields.
fn record_fields(fields: &[Field]) -> Vec<String> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < fields.len() {
        match &fields[i] {
            Field::Value { name, value: v } => {
                let mut extra = Vec::new();
                let mut j = i + 1;
                while let Some(Field::Bare(e)) = fields.get(j) {
                    extra.push(e);
                    j += 1;
                }
                if extra.is_empty() {
                    out.push(format!("{name} = {};", value(v)));
                } else {
                    let mut parts = vec![value(v)];
                    parts.extend(extra.iter().map(|e| value(e)));
                    out.push(format!("{name} = [ {} ];", parts.join(" ")));
                }
                i = j;
            }
            Field::Type { name, .. } => {
                out.push(format!("# {name}: <type>"));
                i += 1;
            }
            Field::Shorthand(n) => {
                out.push(format!("{n} = {n};"));
                i += 1;
            }
            Field::Bare(e) => {
                out.push(format!("{};", value(e)));
                i += 1;
            }
        }
    }
    out
}

fn value(e: &Expr) -> String {
    match e {
        Expr::Str(parts) => nix_string(parts),
        Expr::Int(n) => n.to_string(),
        Expr::Float(f) => format!("{f}"),
        Expr::Bool(b) => if *b { "true" } else { "false" }.into(),
        Expr::Ident(n) => format!("pkgs.{n}"),
        Expr::Field { name, .. } => format!("pkgs.{name}"),
        Expr::List(es) => {
            let parts: Vec<String> = es.iter().map(value).collect();
            format!("[ {} ]", parts.join(" "))
        }
        Expr::Record(fields) => {
            let fs = record_fields(fields);
            if fs.is_empty() {
                "{ }".into()
            } else {
                format!("{{\n  {}\n}}", fs.join("\n  "))
            }
        }
        Expr::Unary { op, expr } => match op {
            UnOp::Neg => format!("(-{})", value(expr)),
            UnOp::Not => format!("(!{})", value(expr)),
        },
        Expr::Binary { op, lhs, rhs } => binary(*op, lhs, rhs),
        Expr::Ternary { cond, then, els } => format!(
            "(if {} then {} else {})",
            value(cond),
            value(then),
            value(els)
        ),
        Expr::Unit => "null".into(),
        other => {
            problem(format!("{} is not a config value", describe(other)));
            "null".into()
        }
    }
}

/// An operator in a config value.
fn binary(op: BinOp, lhs: &Expr, rhs: &Expr) -> String {
    let (l, r) = (value(lhs), value(rhs));
    let infix = |o: &str| format!("({l} {o} {r})");
    match op {
        BinOp::Add => infix("+"),
        BinOp::Sub => infix("-"),
        BinOp::Mul => infix("*"),
        BinOp::Div => format!("(builtins.div {l} {r})"),
        BinOp::Mod => format!("({l} - (builtins.div {l} {r}) * {r})"),
        BinOp::Eq => infix("=="),
        BinOp::Ne => infix("!="),
        BinOp::Lt => infix("<"),
        BinOp::Gt => infix(">"),
        BinOp::Le => infix("<="),
        BinOp::Ge => infix(">="),
        BinOp::And => infix("&&"),
        BinOp::Or => infix("||"),
        BinOp::Concat => infix("+"),
        BinOp::Shl | BinOp::Shr => {
            problem(format!(
                "`{}` has no equivalent in Nix: shift with multiplication or \
                 division by a power of two",
                if matches!(op, BinOp::Shl) { "<<" } else { ">>" }
            ));
            "null".into()
        }
        BinOp::Union => {
            problem("a sum type is not a config value".to_string());
            "null".into()
        }
        BinOp::Pipe => {
            problem("`|>` is not lowered in config mode".to_string());
            "null".into()
        }
    }
}

/// Name a construct the way the author wrote it, for a refusal message.
fn describe(e: &Expr) -> &'static str {
    match e {
        Expr::Call { .. } => "a function call",
        Expr::Lambda { .. } => "a closure",
        Expr::Match { .. } => "`match`",
        Expr::If { .. } => "`if` (config mode takes the `c ? a : b` form)",
        Expr::Block(_) => "a block",
        Expr::Index { .. } => "an index",
        Expr::Range { .. } => "a range",
        Expr::With { .. } => "a record update",
        Expr::Assign { .. } => "an assignment",
        Expr::Try(_) | Expr::Fail(_) | Expr::Reify(_) => "the error operators (`?`, `fail`)",
        Expr::Await(_) | Expr::Spawn(_) => "`await`/`spawn` (config mode is pure)",
        Expr::Ctor { .. } => "a sum value",
        Expr::Path(_) => "a path expression",
        _ => "this construct",
    }
}

/// `user.home.dirs = "Downloads", …` → non-destructive xdg.userDirs.
fn xdg_user_dirs(v: &Expr) -> String {
    let items: Vec<&Expr> = match v {
        Expr::List(es) => es.iter().collect(),
        e => vec![e],
    };
    let mut lines =
        String::from("xdg.userDirs = {\n  enable = true;\n  createDirectories = true;\n");
    for e in items {
        if let Expr::Str(parts) = e {
            let name = plain_text(parts);
            let key = xdg_key(&name);
            lines.push_str(&format!("  {key} = \"$HOME/{name}\";\n"));
        }
    }
    lines.push_str("};");
    lines
}

fn xdg_key(name: &str) -> String {
    match name.to_ascii_lowercase().as_str() {
        "downloads" => "download".into(),
        "documents" => "documents".into(),
        "desktop" => "desktop".into(),
        "music" => "music".into(),
        "pictures" => "pictures".into(),
        "videos" => "videos".into(),
        "templates" => "templates".into(),
        "public" => "publicShare".into(),
        other => other.into(),
    }
}

fn nix_string(parts: &[StrPart]) -> String {
    let mut s = String::from("\"");
    for p in parts {
        match p {
            StrPart::Text(t) => {
                for c in t.chars() {
                    match c {
                        '"' => s.push_str("\\\""),
                        '\\' => s.push_str("\\\\"),
                        '\n' => s.push_str("\\n"),
                        _ => s.push(c),
                    }
                }
            }
            StrPart::Interp(_) => s.push_str("<interp>"),
        }
    }
    s.push('"');
    s
}

fn path_of(e: &Expr) -> Vec<String> {
    match e {
        Expr::Ident(n) => vec![n.clone()],
        Expr::Field { base, name } => {
            let mut p = path_of(base);
            p.push(name.clone());
            p
        }
        _ => Vec::new(),
    }
}
