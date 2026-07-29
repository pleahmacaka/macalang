//! Which file a name comes from.
//!
//! A package is a directory of modules and a module is a file; there is no
//! entry file and no index, so `modules/http/server.maca` is `http/server` and
//! that is the only thing it is. These build real packages on disk and inline
//! them the way a build does, because the whole question is which file a name
//! came from.

use maca_parser::imports::load_with_imports;
use std::path::{Path, PathBuf};

struct Project(PathBuf);

impl Project {
    fn new(tag: &str) -> Project {
        let dir = std::env::temp_dir().join(format!(
            "maca-pkg-{tag}-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("create project");
        std::fs::write(dir.join("maca.toml"), "[package]\nname = \"p\"\n").expect("manifest");
        Project(dir)
    }

    fn path(&self) -> &Path {
        &self.0
    }

    fn write(&self, rel: &str, body: &str) -> PathBuf {
        let p = self.0.join(rel);
        std::fs::create_dir_all(p.parent().expect("a parent")).expect("mkdir");
        std::fs::write(&p, body).expect("write");
        p
    }
}

impl Drop for Project {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn inlined(entry: &Path) -> Result<String, String> {
    load_with_imports(entry)
}

/// The shape the layout exists for: a package's files are reached by the path
/// they are at, from anywhere in the tree.
#[test]
fn a_package_file_is_imported_by_its_path() {
    let p = Project::new("path");
    p.write(
        "modules/http/server.maca",
        "listen(port: int) -> int => port\n",
    );
    let main = p.write(
        "main.maca",
        "import { listen } from http/server\n\nmain() -> int => listen(8080)\n",
    );

    let src = inlined(&main).expect("inlines");
    assert!(src.contains("listen(port: int) -> int"), "{src}");
}

/// A package's files may import each other by path too, and only what is asked
/// for comes along.
#[test]
fn a_package_file_may_import_its_neighbour() {
    let p = Project::new("neighbour");
    p.write(
        "modules/http/server.maca",
        "listen(n: int) -> int => n\n\nENORMOUS = \"a name nothing asked for\"\n",
    );
    p.write(
        "modules/http/serve.maca",
        "import { listen } from http/server\n\nserve() -> int => listen(8000)\n",
    );
    let main = p.write(
        "main.maca",
        "import { serve } from http/serve\n\nmain() -> int => serve()\n",
    );

    let src = inlined(&main).expect("inlines");
    assert!(
        src.contains("listen(n: int)"),
        "the neighbour came along: {src}"
    );
    assert!(
        !src.contains("a name nothing asked for"),
        "and nothing else did: {src}"
    );
}

/// A name a module does not define is an error naming that module — there is no
/// index that might have meant something else.
#[test]
fn a_name_a_module_does_not_define_is_refused() {
    let p = Project::new("missing");
    p.write("modules/http/server.maca", "listen(n: int) -> int => n\n");
    let main = p.write(
        "main.maca",
        "import { nope } from http/server\n\nmain() -> int => 0\n",
    );

    let err = inlined(&main).expect_err("must not resolve");
    assert!(err.contains("from server:"), "names the module: {err}");
    assert!(err.contains("'nope'"), "names the name: {err}");
}

/// A directory is not a module. Importing one is the same mistake as importing
/// a file that isn't there, and it is reported as one rather than resolving to
/// whatever happens to be inside.
#[test]
fn a_directory_is_not_a_module() {
    let p = Project::new("dir");
    p.write("modules/http/server.maca", "listen(n: int) -> int => n\n");
    let main = p.write(
        "main.maca",
        "import { listen } from http\n\nmain() -> int => listen(1)\n",
    );

    let err = inlined(&main).expect_err("http is a directory");
    assert!(err.contains("no module `http`"), "{err}");
}

/// An installed dependency is written by its own name. Where `maca add` put it
/// is the toolchain's business and appears in nobody's source.
#[test]
fn an_installed_dependency_is_written_by_its_name() {
    let p = Project::new("installed");
    p.write(
        "maca_modules/toml/parse.maca",
        "parse(s: str) -> str => s\n",
    );
    let main = p.write(
        "main.maca",
        "import { parse } from toml/parse\n\nmain() -> str => parse(\"a\")\n",
    );

    let src = inlined(&main).expect("inlines");
    assert!(src.contains("parse(s: str)"), "{src}");
}

/// A file inside a project resolves against *that* project or not at all.
/// Falling back to the working directory let a build started from one project
/// pick up another's packages — a build whose meaning depended on where it was
/// run, and one the language server could never agree with.
#[test]
fn a_project_does_not_borrow_another_projects_packages() {
    let a = Project::new("borrow-a");
    let b = Project::new("borrow-b");
    b.write("modules/lib/thing.maca", "greet() -> str => \"b's\"\n");
    let app = a.write(
        "app.maca",
        "import { greet } from lib/thing\n\nmain() -> str => greet()\n",
    );

    let here = std::env::current_dir().expect("cwd");
    std::env::set_current_dir(b.path()).expect("cd");
    let got = inlined(&app);
    std::env::set_current_dir(here).expect("cd back");

    assert!(got.is_err(), "resolved another project's package: {got:?}");
}

/// Two files in one package may each keep a helper to themselves. Everything
/// inlines into one translation unit, so without qualification the C compiler
/// reported a redefinition of a function the reader never wrote twice — and
/// "you cannot split a package into files" is not a package system.
#[test]
fn two_files_may_share_a_private_helper_name() {
    let p = Project::new("private");
    p.write(
        "modules/pkg/alpha.maca",
        "helper(s: str) -> str => \"alpha:\" ++ s\n\nalpha() -> str => helper(\"a\")\n",
    );
    p.write(
        "modules/pkg/beta.maca",
        "helper(s: str) -> str => \"beta:\" ++ s\n\nbeta() -> str => helper(\"b\")\n",
    );
    let main = p.write(
        "main.maca",
        "import { alpha } from pkg/alpha\nimport { beta } from pkg/beta\n\n\
         main() -> str => alpha() ++ beta()\n",
    );

    let src = inlined(&main).expect("inlines");
    assert!(src.contains("alpha__helper"), "alpha's is qualified: {src}");
    assert!(src.contains("beta__helper"), "beta's is qualified: {src}");
    assert!(
        !src.contains("\nhelper("),
        "neither kept the bare name: {src}"
    );
}

/// A name someone asked for by hand keeps the spelling they asked for —
/// qualifying it would break the caller that named it.
#[test]
fn a_requested_name_keeps_its_spelling() {
    let p = Project::new("requested");
    p.write(
        "modules/pkg/alpha.maca",
        "helper(s: str) -> str => s\n\nalpha() -> str => helper(\"a\")\n",
    );
    let main = p.write(
        "main.maca",
        "import { alpha, helper } from pkg/alpha\n\n\
         main() -> str => alpha() ++ helper(\"x\")\n",
    );

    let src = inlined(&main).expect("inlines");
    assert!(src.contains("helper(s: str)"), "unqualified: {src}");
    assert!(!src.contains("alpha__helper"), "not qualified: {src}");
}
