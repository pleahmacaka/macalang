//! Where an `import` looks, on a real directory tree.
//!
//! These are about the filesystem rather than about values, which is why they
//! are here rather than in Maca: each one builds a project on disk and asks
//! `resolve_module_path` what a written import means inside it.

use maca_parser::modules::{Layout, resolve_module, resolve_module_path};
use std::path::{Path, PathBuf};

/// A throwaway project rooted at a unique directory, removed on drop.
struct Project(PathBuf);

impl Project {
    fn new(tag: &str) -> Project {
        let dir = std::env::temp_dir().join(format!(
            "maca-layout-{tag}-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("create project");
        Project(dir)
    }

    fn write(&self, rel: &str, body: &str) -> PathBuf {
        let p = self.0.join(rel);
        std::fs::create_dir_all(p.parent().expect("a parent")).expect("mkdir");
        std::fs::write(&p, body).expect("write");
        p
    }

    /// What `import <path>` resolves to from `importer`, relative to the root.
    fn resolve(&self, importer: &Path, path: &str) -> Option<String> {
        let segs: Vec<String> = path.split('/').map(str::to_string).collect();
        self.relative(resolve_module_path(&segs, importer)?)
    }

    /// The file the same import also names, if it names two.
    fn shadowed(&self, importer: &Path, path: &str) -> Option<String> {
        let segs: Vec<String> = path.split('/').map(str::to_string).collect();
        self.relative(resolve_module(&segs, importer)?.shadowed?)
    }

    fn relative(&self, hit: PathBuf) -> Option<String> {
        let hit = std::fs::canonicalize(&hit).unwrap_or(hit);
        let root = std::fs::canonicalize(&self.0).unwrap_or_else(|_| self.0.clone());
        Some(
            hit.strip_prefix(&root)
                .unwrap_or(&hit)
                .to_string_lossy()
                .replace('\\', "/"),
        )
    }
}

impl Drop for Project {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

const MANIFEST: &str = "[package]\nname = \"p\"\n";

/// `modules/` is a search root, so a package is imported by its own name from
/// anywhere in the tree — not by the path from the importer to it.
#[test]
fn a_package_under_modules_is_imported_by_name() {
    let p = Project::new("byname");
    p.write("maca.toml", MANIFEST);
    p.write("modules/http.maca", "serve() -> int => 0\n");
    let app = p.write("apps/site/main.maca", "import http\n");

    assert_eq!(
        p.resolve(&app, "http").as_deref(),
        Some("modules/http.maca")
    );
}

/// A file inside a package is reached by its path, and the directory holding it
/// is not itself a module: there is no entry file, so `http` names nothing.
#[test]
fn a_directory_is_not_a_module_but_its_files_are() {
    let p = Project::new("dir");
    p.write("maca.toml", MANIFEST);
    p.write("modules/http/server.maca", "listen() -> int => 0\n");
    p.write("modules/http/parse.maca", "parse() -> int => 0\n");
    let app = p.write("apps/site/main.maca", "import http/server\n");

    assert_eq!(p.resolve(&app, "http"), None, "a directory is not a module");
    assert_eq!(
        p.resolve(&app, "http/server").as_deref(),
        Some("modules/http/server.maca")
    );
    assert_eq!(
        p.resolve(&app, "http/parse").as_deref(),
        Some("modules/http/parse.maca")
    );
}

/// An installed dependency is a search root too, so it is written by its own
/// name — the directory `maca add` chose never appears in anybody's source.
#[test]
fn an_installed_dependency_needs_no_prefix() {
    let p = Project::new("installed");
    p.write("maca.toml", MANIFEST);
    p.write("maca_modules/toml/parse.maca", "parse() -> int => 0\n");
    let app = p.write("apps/site/main.maca", "import toml/parse\n");

    assert_eq!(
        p.resolve(&app, "toml/parse").as_deref(),
        Some("maca_modules/toml/parse.maca")
    );
}

/// A single-package repository puts its code in `src/` and needs no manifest
/// entry to say so.
#[test]
fn a_polyrepo_src_is_a_search_root_by_default() {
    let p = Project::new("src");
    p.write("maca.toml", MANIFEST);
    p.write("src/parser.maca", "parse() -> int => 0\n");
    let app = p.write("main.maca", "import parser\n");

    assert_eq!(
        p.resolve(&app, "parser").as_deref(),
        Some("src/parser.maca")
    );
}

/// Both roots are searched, and `modules` is looked at first.
#[test]
fn modules_is_searched_before_src() {
    let p = Project::new("order");
    p.write("maca.toml", MANIFEST);
    p.write("modules/thing.maca", "v() -> int => 1\n");
    p.write("src/thing.maca", "v() -> int => 2\n");
    let app = p.write("main.maca", "import thing\n");

    assert_eq!(
        p.resolve(&app, "thing").as_deref(),
        Some("modules/thing.maca")
    );
}

/// `[layout]` renames either root.
#[test]
fn the_roots_are_renameable() {
    let p = Project::new("rename");
    p.write("maca.toml", "[layout]\nmodules = \"packages\"\n");
    p.write("packages/http.maca", "serve() -> int => 0\n");
    let app = p.write("apps/site/main.maca", "import http\n");

    assert_eq!(
        p.resolve(&app, "http").as_deref(),
        Some("packages/http.maca")
    );
}

/// `apps` is deliberately not a search root: two applications may each have a
/// `conf`, and neither should silently answer for the other. An application is
/// reached by its written path.
#[test]
fn apps_is_not_a_search_root() {
    let p = Project::new("apps");
    p.write("maca.toml", MANIFEST);
    p.write("apps/tomo/conf.maca", "read() -> int => 0\n");
    p.write("apps/site/conf.maca", "read() -> int => 1\n");
    let tool = p.write("tools/build.maca", "import apps/tomo/conf\n");

    assert_eq!(p.resolve(&tool, "conf"), None, "not reachable bare");
    assert_eq!(
        p.resolve(&tool, "tomo/conf"),
        None,
        "not reachable with `apps/` left off — that is the whole difference \
         between a package root and a place applications live"
    );
    assert_eq!(
        p.resolve(&tool, "apps/tomo/conf").as_deref(),
        Some("apps/tomo/conf.maca"),
        "reachable by its written path"
    );
}

/// The written path still wins over the search roots, so a module sitting
/// beside its importer is not shadowed by a same-named package.
///
/// The colliding file is `modules/selfhost/token.maca` and not
/// `modules/token.maca`: the latter is never a candidate for the path
/// `selfhost/token`, so a fixture built that way resolves the same under any
/// ordering and pins nothing.
#[test]
fn a_written_path_beats_a_search_root() {
    let p = Project::new("written");
    p.write("maca.toml", MANIFEST);
    p.write("modules/selfhost/token.maca", "v() -> int => 1\n");
    p.write("selfhost/token.maca", "v() -> int => 2\n");
    let app = p.write("selfhost/main.maca", "import selfhost/token\n");

    assert_eq!(
        p.resolve(&app, "selfhost/token").as_deref(),
        Some("selfhost/token.maca")
    );
}

/// A directory that shares a package's name shadows the package. This is the
/// sharp edge of the rule above, pinned here so a change to it is a change to a
/// test rather than a surprise: the tree had a top-level `bench/` beside
/// `modules/bench/`, whose files the whole benchmark subsystem imports as
/// `bench/…`, and which one `import bench/stat` meant was decided by which file
/// happened to exist.
///
/// It was closed by moving the directory. Reordering the search to put the
/// roots first was tried and reverted: it does not fix the case where the
/// shadowing directory is under `apps/` — the ancestor walk reaches that
/// directory before the root — and it lets an installed dependency under
/// `maca_modules/` outrank the project's own source.
///
/// So the order stands and the shadowing is reported instead: resolution still
/// answers with the written path, and now also says which file that hid.
/// `imports::collect` refuses the import rather than compiling one of the two in
/// silence.
#[test]
fn a_directory_of_the_same_name_shadows_a_package() {
    let p = Project::new("collide");
    p.write("maca.toml", MANIFEST);
    p.write("modules/bench/stat.maca", "median() -> int => 1\n");
    p.write("bench/stat.maca", "median() -> int => 2\n");
    let app = p.write("apps/x/main.maca", "import bench/stat\n");

    assert_eq!(
        p.resolve(&app, "bench/stat").as_deref(),
        Some("bench/stat.maca"),
        "the written path wins — which is why a package's name is not a name \
         to give a directory"
    );
    assert_eq!(
        p.shadowed(&app, "bench/stat").as_deref(),
        Some("modules/bench/stat.maca"),
        "and the package it hid is named, so nobody has to guess"
    );
}

/// An import naming one file names one file. Every ordinary resolution has to
/// come back clean, or a diagnostic about two files fires on programs that have
/// only one.
#[test]
fn an_unambiguous_import_shadows_nothing() {
    let p = Project::new("clean");
    p.write("maca.toml", MANIFEST);
    p.write("modules/std/text.maca", "lines(s: str) -> str[] => [s]\n");
    let inside = p.write("modules/std/fs.maca", "import std/text\n");
    let app = p.write("apps/x/main.maca", "import std/text\n");

    assert_eq!(p.shadowed(&app, "std/text"), None, "from an app");
    assert_eq!(
        p.shadowed(&inside, "std/text"),
        None,
        "and from inside `modules/`, where the written path and the `modules` \
         root find the same file by two different rules"
    );
}

/// An installed dependency does not take over a path the project itself wrote.
/// `maca_modules` is a search root like the others, so without the written
/// path coming first, `maca add`ing anything called `tools` would answer for
/// this project's own `tools/`.
#[test]
fn an_installed_dependency_does_not_outrank_the_projects_own_source() {
    let p = Project::new("vendor");
    p.write("maca.toml", MANIFEST);
    p.write("tools/helper.maca", "v() -> int => 1\n");
    p.write("maca_modules/tools/helper.maca", "v() -> int => 2\n");
    let app = p.write("apps/x/main.maca", "import tools/helper\n");

    assert_eq!(
        p.resolve(&app, "tools/helper").as_deref(),
        Some("tools/helper.maca")
    );
}

/// The search stops at the project root: a stray `modules/` in a parent
/// directory is somebody else's, and reaching it would let the compiler and the
/// language server — whose own search is bounded by the workspace — disagree
/// about where a name is defined.
#[test]
fn the_search_stops_at_the_project_root() {
    let outer = Project::new("outer");
    outer.write("modules/escape.maca", "v() -> int => 0\n");
    outer.write("inner/maca.toml", MANIFEST);
    let app = outer.write("inner/apps/a/main.maca", "import escape\n");

    assert_eq!(outer.resolve(&app, "escape"), None);
}

#[test]
fn layout_defaults_are_the_recommended_shape() {
    let l = Layout::default();
    assert_eq!(
        l.roots,
        vec![
            "modules".to_string(),
            "src".to_string(),
            "maca_modules".to_string()
        ]
    );
    assert_eq!(l.apps, "apps");
}
