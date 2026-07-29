//! What a package's entry module means to the code importing it.
//!
//! `modules/http/_init.maca` is where a package says what its own name stands
//! for. These build real packages on disk and inline them the way a build does,
//! because the whole question is which file a name comes from.

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

/// A package split across files still has one public name. The entry module
/// names what the package offers; where each one lives is the package's
/// business, and moving a function between its files is not a change to
/// anybody's import.
#[test]
fn an_entry_module_hands_on_what_it_imports() {
    let p = Project::new("reexport");
    p.write(
        "modules/http/server.maca",
        "listen(port: int) -> int => port\n",
    );
    p.write(
        "modules/http/_init.maca",
        "import { listen } from http/server\n",
    );
    let main = p.write(
        "main.maca",
        "import { listen } from http\n\nmain() -> int => listen(8080)\n",
    );

    let src = inlined(&main).expect("inlines");
    assert!(
        src.contains("listen(port: int) -> int"),
        "the definition came along: {src}"
    );
}

/// An entry module may also define names itself, and the two kinds are
/// indistinguishable to a caller.
#[test]
fn an_entry_module_may_define_as_well_as_hand_on() {
    let p = Project::new("mixed");
    p.write(
        "modules/http/server.maca",
        "listen(port: int) -> int => port\n",
    );
    p.write(
        "modules/http/_init.maca",
        "import { listen } from http/server\n\nserve(port: int) -> int => listen(port)\n",
    );
    let main = p.write(
        "main.maca",
        "import { serve, listen } from http\n\nmain() -> int => serve(1) + listen(2)\n",
    );

    let src = inlined(&main).expect("inlines");
    assert!(src.contains("serve(port: int)"), "defined here: {src}");
    assert!(src.contains("listen(port: int)"), "handed on: {src}");
}

/// Only an entry module hands names on. Any module re-exporting whatever it
/// imported would make every import path a public one, so a name reached
/// through an ordinary module is an error naming that module.
#[test]
fn an_ordinary_module_does_not_hand_on_what_it_imports() {
    let p = Project::new("noleak");
    p.write("modules/base.maca", "helper(n: int) -> int => n\n");
    p.write(
        "modules/wrap.maca",
        "import { helper } from base\n\nwrapped() -> int => helper(1)\n",
    );
    let main = p.write(
        "main.maca",
        "import { helper } from wrap\n\nmain() -> int => helper(1)\n",
    );

    let err = inlined(&main).expect_err("must not resolve");
    assert!(
        err.contains("'helper' is not defined in that module"),
        "unexpected error: {err}"
    );
}

/// The error names the package the reader wrote, not the `_init.maca` the
/// resolver landed on. Nobody types `_init`.
#[test]
fn a_missing_name_is_reported_against_the_package_name() {
    let p = Project::new("errname");
    p.write("modules/http/_init.maca", "serve() -> int => 0\n");
    let main = p.write(
        "main.maca",
        "import { nope } from http\n\nmain() -> int => 0\n",
    );

    let err = inlined(&main).expect_err("must not resolve");
    assert!(
        err.contains("from http:"),
        "should name the package, not the file: {err}"
    );
    assert!(!err.contains("_init"), "leaked the file name: {err}");
}

/// A selective import only carries what it names, so an entry module can only
/// hand on what it actually asked for.
#[test]
fn an_entry_hands_on_only_the_names_it_imported() {
    let p = Project::new("narrow");
    p.write(
        "modules/http/server.maca",
        "listen(n: int) -> int => n\n\ninternal(n: int) -> int => n\n",
    );
    p.write(
        "modules/http/_init.maca",
        "import { listen } from http/server\n",
    );
    let main = p.write(
        "main.maca",
        "import { internal } from http\n\nmain() -> int => internal(1)\n",
    );

    let err = inlined(&main).expect_err("internal is not part of the package");
    assert!(
        err.contains("'internal' is not defined in that module"),
        "unexpected error: {err}"
    );
}

/// Handing a name on does not drag the rest of its module in — the point of a
/// selective import survives one hop.
#[test]
fn handing_a_name_on_keeps_the_slice_narrow() {
    let p = Project::new("slice");
    p.write(
        "modules/http/server.maca",
        "listen(n: int) -> int => n\n\nENORMOUS = \"a name nothing asked for\"\n",
    );
    p.write(
        "modules/http/_init.maca",
        "import { listen } from http/server\n",
    );
    let main = p.write(
        "main.maca",
        "import { listen } from http\n\nmain() -> int => listen(1)\n",
    );

    let src = inlined(&main).expect("inlines");
    assert!(src.contains("listen(n: int)"), "the asked-for name: {src}");
    assert!(
        !src.contains("a name nothing asked for"),
        "pulled in the whole module: {src}"
    );
}
