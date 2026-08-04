use maca_parser::ast::*;
use std::collections::BTreeMap;
use std::io::Write;
use std::process::Command;

const PLAYGROUND: &str = "../../apps/playground/playground.maca";

fn source() -> String {
    std::fs::read_to_string(PLAYGROUND).expect("apps/playground/playground.maca")
}

/// The part of the blob worth quoting when an assertion fails.
fn verdict(j: &str) -> &str {
    match (j.find("\"parseErrors\""), j.find("\"jsExports\"")) {
        (Some(a), Some(b)) if b > a => &j[a..b],
        _ => j,
    }
}

fn parsed() -> Module {
    let p = maca_parser::parse(&source());
    assert!(
        p.errors.is_empty(),
        "playground does not parse: {:?}",
        p.errors
    );
    p.module
}

/// Every field the page reads, for a program that runs.
#[test]
fn json_carries_what_the_page_reads() {
    let src = "fib(n: int) -> int =>\n    n < 2 ? n : fib(n - 1) + fib(n - 2)\n\
               main() -> int {\n    info(\"{fib(10)}\")\n    0\n}\n";
    let j = maca_wasm::compile_json(src, 0);
    for field in [
        "\"tokens\":",
        "\"symbols\":",
        "\"markers\":",
        "\"parseErrors\":[]",
        "\"diagnostics\":[]",
        "\"limits\":{}",
        "\"C\":",
        "\"JS\":",
        "\"run\":",
        "\"exit\":0",
        "\"flameSvg\":",
    ] {
        assert!(j.contains(field), "missing {field} in {}", verdict(&j));
    }
    assert!(
        j.contains("\"output\":\"55\\n\""),
        "fib(10) wrong: {}",
        verdict(&j)
    );
    assert!(
        !j.contains("\"HTML\":"),
        "the fixed html shell is back: {}",
        verdict(&j)
    );
}

/// A back end that refuses a construct says so by name, and its code is absent rather than wrong.
#[test]
fn a_refusing_backend_names_the_construct() {
    let ui = "who = \"world\"\nmain() -> Element =>\n    \
              div(class=\"p-8\", input(bind:value=who))\n";
    let j = maca_wasm::compile_json(ui, 0);
    assert!(
        j.contains("\"limits\":{\"C\":["),
        "no C refusal: {}",
        verdict(&j)
    );
    assert!(
        j.contains("DOM"),
        "the refusal should say why: {}",
        verdict(&j)
    );
    let outputs = &j[j.find("\"outputs\":").unwrap()..j.find("\"limits\":").unwrap()];
    assert!(
        !outputs.contains("\"C\":"),
        "refused C was emitted anyway: {outputs}"
    );
    assert!(
        outputs.contains("\"JS\":"),
        "js should still emit: {outputs}"
    );
    assert!(
        outputs.contains("\"CSS\":"),
        "styles() should emit: {outputs}"
    );
    assert!(
        j.contains("\"detail\":\"() -> Element\""),
        "no Element main: {}",
        verdict(&j)
    );
}

/// Config mode is reachable and produces Nix, which the page had no tab for.
#[test]
fn config_mode_emits_nix_and_refuses_impurity() {
    let cfg = "networking.hostName = \"rigel\"\nsystem.stateVersion = \"24.11\"\n";
    let j = maca_wasm::compile_json(cfg, 1);
    assert!(j.contains("\"Nix\":"), "no nix: {}", verdict(&j));
    assert!(
        j.contains("networking.hostName = \\\"rigel\\\""),
        "the option did not route: {}",
        verdict(&j)
    );
    assert!(
        j.contains("\"limits\":{}"),
        "nothing to refuse here: {}",
        verdict(&j)
    );

    let bad = "system.stateVersion = match 1 {\n    _ => \"24.11\"\n}\n";
    let j = maca_wasm::compile_json(bad, 1);
    assert!(
        j.contains("\"limits\":{\"Nix\":["),
        "no nix refusal: {}",
        verdict(&j)
    );
}

/// The four questions the editor asks about one caret, in one call.
#[test]
fn lsp_answers_hover_signature_definition_and_references() {
    let src = "add(a: int, b: int) -> int => a + b\n\
               main() -> int {\n    add(1, 2)\n    0\n}\n";

    let on_name = src.rfind("add").unwrap() + 1;
    let j = maca_wasm::lsp_json(src, on_name);
    assert!(
        j.contains("\"hover\":\"add(a: int, b: int) -> int\""),
        "{j}"
    );
    assert!(j.contains("\"definition\":{\"line\":1,\"col\":1"), "{j}");
    assert_eq!(
        j.matches("\"line\":3").count(),
        1,
        "call not referenced: {j}"
    );

    let after_comma = src.find("add(1, ").unwrap() + "add(1, ".len();
    let j = maca_wasm::lsp_json(src, after_comma);
    assert!(j.contains("\"params\":[\"a: int\",\"b: int\"]"), "{j}");
    assert!(j.contains("\"active\":1"), "wrong parameter: {j}");
}

