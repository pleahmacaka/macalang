//! What inlining does to two modules that never imported each other.
//!
//! Everything lands in one translation unit, so a name written in one file can
//! be answered by a definition in another. These build a project on disk and ask
//! `load_with_imports` what the flattened program says, which is the last point
//! at which the answer is still readable: after this it is C, and the failures
//! this pass exists for all surfaced as somebody else's diagnostic.

use maca_parser::imports::load_with_imports;
use std::path::{Path, PathBuf};

/// A throwaway project rooted at a unique directory, removed on drop.
struct Project(PathBuf);

impl Project {
    fn new(tag: &str) -> Project {
        let dir = std::env::temp_dir().join(format!(
            "maca-scopes-{tag}-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("create project");
        std::fs::write(dir.join("maca.toml"), MANIFEST).expect("manifest");
        Project(dir)
    }

    fn write(&self, rel: &str, body: &str) -> PathBuf {
        let p = self.0.join(rel);
        std::fs::create_dir_all(p.parent().expect("a parent")).expect("mkdir");
        std::fs::write(&p, body).expect("write");
        p
    }

    /// The flattened program, or the diagnostic that refused to produce one.
    fn flatten(&self, entry: &Path) -> Result<String, String> {
        load_with_imports(entry)
    }
}

impl Drop for Project {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

const MANIFEST: &str = "[package]\nname = \"p\"\n";

/// How many times `needle` appears as a whole word.
fn times(text: &str, needle: &str) -> usize {
    text.split(|c: char| !c.is_alphanumeric() && c != '_')
        .filter(|w| *w == needle)
        .count()
}

// ---- two modules, one private name ----------------------------------------

/// The collision this pass exists for. Both helpers survive, under names of
/// their own, and each module's own call goes to its own.
#[test]
fn two_modules_may_each_keep_a_private_helper() {
    let p = Project::new("private");
    p.write(
        "modules/pkg/alpha.maca",
        "/// The alpha reading.\nread_a(n: int) -> int => helper(n) + 1\n\
         helper(n: int) -> int => n * 2\n",
    );
    p.write(
        "modules/pkg/beta.maca",
        "/// The beta reading.\nread_b(n: int) -> str => helper(n)\n\
         helper(n: int) -> str => \"b\"\n",
    );
    let app = p.write(
        "apps/x/main.maca",
        "import pkg/alpha\nimport pkg/beta\nmain() -> int => read_a(1)\n",
    );

    let flat = p.flatten(&app).expect("should flatten");
    assert!(
        flat.contains("alpha__helper") && flat.contains("beta__helper"),
        "each private helper should carry its module's name:\n{flat}"
    );
    assert_eq!(
        times(&flat, "helper"),
        0,
        "no bare `helper` should be left to be answered twice:\n{flat}"
    );
}

/// A `///` block is what marks an item API, so a documented name keeps it: the
/// private side of the collision is the side that moves.
#[test]
fn a_documented_name_keeps_it_and_the_private_one_moves() {
    let p = Project::new("documented");
    p.write(
        "modules/pkg/pub.maca",
        "/// Measure `s`.\nmeasure(s: str) -> int => s.length()\n",
    );
    p.write(
        "modules/pkg/priv.maca",
        "/// What priv offers.\nreport(n: int) -> int => measure(n)\n\
         measure(n: int) -> int => n + 1\n",
    );
    let app = p.write(
        "apps/x/main.maca",
        "import pkg/pub\nimport pkg/priv\nmain() -> int => report(1)\n",
    );

    let flat = p.flatten(&app).expect("should flatten");
    assert!(
        flat.contains("measure(s: str)"),
        "the documented one stays put:\n{flat}"
    );
    assert!(
        flat.contains("priv__measure"),
        "the undocumented one moves:\n{flat}"
    );
}

/// A name an importer asked for by hand is that module's API however it is
/// commented, so it does not move out from under the importer.
#[test]
fn a_name_asked_for_by_hand_is_api() {
    let p = Project::new("asked");
    p.write("modules/pkg/one.maca", "width(s: str) -> int => 1\n");
    p.write(
        "modules/pkg/two.maca",
        "/// Two's answer.\nanswer() -> int => width(\"x\")\n\
         width(s: str) -> int => 2\n",
    );
    let app = p.write(
        "apps/x/main.maca",
        "import { width } from pkg/one\nimport pkg/two\n\
         main() -> int => width(\"a\") + answer()\n",
    );

    let flat = p.flatten(&app).expect("should flatten");
    assert!(
        flat.contains("two__width"),
        "the module nobody asked should move:\n{flat}"
    );
    assert!(
        flat.contains("width(s: str)"),
        "the requested name is still the requested name:\n{flat}"
    );
}

/// Where both sides are API, there is nothing to move, and the clash is
/// reported naming both files rather than left to surface as a type error
/// against whichever signature won.
#[test]
fn a_clash_between_two_api_names_names_both_files() {
    let p = Project::new("clash");
    p.write(
        "modules/pkg/one.maca",
        "/// One's take.\nshared(n: int) -> int => n\n",
    );
    p.write(
        "modules/pkg/two.maca",
        "/// Two's take.\nshared(n: int) -> str => \"n\"\n",
    );
    let app = p.write(
        "apps/x/main.maca",
        "import pkg/one\nimport pkg/two\nmain() -> int => 0\n",
    );

    let err = p.flatten(&app).expect_err("should refuse");
    assert!(err.contains("shared"), "names the name: {err}");
    assert!(err.contains("one.maca"), "names the first file: {err}");
    assert!(err.contains("two.maca"), "names the second file: {err}");
}

/// A UFCS call writes the function as a field of its first argument, and the
/// rename cannot follow it. So the name stays where it is: leaving a collision
/// to be reported is a worse answer than a repair, and a repair that loses a
/// call site is worse than both.
#[test]
fn a_name_called_ufcs_is_left_alone() {
    let p = Project::new("ufcs");
    p.write(
        "modules/pkg/one.maca",
        "/// One's trim.\nsnip(s: str) -> str => s\n",
    );
    p.write(
        "modules/pkg/two.maca",
        "/// Two's use of it.\nrun(s: str) -> str => s.snip()\n\
         snip(s: str) -> str => s\n",
    );
    let app = p.write(
        "apps/x/main.maca",
        "import pkg/one\nimport pkg/two\nmain() -> int => 0\n",
    );

    let err = p
        .flatten(&app)
        .expect_err("nothing can move, so it is reported");
    assert!(err.contains("snip"), "names the name: {err}");
}

/// A third module reaching both sides of the collision means the API side, and
/// repairing the collision must not take its reference along.
///
/// This one compiled and ran, with the wrong answer: the private definition
/// moved, every module that could reach it was rewritten, and a call written
/// against the documented function ran a helper of a module never opened.
#[test]
fn a_third_module_means_the_published_definition() {
    let p = Project::new("meant");
    p.write(
        "modules/pkg/gauge.maca",
        "/// How wide `s` is.\nfits(s: str) -> int => s.length()\n",
    );
    p.write(
        "modules/pkg/snug.maca",
        "/// Snug's own reading.\nsnug_use() -> int => fits(\"ab\")\n\
         fits(s: str) -> int => s.length() * 3\n",
    );
    p.write(
        "modules/pkg/pick.maca",
        "import pkg/gauge\nimport pkg/snug\n\
         /// Pick's reading.\npicked() -> int => fits(\"abcd\")\n",
    );
    let app = p.write(
        "apps/x/main.maca",
        "import pkg/pick\nmain() -> int => picked()\n",
    );

    let flat = p.flatten(&app).expect("should flatten");
    assert!(
        flat.contains("snug__fits"),
        "the private side still moves:\n{flat}"
    );
    assert!(
        flat.contains("picked() -> int => fits(\"abcd\")"),
        "and the third module keeps naming the published one:\n{flat}"
    );
}

/// Where no one of the definitions in reach is the API one, nothing in the
/// referring module says which it meant, and it is refused naming that file.
/// The alternative is a call bound by which definition happened to move first.
#[test]
fn a_reference_nothing_settles_is_refused() {
    let p = Project::new("unsettled");
    for (m, n) in [("one", "1"), ("two", "2")] {
        p.write(
            &format!("modules/pkg/{m}.maca"),
            &format!("helper(n: int) -> int => n + {n}\n"),
        );
    }
    p.write(
        "modules/pkg/user.maca",
        "import pkg/one\nimport pkg/two\n\
         /// Which helper?\nuser_use() -> int => helper(0)\n",
    );
    let app = p.write(
        "apps/x/main.maca",
        "import pkg/user\nmain() -> int => user_use()\n",
    );

    let err = p.flatten(&app).expect_err("should refuse");
    assert!(err.contains("user.maca"), "names the referring file: {err}");
    assert!(
        err.contains("one.maca") && err.contains("two.maca"),
        "{err}"
    );
    assert!(
        err.contains("import { helper } from"),
        "says the fix: {err}"
    );
}

// ---- a top level against another module's binding --------------------------

/// A lambda capturing a parameter, and another module defining that name at top
/// level. The top level moves; the parameter is left saying what it says.
#[test]
fn a_captured_parameter_is_not_shadowed_by_another_module() {
    let p = Project::new("capture");
    p.write(
        "modules/pkg/style.maca",
        "/// Pad `s` to `w`.\npad(s: str, w: int) -> str => s\n",
    );
    p.write(
        "modules/pkg/text.maca",
        "/// Prefix every line.\n\
         indent(s: str, pad: str) -> str =>\n\
         \x20   lines(s).map(l => joined(l, pad)).join(\"\\n\")\n\
         joined(line: str, pad: str) -> str => pad ++ line\n\
         lines(s: str) -> str[] => s.split(\"\\n\")\n",
    );
    let app = p.write(
        "apps/x/main.maca",
        "import pkg/style\nimport pkg/text\n\
         main() -> int => len(pad(indent(\"a\", \" \"), 2))\n",
    );

    let flat = p.flatten(&app).expect("should flatten");
    assert!(
        flat.contains("style__pad"),
        "the top level moves out of the parameter's way:\n{flat}"
    );
    assert!(
        flat.contains("joined(l, pad)"),
        "the captured parameter keeps its name:\n{flat}"
    );
    assert!(
        flat.contains("style__pad(indent"),
        "the caller's reference follows the definition:\n{flat}"
    );
}

/// A module that imports the definition may legitimately mean both it and a
/// local of the same name, so nothing is moved on its account: which one a line
/// means is a question about that file.
#[test]
fn a_module_that_imports_the_name_does_not_move_it() {
    let p = Project::new("importer");
    p.write(
        "modules/pkg/style.maca",
        "/// Pad `s`.\npad(s: str) -> str => s\n",
    );
    p.write(
        "modules/pkg/user.maca",
        "import pkg/style\n/// Use both.\n\
         wrap(pad: str) -> str => pad ++ pad\n",
    );
    let app = p.write(
        "apps/x/main.maca",
        "import pkg/user\nmain() -> int => len(wrap(\"x\"))\n",
    );

    let flat = p.flatten(&app).expect("should flatten");
    assert!(
        !flat.contains("style__pad"),
        "an importer's own local is not inlining's business:\n{flat}"
    );
}

/// The entry file's names are what the person running the program typed, so a
/// collision with one of them is repaired on the module's side.
#[test]
fn the_entry_files_own_names_are_never_moved() {
    let p = Project::new("entry");
    p.write(
        "modules/pkg/lib.maca",
        "/// A step.\nstep(n: int) -> int => tally(n)\ntally(n: int) -> int => n\n",
    );
    let app = p.write(
        "apps/x/main.maca",
        "import pkg/lib\ntally(n: int) -> str => \"e\"\n\
         main() -> int => step(1)\n",
    );

    let flat = p.flatten(&app).expect("should flatten");
    assert!(
        flat.lines().any(|l| l.starts_with("tally(n: int) -> str")),
        "the entry's own definition stays put:\n{flat}"
    );
    assert!(
        flat.contains("lib__tally"),
        "the module's private one moves instead:\n{flat}"
    );
}

/// A module that defines `main` *is* the program, and `main` is the symbol the
/// linker asks for. Moving it because some unrelated module happens to bind the
/// name would leave a program with no entry point at all.
#[test]
fn a_programs_main_is_never_moved() {
    let p = Project::new("mainguard");
    p.write("modules/pkg/prog.maca", "main() -> int => 0\n");
    p.write(
        "modules/pkg/other.maca",
        "/// Hand `main` on.\ncall(main: int) -> int => again(main)\n\
         again(main: int) -> int => main\n",
    );
    let app = p.write("apps/x/run.maca", "import pkg/prog\nimport pkg/other\n");

    let flat = p.flatten(&app).expect("should flatten");
    assert!(
        flat.lines().any(|l| l.starts_with("main() -> int")),
        "the program kept its entry point:\n{flat}"
    );
}

/// `maca test` finds a suite by looking for `test_…` in the *flattened* program,
/// so a module's test function is read from outside the source the way `main` is.
/// Renaming one does not fail: it makes the test disappear, and a suite that
/// silently runs fewer tests than it has is worse than one that will not build.
#[test]
fn a_modules_test_function_is_never_moved() {
    let p = Project::new("suiteguard");
    for m in ["one", "two"] {
        p.write(
            &format!("modules/pkg/{m}.maca"),
            "test_shared() {\n    assert(true, \"x\")\n}\n",
        );
    }
    let app = p.write("apps/x/suite.maca", "import pkg/one\nimport pkg/two\n");

    let err = p.flatten(&app).expect_err("should refuse");
    assert!(err.contains("test_shared"), "names the test: {err}");
    assert!(
        err.contains("one.maca") && err.contains("two.maca"),
        "names both suites: {err}"
    );
}

/// A body-less declaration *is* the symbol some library provides, so its name is
/// not this pass's to change: `atol` renamed to `ffi__atol` asked the linker for
/// a symbol nothing defines.
#[test]
fn a_foreign_declaration_keeps_the_symbols_name() {
    let p = Project::new("foreign");
    p.write(
        "modules/pkg/ffi.maca",
        "/// C's `atol`.\natol(s: str) -> int\n",
    );
    p.write(
        "modules/pkg/user.maca",
        "/// A name that happens to be `atol`.\n\
         len_of(atol: str) -> int => atol.length()\n",
    );
    let app = p.write(
        "apps/x/main.maca",
        "import pkg/ffi\nimport pkg/user\n\
         main() -> int => atol(\"41\") + len_of(\"ab\")\n",
    );

    let flat = p.flatten(&app).expect("should flatten");
    assert!(
        !flat.contains("ffi__atol"),
        "the declaration keeps the name the library exports:\n{flat}"
    );
}

// ---- one written import, two files ----------------------------------------

/// The shadowing this repository closed by moving a directory, now said out
/// loud: a top-level `bench/` beside `modules/bench/` makes `import bench/stat`
/// name two files, and nothing in the import line says which.
#[test]
fn an_import_that_names_two_files_is_refused() {
    let p = Project::new("ambiguous");
    p.write("modules/bench/stat.maca", "median() -> int => 1\n");
    p.write("bench/stat.maca", "median() -> int => 2\n");
    let app = p.write(
        "apps/x/main.maca",
        "import bench/stat\nmain() -> int => 0\n",
    );

    let err = p.flatten(&app).expect_err("should refuse");
    assert!(err.contains("ambiguous"), "says what is wrong: {err}");
    assert!(err.contains("bench/stat"), "names the import: {err}");
    for want in ["modules/bench/stat.maca", "bench/stat.maca"] {
        assert!(err.contains(want), "names {want}: {err}");
    }
}

/// One file reached two ways is not two files. From inside `modules/`, the
/// written path and the `modules` root find the same `pkg/two.maca`, and a check
/// that only compared rules rather than files refused every package in the tree.
#[test]
fn one_file_found_by_both_rules_is_not_ambiguous() {
    let p = Project::new("samefile");
    p.write(
        "modules/pkg/one.maca",
        "import pkg/two\n/// One.\nfrom_one() -> int => from_two()\n",
    );
    p.write("modules/pkg/two.maca", "/// Two.\nfrom_two() -> int => 2\n");
    let app = p.write(
        "apps/x/main.maca",
        "import pkg/one\nmain() -> int => from_one()\n",
    );

    let flat = p.flatten(&app).expect("should flatten");
    assert!(flat.contains("from_two"), "the module came along:\n{flat}");
}

/// An installed dependency losing to the project's own source is the documented
/// precedence, not an accident: `maca_modules` is a directory `maca add` made,
/// and a vendored copy is meant to be overridable.
#[test]
fn a_vendored_copy_being_outranked_is_not_ambiguous() {
    let p = Project::new("vendor");
    p.write("tools/helper.maca", "/// Ours.\nv() -> int => 1\n");
    p.write(
        "maca_modules/tools/helper.maca",
        "/// Theirs.\nv() -> int => 2\n",
    );
    let app = p.write(
        "apps/x/main.maca",
        "import tools/helper\nmain() -> int => v()\n",
    );

    let flat = p.flatten(&app).expect("should flatten");
    assert!(flat.contains("Ours"), "the project's own wins:\n{flat}");
}
