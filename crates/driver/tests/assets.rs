mod common;
use common::*;

use std::path::{Path, PathBuf};
use std::process::Command;

/// A scratch project, keyed by name so concurrent tests do not share one.
fn project(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("maca-asset-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("maca.toml"), "[package]\nname = \"page\"\n").unwrap();
    dir
}

fn write(dir: &Path, rel: &str, text: &str) {
    let p = dir.join(rel);
    std::fs::create_dir_all(p.parent().unwrap()).unwrap();
    std::fs::write(p, text).unwrap();
}

fn app(dir: &Path, imports: &str) {
    write(
        dir,
        "home.maca",
        &format!("{imports}\nmain() -> Element =>\n    div(class=\"card\" \"hello\")\n"),
    );
}

fn build(dir: &Path) -> std::process::Output {
    Command::new(maca())
        .args(["build", "--target", "js"])
        .arg(dir.join("home.maca"))
        .arg("-o")
        .arg(dir.join("out"))
        .output()
        .expect("spawn maca")
}

fn errors(o: &std::process::Output) -> String {
    String::from_utf8_lossy(&o.stdout).to_string() + &String::from_utf8_lossy(&o.stderr)
}

fn built_page(dir: &Path) -> String {
    let o = build(dir);
    assert!(o.status.success(), "build failed: {}", errors(&o));
    std::fs::read_to_string(dir.join("out/index.html")).expect("index.html")
}

/// An installed package that states one entry point per kind, as `maca add npm:daisyui` would leave it.
fn install(dir: &Path, name: &str, manifest: &str) {
    write(dir, &format!("maca_modules/{name}/package.json"), manifest);
}

fn daisyui(dir: &Path) {
    install(
        dir,
        "daisyui",
        "{ \"name\": \"daisyui\", \"version\": \"4.0.0\", \
         \"main\": \"lib/index.js\", \"style\": \"dist/full.css\" }",
    );
    write(
        dir,
        "maca_modules/daisyui/dist/full.css",
        ".daisy-marker { color: rebeccapurple }\n",
    );
    write(
        dir,
        "maca_modules/daisyui/lib/index.js",
        "globalThis.daisyMarker = 1;\n",
    );
}

/// A package named without an extension is whatever its own manifest leads with.
#[test]
fn a_packages_own_entry_point_reaches_the_page() {
    let dir = project("entry");
    daisyui(&dir);
    app(&dir, "import \"npm:daisyui\"\n");

    let page = built_page(&dir);
    assert!(
        page.contains(".daisy-marker { color: rebeccapurple }"),
        "daisyui states `style`, so that is the entry that lands\n{page}"
    );
    assert!(
        !page.contains("globalThis.daisyMarker = 1;"),
        "and the entry it did not lead with is not also pulled in\n{page}"
    );
}

/// The entry a package does not lead with is reached by naming the file, which is the only way to ask for a kind.
#[test]
fn an_entry_the_package_does_not_lead_with_is_named_as_a_file() {
    let dir = project("other-entry");
    daisyui(&dir);
    app(&dir, "import \"npm:daisyui/lib/index.js\"\n");

    let page = built_page(&dir);
    assert!(
        page.contains("globalThis.daisyMarker = 1;"),
        "the file the import named is the one that lands\n{page}"
    );
}

#[test]
fn the_source_never_names_the_directory_maca_add_chose() {
    let dir = project("no-dist-path");
    daisyui(&dir);
    app(&dir, "import \"npm:daisyui\"\n");

    let _ = built_page(&dir);
    let source = std::fs::read_to_string(dir.join("home.maca")).unwrap();
    assert!(
        !source.contains("maca_modules") && !source.contains("dist/full.css"),
        "the program says the package's name and nothing about where it was installed\n{source}"
    );
}

#[test]
fn a_package_that_is_not_installed_fails_the_build_naming_it() {
    let dir = project("missing-package");
    app(&dir, "import \"npm:tailwindcss\"\n");

    let o = build(&dir);
    assert!(
        !o.status.success(),
        "an uninstalled package should fail the build"
    );
    let text = errors(&o);
    assert!(
        text.contains("tailwindcss"),
        "the message should name the package\n{text}"
    );
    assert!(
        text.contains("maca add npm:tailwindcss"),
        "and say how to install it\n{text}"
    );
}

/// A package that states no entry a page can carry is refused by name, never inlined as nothing.
#[test]
fn a_package_with_no_entry_of_that_kind_fails_the_build_naming_it() {
    let dir = project("no-entry");
    install(
        &dir,
        "typesonly",
        "{ \"name\": \"typesonly\", \"types\": \"dist/index.d.ts\" }",
    );
    write(
        &dir,
        "maca_modules/typesonly/dist/index.d.ts",
        "export {};\n",
    );
    app(&dir, "import \"npm:typesonly\"\n");

    let o = build(&dir);
    assert!(!o.status.success(), "no entry is not an empty one");
    let text = errors(&o);
    assert!(
        text.contains("typesonly") && text.contains("entry point"),
        "the message should name the package and what it lacks\n{text}"
    );
    assert!(
        text.contains("npm:typesonly/dist/"),
        "and point at the form that names the file\n{text}"
    );
}