/// The `import js` block, as one string.
fn block(m: &Module) -> String {
    m.items
        .iter()
        .filter_map(|it| match it {
            Stmt::Import(Import::Foreign { lang, spec }) if lang == "js" => Some(spec.clone()),
            _ => None,
        })
        .collect()
}

/// Every function the page declares with no body, which is the whole boundary.
fn host_functions(m: &Module) -> Vec<String> {
    m.items
        .iter()
        .filter_map(|it| match it {
            Stmt::Fn(f) if f.body.is_none() => Some(f.name.clone()),
            _ => None,
        })
        .collect()
}

/// Every declared function the page calls is one the block hands back through the bridge.
#[test]
fn every_declared_function_is_provided_by_the_block() {
    let m = parsed();
    let js = block(&m);
    assert!(!js.is_empty(), "no import js block");
    assert!(
        js.contains("maca.provide("),
        "the block reaches the page some other way than the bridge"
    );
    assert!(
        !js.contains("window.mc"),
        "the block is hanging functions on `window` again; `maca.provide` is \
         what the page declared them for"
    );

    let names = host_functions(&m);
    assert!(names.len() >= 10, "only found {names:?}; the scan broke");
    for name in &names {
        assert!(
            js.contains(&format!("{name}:")),
            "the page declares `{name}` and the block never provides it, so the \
             first call to it throws"
        );
    }

    let answered = under_node(
        &maca_backend_js::emit(&m).js,
        &[
            "(m.run(), \"ran\")",
            "m.outlineHtml().length > 0",
            "m.statusLine().length > 0",
        ],
    );
    assert_eq!(
        answered,
        vec!["\"ran\"", "true", "true"],
        "a page function reached the bridge and got nothing back"
    );
}

/// Every example the picker offers, as `(constant name, program)`.
fn examples(m: &Module) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for item in &m.items {
        if let Stmt::Bind(b) = item
            && let Expr::Ident(name) = &b.target
            && name.ends_with("Src")
            && let Expr::Str(parts) = &b.value
        {
            out.push((name.clone(), text_of(parts).trim().to_string()));
        }
    }
    out
}

fn text_of(parts: &[StrPart]) -> String {
    parts
        .iter()
        .filter_map(|p| match p {
            StrPart::Text(t) => Some(t.as_str()),
            StrPart::Interp(_) => None,
        })
        .collect()
}

#[test]
fn every_example_compiles_clean() {
    let m = parsed();
    let found = examples(&m);
    assert!(found.len() >= 8, "expected the whole picker, got {found:?}");

    for (name, src) in &found {
        let mode = if maca_lsp::is_config_source(src) {
            1
        } else {
            0
        };
        let j = maca_wasm::compile_json(src, mode);
        assert!(
            j.contains("\"parseErrors\":[]"),
            "{name} does not parse: {}",
            verdict(&j)
        );
        assert!(
            j.contains("\"diagnostics\":[]"),
            "{name} has type/effect diagnostics: {}",
            verdict(&j)
        );
    }
    assert!(
        found.iter().any(|(_, s)| maca_lsp::is_config_source(s)),
        "no config example"
    );
}

/// The literal class strings a `class=` attribute can reach.
fn class_strings(m: &Module) -> Vec<String> {
    let mut binds: BTreeMap<&str, &Expr> = BTreeMap::new();
    let mut fns: BTreeMap<&str, &FnDef> = BTreeMap::new();
    for item in &m.items {
        match item {
            Stmt::Bind(b) => {
                if let Expr::Ident(n) = &b.target {
                    binds.insert(n, &b.value);
                }
            }
            Stmt::Fn(f) => {
                fns.insert(&f.name, f);
            }
            _ => {}
        }
    }
    let mut out = Vec::new();
    for item in &m.items {
        if let Stmt::Fn(f) = item {
            for e in fn_exprs(f) {
                walk_for_class(e, &binds, &fns, &mut out);
            }
        }
    }
    out
}

