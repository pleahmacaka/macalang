//! maca-backend-nix: lower config-mode Maca to an idiomatic NixOS module.
//!
//! Routing + the config-mode conveniences from the spec:
//!   * `system.*` → NixOS options; `user.*` → home-manager (`home-manager.users.<u>`)
//!   * implicit enable: a `services.X = { .. }` / program block injects `enable = true`
//!   * smart values: `system.fonts = pkg, …` hoists to `fonts.packages`
//!   * `system.packages` → `environment.systemPackages`
//!   * `user.home.dirs = …` → non-destructive `xdg.userDirs`
//!   * `name: Program : pkg = { .. }` merge → `programs.name`
//!
//! Output is a `{ config, pkgs, lib, ... }:` module. Package references
//! (bare idents and `nixpkgs.x`) become `pkgs.x`.

use maca_parser::ast::*;

pub fn emit(m: &Module) -> String {
    emit_for_user(m, "alice")
}

/// Config-mode → a **dev-environment flake** (`flake.nix`), replacing a
/// hand-written flake. Reads `dev.*` bindings:
///   * `dev.name = "…"`        → shell description
///   * `dev.packages = a, b`   → `mkShell { packages = [ pkgs.a pkgs.b ]; }`
///   * `dev.env = { K = "v" }` → shell environment variables
///   * `dev.shellHook = "…"`   → `shellHook`
///
/// The flake is self-contained (nixpkgs input only, multi-system via
/// `genAttrs`) so `nix develop` works with no extra inputs.
pub fn emit_flake(m: &Module) -> String {
    let mut name = "dev".to_string();
    let mut packages = String::from("[ ]");
    let mut env: Vec<String> = Vec::new();
    let mut shell_hook: Option<String> = None;

    for item in &m.items {
        let Stmt::Bind(b) = item else { continue };
        let path = path_of(&b.target);
        match path.iter().map(String::as_str).collect::<Vec<_>>().as_slice() {
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
         \x20 description = \"{name} dev environment — generated from dev.maca by `maca dev`\";\n\n\
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

/// `user` is the home-manager username (from `maca.toml [hosts.X] user = …`).
pub fn emit_for_user(m: &Module, user: &str) -> String {
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

    // `name: Program : pkg = { .. }` — a typed program merge
    if !b.tys.is_empty() && p.len() == 1 {
        hm.push(format!("programs.{} = {};", p[0], record_with_enable(&b.value)));
        return;
    }

    match p.as_slice() {
        ["system", "packages"] => {
            top.push(format!("environment.systemPackages = {};", pkg_list(&b.value)))
        }
        ["system", "fonts"] => {
            // smart value: fonts hoist to fonts.packages
            top.push(format!("fonts.packages = {};", pkg_list(&b.value)))
        }
        ["user", "packages"] => hm.push(format!("home.packages = {};", pkg_list(&b.value))),
        ["user", "home", "dirs"] => hm.push(xdg_user_dirs(&b.value)),
        ["services", svc] => {
            top.push(format!("services.{svc} = {};", record_with_enable(&b.value)))
        }
        _ => {
            // generic: networking.hostName, system.stateVersion, …
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

/// Coalesce `f = a, b` (Value followed by Bare entries) into `f = [ a b ];`,
/// and render `key = value;` / `key = <nested>;` fields.
fn record_fields(fields: &[Field]) -> Vec<String> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < fields.len() {
        match &fields[i] {
            Field::Value { name, value: v } => {
                // gather trailing bare entries → list value
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
        _ => "null".into(),
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

fn plain_text(parts: &[StrPart]) -> String {
    parts
        .iter()
        .filter_map(|p| match p {
            StrPart::Text(t) => Some(t.clone()),
            _ => None,
        })
        .collect()
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
