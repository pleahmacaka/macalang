//! `maca -m <module>[.<function>]` — run a function out of a module.
//!
//! A package under `modules/` is a thing you import, not a program, and until
//! now the only way to run something in one was to write a file whose whole
//! body was one call. That file is boilerplate, and boilerplate that has to be
//! committed somewhere is boilerplate everybody writes slightly differently.
//!
//!     maca -m http.serve          # modules/http, its `serve`
//!     maca -m http                # modules/http, its `main` or its `http`
//!     maca -m std/text            # a path names a module too
//!
//! What is actually compiled is a generated entry module: an import of the
//! target and a `main` that calls it. It is written under `.maca/run/` in the
//! project root rather than a temp directory, because that is what makes
//! `import http` inside it mean the same package it would mean anywhere else in
//! the project. A module whose entry point is already called `main` skips all
//! of that and is compiled as itself — a second `main` in one translation unit
//! is the wrapper's, and the call would bind to that one.
//!
//! A function is called with the leftover command line when it takes a `str[]`,
//! and with nothing when it takes nothing. Anything else is a clean error
//! naming the signature, rather than a C compiler complaining about arity.

use maca_parser::ast::{FnDef, Stmt, Type};
use std::path::{Path, PathBuf};

/// Split `http.serve` into its module and function.
///
/// The dot is the separator, and only the last one: `std.text.lines` is
/// `std/text` and `lines`, so a dotted spelling works as well as a slashed one.
/// With no dot, the function is decided after the module is read — see
/// [`entry_function`].
pub fn parse_spec(spec: &str) -> (String, Option<String>) {
    match spec.rsplit_once('.') {
        Some((m, f)) if !m.is_empty() && !f.is_empty() => (m.replace('.', "/"), Some(f.into())),
        _ => (spec.replace('.', "/"), None),
    }
}

/// Is this a module path a program could have written?
///
/// A leading dot, a trailing dot and an absolute path all reached resolution
/// and failed somewhere further in — `maca -m .run_it` searched `/run_it.maca`,
/// outside the project entirely, because joining an absolute segment replaces
/// the base. A keyword segment resolved to a real file and then failed to parse
/// inside a generated shim the user could not see. All of them are the same
/// answer: that is not a module name.
pub fn module_name_error(spec: &str, module: &str) -> Option<String> {
    if spec.starts_with('.') || spec.ends_with('.') {
        return Some(format!("`{spec}` starts or ends with a dot"));
    }
    if module.starts_with('/') {
        return Some(format!("`{spec}` is an absolute path, not a module"));
    }
    for seg in module.split('/') {
        if seg.is_empty() {
            return Some(format!("`{spec}` has an empty path segment"));
        }
        if maca_lexer::is_keyword(seg) {
            return Some(format!(
                "`{seg}` is a keyword, so no program can import it"
            ));
        }
    }
    None
}

/// The module file `spec` names, resolved the way an `import` would be.
///
/// Resolution is relative to a file in the project root, so `-m` and an
/// `import` written in that project mean the same thing by the same name.
pub fn resolve(module: &str, root: &Path) -> Option<PathBuf> {
    let segs: Vec<String> = module.split('/').map(str::to_string).collect();
    maca_parser::modules::resolve_module_path(&segs, &root.join("_m.maca"))
}

/// The function to call when the spec named no function.
///
/// `main` first — a module that can be run says so by defining one. Otherwise
/// the function named after the module, which is what makes a one-function
/// package (`modules/serve.maca` defining `serve`) run under its own name.
pub fn entry_function(module: &str, items: &[Stmt]) -> Option<String> {
    let own = module.rsplit('/').next().unwrap_or(module);
    for want in [&"main", &own] {
        if items
            .iter()
            .any(|i| matches!(i, Stmt::Fn(f) if f.name == **want))
        {
            return Some((*want).to_string());
        }
    }
    None
}

/// How the entry function wants to be called.
#[derive(Debug, PartialEq, Eq)]
pub enum Call {
    /// No parameters.
    Nothing,
    /// One `str[]` — the leftover command line.
    Args,
}

/// Decide how to call `f`, or say why it can't be an entry point.
pub fn call_shape(f: &FnDef) -> Result<Call, String> {
    match f.params.as_slice() {
        [] => Ok(Call::Nothing),
        [p] if is_str_array(p.ty.as_ref()) => Ok(Call::Args),
        _ => Err(format!(
            "`{}` takes {} — an entry point takes either nothing or one `str[]`",
            f.name,
            params_of(f)
        )),
    }
}

fn params_of(f: &FnDef) -> String {
    let ps: Vec<String> = f
        .params
        .iter()
        .map(|p| match &p.ty {
            Some(t) => format!("{}: {}", p.name, type_name(t)),
            None => p.name.clone(),
        })
        .collect();
    format!("({})", ps.join(", "))
}

fn type_name(t: &Type) -> String {
    match t {
        Type::Name(segs) => segs.join("."),
        Type::Array(inner) => format!("{}[]", type_name(inner)),
        Type::Opt(inner) => format!("{}?", type_name(inner)),
        Type::Paren(inner) => type_name(inner),
        Type::Apply(head, args) => {
            let a: Vec<String> = args.iter().map(type_name).collect();
            format!("{} {}", type_name(head), a.join(" "))
        }
    }
}