#[test]
fn a_package_that_states_several_entries_answers_with_the_first_one_listed() {
    let dir = project("several");
    install(
        &dir,
        "many",
        "{ \"name\": \"many\", \"main\": \"main.js\", \
         \"module\": \"module.js\", \"browser\": \"browser.js\" }",
    );
    write(
        &dir,
        "maca_modules/many/main.js",
        "globalThis.pick = 'main';\n",
    );
    write(
        &dir,
        "maca_modules/many/module.js",
        "globalThis.pick = 'module';\n",
    );
    write(
        &dir,
        "maca_modules/many/browser.js",
        "globalThis.pick = 'browser';\n",
    );
    app(&dir, "import \"npm:many\"\n");

    let page = built_page(&dir);
    assert!(
        page.contains("globalThis.pick = 'browser';"),
        "a page is a browser, so `browser` outranks `module` and `main`\n{page}"
    );
    assert!(
        !page.contains("globalThis.pick = 'main';"),
        "and only one of them lands\n{page}"
    );
}

#[test]
fn a_file_inside_a_package_can_be_named_when_the_entry_is_not_the_one_wanted() {
    let dir = project("subpath");
    daisyui(&dir);
    write(
        &dir,
        "maca_modules/daisyui/dist/themes.css",
        ".theme-marker { color: teal }\n",
    );
    app(&dir, "import \"npm:daisyui/dist/themes.css\"\n");

    let page = built_page(&dir);
    assert!(page.contains(".theme-marker { color: teal }"), "{page}");
    assert!(
        !page.contains(".daisy-marker"),
        "the named file is the one that lands, not the entry point\n{page}"
    );
}

#[test]
fn a_file_a_package_does_not_hold_fails_the_build_naming_it() {
    let dir = project("bad-subpath");
    daisyui(&dir);
    app(&dir, "import \"npm:daisyui/dist/nope.css\"\n");

    let o = build(&dir);
    assert!(!o.status.success(), "a missing file should fail the build");
    let text = errors(&o);
    assert!(
        text.contains("dist/nope.css") && text.contains("daisyui"),
        "the message should name the package and the file\n{text}"
    );
}

#[test]
fn a_scoped_package_is_reached_under_the_name_maca_add_installed_it_as() {
    let dir = project("scoped");
    install(
        &dir,
        "starry-night",
        "{ \"name\": \"@wooorm/starry-night\", \"style\": \"style/core.css\" }",
    );
    write(
        &dir,
        "maca_modules/starry-night/style/core.css",
        ".starry-marker { color: navy }\n",
    );
    app(&dir, "import \"npm:@wooorm/starry-night\"\n");

    let page = built_page(&dir);
    assert!(page.contains(".starry-marker { color: navy }"), "{page}");
}

#[test]
fn a_path_beside_the_source_still_means_that_file() {
    let dir = project("relative");
    daisyui(&dir);
    write(&dir, "vendor/local.css", ".local-marker { color: olive }\n");
    app(
        &dir,
        "import \"vendor/local.css\"\nimport \"npm:daisyui\"\n",
    );

    let page = built_page(&dir);
    assert!(
        page.contains(".local-marker { color: olive }"),
        "a relative path is unchanged by the package form existing\n{page}"
    );
    assert!(page.contains(".daisy-marker"), "{page}");
}

#[test]
fn a_package_installed_at_the_project_root_is_found_from_a_source_below_it() {
    let dir = project("nested");
    daisyui(&dir);
    write(
        &dir,
        "pages/home.maca",
        "import \"npm:daisyui\"\n\nmain() -> Element =>\n    div(\"hello\")\n",
    );

    let o = Command::new(maca())
        .args(["build", "--target", "js"])
        .arg(dir.join("pages/home.maca"))
        .arg("-o")
        .arg(dir.join("out"))
        .output()
        .expect("spawn maca");
    assert!(o.status.success(), "build failed: {}", errors(&o));
    let page = std::fs::read_to_string(dir.join("out/index.html")).unwrap();
    assert!(
        page.contains(".daisy-marker"),
        "the walk that finds an imported module finds an installed package too\n{page}"
    );
}

/// `maca install` is what a checkout runs before it builds: the packages `maca.toml` names, at the versions `maca.lock` pinned.
#[test]
fn install_says_so_when_the_manifest_names_nothing() {
    let dir = project("install-empty");
    let out = Command::new(maca())
        .arg("install")
        .current_dir(&dir)
        .output()
        .expect("spawn maca install");
    assert!(out.status.success(), "{}", errors(&out));
    assert!(
        String::from_utf8_lossy(&out.stdout).contains("no dependencies"),
        "{}",
        errors(&out)
    );
}

/// A package already in `maca_modules` is left alone, so a second run costs nothing and a vendored copy is not overwritten.
#[test]
fn install_leaves_a_package_that_is_already_there() {
    let dir = project("install-present");
    std::fs::write(
        dir.join("maca.toml"),
        "[package]\nname = \"page\"\n\n[dependencies]\ndaisyui = \"npm:daisyui@^4\"\n",
    )
    .unwrap();
    daisyui(&dir);

    let out = Command::new(maca())
        .arg("install")
        .current_dir(&dir)
        .output()
        .expect("spawn maca install");
    assert!(out.status.success(), "{}", errors(&out));
    assert!(
        String::from_utf8_lossy(&out.stdout).contains("daisyui is already there"),
        "{}",
        errors(&out)
    );
}