fn fn_exprs(f: &FnDef) -> Vec<&Expr> {
    match &f.body {
        Some(FnBody::Expr(e)) => vec![e.as_ref()],
        Some(FnBody::Block(stmts)) => stmts
            .iter()
            .filter_map(|s| match s {
                Stmt::Expr(e) => Some(e),
                Stmt::Bind(b) => Some(&b.value),
                _ => None,
            })
            .collect(),
        None => Vec::new(),
    }
}

fn walk_for_class(
    e: &Expr,
    binds: &BTreeMap<&str, &Expr>,
    fns: &BTreeMap<&str, &FnDef>,
    out: &mut Vec<String>,
) {
    if let Expr::Call { args, .. } = e {
        for a in args {
            match a {
                Arg::Named { name, value } if name == "class" => {
                    resolve_class(value, binds, fns, out)
                }
                _ => walk_for_class(arg_value(a), binds, fns, out),
            }
        }
    }
}

fn arg_value(a: &Arg) -> &Expr {
    match a {
        Arg::Pos(e) | Arg::Named { value: e, .. } | Arg::Directive { value: e, .. } => e,
    }
}

fn resolve_class(
    e: &Expr,
    binds: &BTreeMap<&str, &Expr>,
    fns: &BTreeMap<&str, &FnDef>,
    out: &mut Vec<String>,
) {
    match e {
        Expr::Str(parts) => out.push(text_of(parts)),
        Expr::Ident(n) => {
            let target = binds.get(n.as_str()).copied();
            assert!(target.is_some(), "class={n} names nothing at top level");
            resolve_class(target.unwrap(), binds, fns, out);
        }
        Expr::Call { callee, .. } => {
            let Expr::Ident(name) = callee.as_ref() else {
                return;
            };
            let f = fns.get(name.as_str()).copied();
            assert!(f.is_some(), "class={name}() is not a function here");
            for e in fn_exprs(f.unwrap()) {
                resolve_class(e, binds, fns, out);
            }
        }
        Expr::Ternary { then, els, .. } => {
            resolve_class(then, binds, fns, out);
            resolve_class(els, binds, fns, out);
        }
        Expr::If { then, els, .. } => {
            for stmt in then.iter().chain(els.iter().flatten()) {
                if let Stmt::Expr(e) = stmt {
                    resolve_class(e, binds, fns, out);
                }
            }
        }
        Expr::Block(stmts) => {
            for stmt in stmts {
                if let Stmt::Expr(e) = stmt {
                    resolve_class(e, binds, fns, out);
                }
            }
        }
        other => panic!("a class= this test cannot follow: {other:?}"),
    }
}

/// Every utility the page names has a rule in the sheet the page ships.
#[test]
fn every_class_the_page_names_has_a_rule() {
    let m = parsed();
    let css = maca_backend_js::emit(&m).css;

    let mut checked = 0;
    for group in &class_strings(&m) {
        for class in group.split_whitespace() {
            let rule = maca_backend_js::rule(class);
            assert!(
                rule.is_some(),
                "`{class}` is not a utility the generator knows, so it will \
                 silently produce no CSS"
            );
            assert!(
                css.contains(&rule.unwrap()),
                "`{class}` has a rule but it is not in the emitted sheet, so \
                 the page ships without it"
            );
            checked += 1;
        }
    }
    assert!(checked > 60, "only checked {checked} classes; walk broke");
}

