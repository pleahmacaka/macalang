//! UI elements and Tailwind on the native target.
//!
//! The JS backend turns `div(class="…", child)` into a reactive DOM. On native
//! there is no DOM, so the same syntax renders to an HTML *string* — which is
//! what a static site generator needs, and what `apps/tomo` was doing by hand
//! with string concatenation and a literal `<style>` block while the language
//! had a UI syntax and a Tailwind engine sitting unused.
//!
//! `styles()` returns the stylesheet for exactly the utilities the module's
//! `class=` literals mention, generated at compile time by the same engine the
//! JS backend uses.

use std::process::Command;

fn have(cmd: &str) -> bool {
    Command::new(cmd)
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}
fn wsl() -> bool {
    Command::new("wsl")
        .arg("true")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

fn run(name: &str, src: &str) -> (bool, String) {
    let dir = std::env::temp_dir().join("maca-native-ui");
    let _ = std::fs::create_dir_all(&dir);
    let f = dir.join(format!("{name}.maca"));
    std::fs::write(&f, src).unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_maca"))
        .args(["run", &f.to_string_lossy()])
        .output()
        .expect("spawn maca run");
    (
        out.status.success(),
        String::from_utf8_lossy(&out.stdout).to_string() + &String::from_utf8_lossy(&out.stderr),
    )
}

#[test]
fn elements_render_to_html() {
    if wsl() || !have("cc") {
        eprintln!("skipping: needs a host cc and no wsl");
        return;
    }
    let (ok, out) = run(
        "elements",
        "main() -> int {\n\
        \x20   info(div(class=\"wrap\",\n\
        \x20       h1(\"Title\")\n\
        \x20       p(\"text\")\n\
        \x20       a(href=\"x.html\", \"link\")\n\
        \x20       hr()\n\
        \x20   ))\n\
        \x20   0\n\
        }\n",
    );
    assert!(ok, "UI elements didn't compile natively:\n{out}");
    assert!(
        out.contains(
            "<div class=\"wrap\"><h1>Title</h1><p>text</p><a href=\"x.html\">link</a><hr></div>"
        ),
        "wrong HTML:\n{out}"
    );
}

/// An attribute value is escaped; a child is not. A generator emitting a code
/// block has already escaped its contents and must not have it done twice.
#[test]
fn attributes_are_escaped_and_children_are_not() {
    if wsl() || !have("cc") {
        eprintln!("skipping: needs a host cc and no wsl");
        return;
    }
    let (ok, out) = run(
        "escaping",
        "main() -> int {\n\
        \x20   info(p(title=\"a \\\"q\\\" & <b>\", \"already &lt;escaped&gt;\"))\n\
        \x20   0\n\
        }\n",
    );
    assert!(ok, "{out}");
    assert!(
        out.contains("title=\"a &quot;q&quot; &amp; &lt;b&gt;\""),
        "attribute not escaped:\n{out}"
    );
    assert!(
        out.contains(">already &lt;escaped&gt;<"),
        "child was re-escaped:\n{out}"
    );
}

/// Interpolation and nesting: elements are ordinary string-valued expressions,
/// so they compose with everything else in the language.
#[test]
fn elements_are_ordinary_string_expressions() {
    if wsl() || !have("cc") {
        eprintln!("skipping: needs a host cc and no wsl");
        return;
    }
    let (ok, out) = run(
        "compose",
        "item(s: str) -> str => li(s)\n\n\
         main() -> int {\n\
        \x20   n = 2\n\
        \x20   body = ul(item(\"one\") item(\"two\"))\n\
        \x20   info(section(p(\"{n} items\") body))\n\
        \x20   0\n\
        }\n",
    );
    assert!(ok, "{out}");
    assert!(
        out.contains("<section><p>2 items</p><ul><li>one</li><li>two</li></ul></section>"),
        "composition wrong:\n{out}"
    );
}

/// `styles()` ships the utilities used and nothing else.
#[test]
fn styles_are_generated_and_tree_shaken() {
    if wsl() || !have("cc") {
        eprintln!("skipping: needs a host cc and no wsl");
        return;
    }
    let (ok, out) = run(
        "styles",
        "main() -> int {\n\
        \x20   info(div(class=\"max-w-2xl mx-auto font-bold\", \"x\"))\n\
        \x20   info(styles())\n\
        \x20   0\n\
        }\n",
    );
    assert!(ok, "{out}");
    // the reading measure and centring a column are what a text page needs
    assert!(
        out.contains(".max-w-2xl { max-width:42rem; }"),
        "no max-w rule:\n{out}"
    );
    assert!(
        out.contains(".mx-auto { margin-left:auto;margin-right:auto; }"),
        "no mx-auto rule:\n{out}"
    );
    assert!(
        out.contains(".font-bold { font-weight:700; }"),
        "no font rule:\n{out}"
    );
    // tree-shaken: a utility the program never mentions is not in the sheet
    assert!(!out.contains(".text-2xl"), "shipped an unused rule:\n{out}");
}

/// A DOM handler cannot work in a string, and says so rather than emitting
/// markup that silently does nothing.
#[test]
fn event_handlers_are_rejected_on_the_native_target() {
    if wsl() || !have("cc") {
        eprintln!("skipping: needs a host cc and no wsl");
        return;
    }
    let (ok, out) = run(
        "handler",
        "go() -> int => 0\n\n\
         main() -> int {\n\
        \x20   info(button(on:click=go, \"press\"))\n\
        \x20   0\n\
        }\n",
    );
    assert!(!ok, "an event handler should not compile natively:\n{out}");
    assert!(out.contains("--target js"), "unhelpful message:\n{out}");
}

/// A user's own function wins over the tag of the same name.
///
/// `label`, `main`, `section`, `code`, `p`, `a`, `form` and `option` are all
/// HTML tags *and* names people give functions. Treating the tag as the winner
/// broke `examples/record_pattern.maca`, which defines `label(pos: bool)` and
/// started printing `<label>true</label>`.
#[test]
fn a_user_function_shadows_the_tag_of_the_same_name() {
    if wsl() || !have("cc") {
        eprintln!("skipping: needs a host cc and no wsl");
        return;
    }
    let (ok, out) = run(
        "shadow",
        "label(pos: bool) -> str => \"side: \" ++ (pos ? \"right\" : \"left\")\n\
         code(n: int) -> int => n * 2\n\n\
         main() -> int {\n\
        \x20   info(label(true))\n\
        \x20   info(\"{code(21)}\")\n\
        \x20   // the tag is still available where nothing shadows it\n\
        \x20   info(span(\"tag\"))\n\
        \x20   0\n\
        }\n",
    );
    assert!(ok, "{out}");
    assert!(
        out.contains("side: right"),
        "user `label` was hijacked:\n{out}"
    );
    assert!(out.contains("42"), "user `code` was hijacked:\n{out}");
    assert!(
        out.contains("<span>tag</span>"),
        "unshadowed tag broke:\n{out}"
    );
}

/// A local of the same name shadows it too.
#[test]
fn a_local_shadows_the_tag_of_the_same_name() {
    if wsl() || !have("cc") {
        eprintln!("skipping: needs a host cc and no wsl");
        return;
    }
    let (ok, out) = run(
        "shadow_local",
        "apply(f, x: int) -> int => f(x)\n\n\
         main() -> int {\n\
        \x20   info(\"{apply(n => n + 1, 41)}\")\n\
        \x20   0\n\
        }\n",
    );
    assert!(ok, "{out}");
    assert!(out.contains("42"), "{out}");
}
