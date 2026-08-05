use std::fmt::Write;

/// The human specification, embedded so the text a model is given is the text a contributor reads.
const SPEC: &str = include_str!("../../../docs/SPEC.md");

/// The heading whose section is the normative description of the language.
const CHEATSHEET: &str = "## Language cheatsheet";

/// What `maca spec --llm` is allowed to cost, in tokens.
pub const BUDGET: usize = 15_000;

/// A conservative token estimate: English prose runs about four bytes to the token, and code runs denser, so three is the pessimistic side of the real number.
pub fn tokens(text: &str) -> usize {
    text.len().div_ceil(3)
}

/// The section of `docs/SPEC.md` under `heading`, up to the next heading of the same depth.
fn section<'a>(spec: &'a str, heading: &str) -> &'a str {
    let Some(start) = spec.find(heading) else {
        return "";
    };
    let rest = &spec[start + heading.len()..];
    let end = rest.find("\n## ").map_or(rest.len(), |i| i + 1);
    rest[..end].trim_matches('\n')
}

/// One `///` line above an item, paired with the item's own line.
fn documented_items(source: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let mut doc: Option<String> = None;
    for line in source.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("///") {
            doc = Some(rest.trim().to_string());
        } else if let Some(summary) = doc.take() {
            let signature = trimmed.split('{').next().unwrap_or(trimmed).trim();
            if !signature.is_empty() {
                out.push((signature.to_string(), summary));
            }
        }
    }
    out
}

/// The first `//` line of a module file, which is the blurb its own index prints.
fn blurb(source: &str) -> Option<String> {
    let first = source.lines().next()?.trim();
    first
        .strip_prefix("//")
        .filter(|rest| !rest.starts_with('/'))
        .map(|rest| rest.trim().to_string())
}

/// Every documented item of `std`, signature only, grouped by module.
///
/// `std` is indexed item by item because it is what an ordinary program
/// imports. The other seven packages get a line each: listing their internals
/// costs eleven thousand tokens and answers a question almost nobody writing a
/// program is asking.
fn stdlib_index() -> String {
    let mut out = String::new();
    let mut files: Vec<&(&str, &str)> = maca_stdlib::FILES
        .iter()
        .filter(|(path, _)| {
            path.starts_with("std/") && path.ends_with(".maca") && !path.contains("/tests/")
        })
        .collect();
    files.sort_by_key(|(path, _)| *path);

    for (path, source) in files {
        let items = documented_items(source);
        if items.is_empty() {
            continue;
        }
        let _ = writeln!(out, "\n`{}`\n", path.trim_end_matches(".maca"));
        for (signature, _) in items {
            let _ = writeln!(out, "- `{signature}`");
        }
    }
    out
}

/// The packages beside `std`, one line each, so a model knows they exist and what to ask for.
fn other_packages() -> String {
    let mut out = String::from(
        "\nBeside `std`, seven packages ride in the same binary. \
                                Run `maca spec --llm --package <name>` for one of their indexes.\n\n",
    );
    let mut names: Vec<&str> = maca_stdlib::packages()
        .into_iter()
        .filter(|p| *p != "std")
        .collect();
    names.sort_unstable();
    for name in names {
        let summary = maca_stdlib::FILES
            .iter()
            .find(|(path, _)| path.starts_with(&format!("{name}/")))
            .and_then(|(_, source)| blurb(source))
            .unwrap_or_else(|| "a package this binary carries".to_string());
        let _ = writeln!(out, "- `{name}`: {summary}");
    }
    out
}

/// Every documented item of one package, signature and summary, for when a model is working in it.
fn package_index(name: &str) -> String {
    let mut out = String::new();
    let mut files: Vec<&(&str, &str)> = maca_stdlib::FILES
        .iter()
        .filter(|(path, _)| {
            path.starts_with(&format!("{name}/"))
                && path.ends_with(".maca")
                && !path.contains("/tests/")
        })
        .collect();
    files.sort_by_key(|(path, _)| *path);

    for (path, source) in files {
        let items = documented_items(source);
        if items.is_empty() {
            continue;
        }
        let _ = writeln!(out, "\n### `{}`\n", path.trim_end_matches(".maca"));
        for (signature, summary) in items {
            let _ = writeln!(out, "- `{signature}` {summary}");
        }
    }
    out
}