/// Load the emitted bundle under Node and evaluate each expression in `calls`.
fn under_node(js: &str, calls: &[&str]) -> Vec<String> {
    static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("maca-pg-{}-{n}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::File::create(dir.join("app.js"))
        .unwrap()
        .write_all(js.as_bytes())
        .unwrap();

    let mut driver = String::from(
        "globalThis.window = globalThis;\n\
         globalThis.location = { hash: \"\", origin: \"\", pathname: \"/\" };\n\
         globalThis.document = { getElementById: (id) =>\n\
         \x20 (id === \"wasm-b64\" ? { textContent: \"\" } : null) };\n\
         const m = require(\"./app.js\");\n",
    );
    for c in calls {
        driver.push_str(&format!("console.log(JSON.stringify({c}));\n"));
    }
    std::fs::File::create(dir.join("run.js"))
        .unwrap()
        .write_all(driver.as_bytes())
        .unwrap();

    let out = Command::new("node")
        .arg(dir.join("run.js"))
        .output()
        .expect("node is required for the playground bundle test");
    let _ = std::fs::remove_dir_all(&dir);
    assert!(
        out.status.success(),
        "node failed:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .map(str::to_string)
        .collect()
}

/// Every `<option value="…">` in the picker, in the order the page offers them.
fn picker_values(m: &Module) -> Vec<String> {
    fn walk(e: &Expr, out: &mut Vec<String>) {
        let Expr::Call { callee, args } = e else {
            return;
        };
        let is_option = matches!(callee.as_ref(), Expr::Ident(n) if n == "option");
        for a in args {
            if is_option
                && let Arg::Named { name, value } = a
                && name == "value"
                && let Expr::Str(parts) = value
            {
                out.push(text_of(parts));
            }
            walk(arg_value(a), out);
        }
    }
    let mut out = Vec::new();
    for item in &m.items {
        if let Stmt::Fn(f) = item {
            for e in fn_exprs(f) {
                walk(e, &mut out);
            }
        }
    }
    out
}

/// No two options load the same program, so every one of them reaches a branch.
#[test]
fn every_option_in_the_picker_loads_its_own_program() {
    let m = parsed();
    let values = picker_values(&m);
    assert!(values.len() >= 9, "the picker scan broke: {values:?}");
    assert!(
        values.contains(&String::new()),
        "no placeholder option, which is what a shared link selects: {values:?}"
    );

    let named: Vec<&String> = values.iter().filter(|v| !v.is_empty()).collect();
    let calls: Vec<String> = named
        .iter()
        .map(|v| format!("m.example({v:?}).src"))
        .collect();
    let refs: Vec<&str> = calls.iter().map(String::as_str).collect();
    let srcs = under_node(&maca_backend_js::emit(&m).js, &refs);

    for (i, a) in srcs.iter().enumerate() {
        for (j, b) in srcs.iter().enumerate().skip(i + 1) {
            assert_ne!(
                a, b,
                "the picker offers `{}` and `{}` and both load the same \
                 program, so one of them reaches no branch of example()",
                named[i], named[j]
            );
        }
    }
}

/// What the panes *say*, driven by a real compile result.
#[test]
fn the_panes_say_what_actually_happened() {
    let js = maca_backend_js::emit(&parsed()).js;

    let broken = maca_wasm::compile_json("main() -> int {\n    0\n", 0);
    let program_as_config = maca_wasm::compile_json("main() -> int {\n    0\n}\n", 1);
    let shown = |json: &str, mode: u32| {
        format!(
            "(m.state.ready = true, m.state.mode = {mode}, \
             m.present(JSON.stringify({json})))"
        )
    };
    let calls = [
        format!("({}, m.paneText(\"Console\"))", shown(&broken, 0)),
        format!(
            "({}, m.paneText(\"Diagnostics\"))",
            shown(&program_as_config, 1)
        ),
        format!("({}, m.paneText(\"Nix\"))", shown(&program_as_config, 1)),
        format!("({}, m.statusLine())", shown(&program_as_config, 1)),
    ];
    let refs: Vec<&str> = calls.iter().map(String::as_str).collect();
    let said = under_node(&js, &refs);

    assert!(
        !said[0].contains("config"),
        "a parse error still blames config mode: {}",
        said[0]
    );
    assert!(
        said[0].contains("parse error"),
        "the Console should point at the parse error: {}",
        said[0]
    );
    for pane in [&said[1], &said[2]] {
        assert!(
            pane.contains("main"),
            "the dropped function is not named: {pane}"
        );
    }
    assert!(
        !said[3].contains("clean"),
        "an empty config built from a program still reports clean: {}",
        said[3]
    );
}

/// The page's own logic, executed.
#[test]
fn the_pages_logic_runs() {
    let js = maca_backend_js::emit(&parsed()).js;
    let answers = under_node(
        &js,
        &[
            "m.example(\"page\").tab",
            "m.example(\"config\").mode",
            "m.example(\"nope\").note === m.example(\"hello\").note",
            "(m.state.mode = 0, m.tabClass(\"Nix\"))",
            "(m.state.mode = 1, m.tabClass(\"C\"))",
            "(m.state.mode = 1, m.tabInMode(\"Nix\"))",
            "(m.state.tab = \"Preview\", m.textPaneClass())",
            "(m.state.tab = \"Preview\", m.previewPaneClass() !== \"hidden\")",
            "(m.state.tab = \"C\", m.previewPaneClass())",
            "m.example(\"tour\").src.trim().startsWith(\"//\")",
        ],
    );
    assert_eq!(
        answers,
        vec![
            "\"Preview\"",
            "1",
            "true",
            "\"hidden\"",
            "\"hidden\"",
            "true",
            "\"hidden\"",
            "true",
            "\"hidden\"",
            "true",
        ],
        "the page's logic changed meaning"
    );
}