fn is_str_array(t: Option<&Type>) -> bool {
    matches!(t, Some(Type::Array(inner)) if matches!(&**inner, Type::Name(s) if s == &["str"]))
}

/// What the entry function's answer means to the shell.
#[derive(Debug, PartialEq, Eq)]
pub enum Answer {
    /// `-> int` — the exit status itself.
    Code,
    /// `-> bool` — success or failure, the shell's 0 and 1.
    Success,
    /// Anything else, or nothing: the call is made for its effect.
    Effect,
}

/// How a declared return type reaches the shell.
pub fn answer_of(f: &FnDef) -> Answer {
    match f.ret.as_ref().map(type_name).as_deref() {
        Some("int") => Answer::Code,
        Some("bool") => Answer::Success,
        _ => Answer::Effect,
    }
}

/// The source of the entry module to compile.
///
/// A selective import, so only the entry point and what it needs come along —
/// running one function out of a package should not drag in the rest of it.
pub fn entry_source(module: &str, function: &str, call: &Call, answer: &Answer) -> String {
    let invoke = match call {
        Call::Nothing => format!("{function}()"),
        Call::Args => format!("{function}(args)"),
    };
    // `args` is bound whether or not it is passed on, so the generated `main`
    // has one shape and an unused binding is the only cost.
    let body = match answer {
        Answer::Code => format!("    {invoke}\n"),
        Answer::Success => format!("    {invoke} ? 0 : 1\n"),
        Answer::Effect => format!("    {invoke}\n    0\n"),
    };
    format!(
        "// Generated by `maca -m {module}.{function}` — rewritten on every run\n\
         // and removed afterwards.\n\
         import {{ {function} }} from {module}\n\
         \n\
         main(args: str[]) -> int {{\n\
         {body}}}\n"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_dotted_spec_splits_at_the_last_dot() {
        assert_eq!(parse_spec("http.serve"), ("http".into(), Some("serve".into())));
        assert_eq!(
            parse_spec("std.text.lines"),
            ("std/text".into(), Some("lines".into()))
        );
    }

    #[test]
    fn a_bare_spec_names_only_a_module() {
        assert_eq!(parse_spec("http"), ("http".into(), None));
        assert_eq!(parse_spec("std/text"), ("std/text".into(), None));
    }

    fn parse_one(src: &str) -> FnDef {
        let m = maca_parser::parse(src).module;
        m.items
            .into_iter()
            .find_map(|i| match i {
                Stmt::Fn(f) => Some(f),
                _ => None,
            })
            .expect("a function")
    }

    #[test]
    fn an_entry_takes_nothing_or_one_string_list() {
        assert_eq!(call_shape(&parse_one("serve() -> int => 0\n")), Ok(Call::Nothing));
        assert_eq!(
            call_shape(&parse_one("serve(args: str[]) -> int => 0\n")),
            Ok(Call::Args)
        );
        let err = call_shape(&parse_one("serve(port: int) -> int => 0\n")).expect_err("refused");
        assert!(err.contains("port: int"), "names the signature: {err}");
    }

    #[test]
    fn the_return_type_decides_the_exit_status() {
        assert_eq!(answer_of(&parse_one("f() -> int => 0\n")), Answer::Code);
        assert_eq!(answer_of(&parse_one("f() -> bool => true\n")), Answer::Success);
        assert_eq!(answer_of(&parse_one("f() -> str => \"x\"\n")), Answer::Effect);
        assert_eq!(answer_of(&parse_one("f() => 1\n")), Answer::Effect);
    }

    /// `main` is preferred; a package with none is run by its own name.
    #[test]
    fn the_entry_function_is_main_then_the_module_name() {
        let items = maca_parser::parse("main() -> int => 0\nhttp() -> int => 1\n")
            .module
            .items;
        assert_eq!(entry_function("http", &items).as_deref(), Some("main"));

        let items = maca_parser::parse("http() -> int => 1\n").module.items;
        assert_eq!(entry_function("http", &items).as_deref(), Some("http"));

        let items = maca_parser::parse("other() -> int => 1\n").module.items;
        assert_eq!(entry_function("http", &items), None);

        // a nested path is named by its last segment
        let items = maca_parser::parse("text() -> int => 1\n").module.items;
        assert_eq!(entry_function("std/text", &items).as_deref(), Some("text"));
    }

    #[test]
    fn the_generated_entry_imports_only_what_it_runs() {
        let src = entry_source("http", "serve", &Call::Args, &Answer::Code);
        assert!(src.contains("import { serve } from http"), "{src}");
        assert!(src.contains("serve(args)"), "{src}");

        let src = entry_source("http", "serve", &Call::Nothing, &Answer::Effect);
        assert!(src.contains("serve()\n    0"), "discards and succeeds: {src}");

        let src = entry_source("http", "ok", &Call::Nothing, &Answer::Success);
        assert!(src.contains("ok() ? 0 : 1"), "{src}");
    }
}