/// Programs from the golden regression set, which every `cargo test` compiles and runs.
///
/// A specification written as rules leaves a model to guess how the rules
/// combine. These are whole programs, and they are the same files
/// `apps/examples` gates, so an example that stopped compiling stops the build
/// rather than teaching the wrong thing.
const EXAMPLES: &[(&str, &str)] = &[
    (
        "a whole program",
        include_str!("../../../apps/examples/tour.maca"),
    ),
    (
        "sum types and match",
        include_str!("../../../apps/examples/payload_sum.maca"),
    ),
    (
        "errors, without exceptions",
        include_str!("../../../apps/examples/catch.maca"),
    ),
    (
        "concurrency, with no `async` keyword",
        include_str!("../../../apps/examples/async.maca"),
    ),
    (
        "a generic function",
        include_str!("../../../apps/examples/generic.maca"),
    ),
    (
        "config mode, which compiles to Nix",
        include_str!("../../../apps/examples/ffi_nix.maca"),
    ),
];

/// The examples as one section, each under what it is there to show.
fn examples() -> String {
    let mut out = String::new();
    for (what, source) in EXAMPLES {
        let _ = writeln!(out, "\n### {what}\n\n```maca\n{}```", source);
    }
    out
}

/// The forms a reader reaches for that Maca does not have, taken from the table the checker itself uses.
fn mistakes() -> String {
    let mut out = String::from(
        "\nThese are the five the checker sees most, and it rejects each by name.\n\n",
    );
    for (name, hint) in maca_core::PHANTOM_KEYWORDS {
        let _ = writeln!(out, "- **No `{name}`.** {hint}.");
    }
    out.push_str(concat!(
        "\nTwo more that parse but mean something else:\n\n",
        "- **`lo..hi` excludes `hi`.** `0..xs.length()` is every index of `xs`. ",
        "Writing `0..xs.length() - 1` misses the last element.\n",
        "- **A block after `=>` is a record literal.** `f() => { a = 1 }` builds a ",
        "record. Use `f() { … }` when you meant a body.\n",
    ));
    out
}

/// The methods a receiver accepts, which are builtins rather than anything a module exports.
///
/// `trim` is a method on `str`, not a function in `std/text`, and a model that
/// cannot see this list writes the import that does not resolve. The tables are
/// the compiler's own, so a method added there appears here.
fn builtin_methods() -> String {
    let mut out = String::from(
        "\nThese are builtins on the receiver, not module exports: write `s.trim()`, \
         never `import { trim } from std/text`.\n\n",
    );
    for (receiver, methods) in [
        ("str", maca_core::STR_METHODS),
        ("T[]", maca_core::LIST_METHODS),
        ("Map k v", maca_core::MAP_METHODS),
    ] {
        let list: Vec<String> = methods.iter().map(|m| format!("`{m}`")).collect();
        let _ = writeln!(out, "- **`{receiver}`**: {}", list.join(", "));
    }
    out
}

/// What each target can carry, read out of the table the checker gates on.
///
/// A hand-written table would say what someone believed when they wrote it.
/// This one is `maca_core::TARGETS`, so `maca check --target` and this document
/// cannot disagree about what a target can do.
fn targets() -> String {
    let mut out = String::from(
        "\n`maca check --target <t>` refuses a program that performs an effect its \
         target cannot carry. With no `--target` a program is held to `native`, \
         which is what `maca build` produces; `--target all` holds it to what \
         every program target shares.\n\n\
         | target | flag | carries |\n|---|---|---|\n",
    );
    for (name, mask) in maca_core::TARGETS {
        if *name == "c" || *name == "tauri" {
            continue;
        }
        let flag = if *name == "native" {
            "(default)".to_string()
        } else {
            format!("`--target {name}`")
        };
        let carried = maca_core::effect_names(*mask);
        let carries = if carried.is_empty() {
            "nothing: config mode is data".to_string()
        } else {
            carried.join(", ")
        };
        let _ = writeln!(out, "| {name} | {flag} | {carries} |");
    }
    out.push_str(concat!(
        "\nThere is no BEAM target, by design.\n\n",
        "Two differences are not effects, and are the back end's own error: ",
        "`int / int` truncates natively and does not on `js`, and a ",
        "function-typed record field is refused on `rust` and `jvm`.\n"
    ));
    out
}

/// The whole specification, as one markdown document written for a model's context window.
pub fn llm_spec() -> String {
    let mut out = String::new();
    let _ = writeln!(
        out,
        "# Maca {}\n\nOne typed language for programs and infrastructure config. \
         Everything you write is `.maca` or `maca.toml`.\n\n\
         This document is generated by `maca spec --llm`. The language section is \
         `docs/SPEC.md` verbatim; the indexes below are read out of the compiler and \
         the standard library it carries, so nothing here can drift from what the \
         toolchain does.",
        env!("CARGO_PKG_VERSION")
    );

    let _ = writeln!(out, "\n## The language\n\n{}", section(SPEC, CHEATSHEET));
    let _ = writeln!(
        out,
        "\n## Whole programs\n\nEach compiles and runs as written.\n{}",
        examples()
    );
    let _ = writeln!(out, "\n## Mistakes to avoid\n{}", mistakes());
    let _ = writeln!(out, "\n## Targets\n{}", targets());
    let _ = writeln!(
        out,
        "\n## The standard library\n\nCarried inside the binary, so `import std/json` \
         resolves anywhere. Every item below is one `import` away.\n{}{}{}",
        builtin_methods(),
        stdlib_index(),
        other_packages()
    );
    out
}

/// `maca spec [--llm] [--package <name>]`: print the specification, or one package's index.
pub fn cmd_spec(args: &[String]) {
    if args.iter().any(|a| a == "--help" || a == "-h") {
        println!(
            "usage: maca spec --llm [--package <name>]\n\n\
             \x20 --llm       the whole specification as one markdown document\n\
             \x20 --package   that one package's index instead, in the same form"
        );
        return;
    }
    if let Some(at) = args.iter().position(|a| a == "--package") {
        let Some(name) = args.get(at + 1) else {
            eprintln!("maca spec: --package wants a name");
            std::process::exit(2);
        };
        let index = package_index(name);
        if index.is_empty() {
            eprintln!(
                "maca spec: no package `{name}`; the binary carries {}",
                maca_stdlib::packages().join(", ")
            );
            std::process::exit(1);
        }
        print!("# `{name}`\n{index}");
        return;
    }
    let text = llm_spec();
    let used = tokens(&text);
    if used > BUDGET {
        eprintln!("maca spec: {used} tokens, over the {BUDGET} budget");
        std::process::exit(1);
    }
    print!("{text}");
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The output is only useful if it fits beside the code a model is writing.
    #[test]
    fn the_whole_specification_fits_the_budget() {
        let used = tokens(&llm_spec());
        assert!(
            used <= BUDGET,
            "the specification is {used} tokens, over the {BUDGET} budget"
        );
        assert!(
            used > 2_000,
            "{used} tokens is too little to be the whole language: a section stopped being found"
        );
    }

    /// The language section is `docs/SPEC.md` itself, so the two cannot disagree.
    #[test]
    fn the_language_section_is_the_human_specification() {
        let cheatsheet = section(SPEC, CHEATSHEET);
        assert!(
            cheatsheet.len() > 3_000,
            "the cheatsheet heading stopped matching docs/SPEC.md: {} bytes",
            cheatsheet.len()
        );
        assert!(llm_spec().contains(cheatsheet.trim()));
    }

    /// A model told about a package the binary does not carry writes an import that does not resolve.
    #[test]
    fn every_carried_package_is_indexed() {
        let spec = llm_spec();
        for package in maca_stdlib::packages() {
            assert!(
                spec.contains(&format!("`{package}/")) || spec.contains(&format!("`{package}`")),
                "`{package}` is carried in the binary but absent from the index"
            );
        }
    }

    /// The advice and the diagnostic come from one table, so a reworded hint reaches both.
    #[test]
    fn the_mistakes_are_the_ones_the_checker_rejects() {
        let spec = llm_spec();
        for (name, hint) in maca_core::PHANTOM_KEYWORDS {
            assert!(spec.contains(hint), "`{name}`'s hint is not in the output");
            assert!(
                maca_core::phantom_hint(name).is_some(),
                "`{name}` is advertised but the checker does not reject it"
            );
        }
    }

    /// The table is the checker's, so a target added there appears here without anyone remembering to.
    #[test]
    fn the_target_table_is_the_one_the_checker_gates_on() {
        let table = targets();
        for (name, _) in maca_core::TARGETS {
            if *name == "c" || *name == "tauri" {
                continue;
            }
            assert!(
                table.contains(name),
                "`{name}` is a target and the table omits it"
            );
        }
        assert!(
            table.contains("| embedded |") && table.contains("exn"),
            "embedded carries exn and nothing else: {table}"
        );
        assert!(
            !table.contains("beam"),
            "there is no BEAM back end, by design"
        );
    }

    #[test]
    fn a_documented_item_is_read_with_its_summary() {
        let items =
            documented_items("/// Adds two numbers.\nadd(a: int, b: int) -> int => a + b\n");
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].0, "add(a: int, b: int) -> int => a + b");
        assert_eq!(items[0].1, "Adds two numbers.");
    }
}
