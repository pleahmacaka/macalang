use std::io::{BufRead, BufReader, Write};
use std::process::{Command, Stdio};

fn frame(body: &str) -> String {
    format!("Content-Length: {}\r\n\r\n{}", body.len(), body)
}

/// A path as a `file://` URI. The percent-encoding is what keeps a Windows path out of the JSON escapes: a raw `C:\Users` spells `\U`, and the request never parses.
fn file_uri(p: &std::path::Path) -> String {
    let mut out = String::from("file://");
    for b in p.to_string_lossy().bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' | b'/' => {
                out.push(b as char)
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

/// A scratch workspace no other run can be inside.
fn scratch(tag: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("maca-lsp-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

/// Kills the server if an assertion unwinds before the session is closed.
struct Child(std::process::Child);

impl Drop for Child {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

/// Read one Content-Length-framed message body from `r`.
fn read_frame(r: &mut impl BufRead) -> String {
    let mut len = 0usize;
    loop {
        let mut line = String::new();
        if r.read_line(&mut line).unwrap() == 0 {
            return String::new();
        }
        let t = line.trim_end_matches(['\r', '\n']);
        if t.is_empty() {
            break;
        }
        if let Some(v) = t.strip_prefix("Content-Length:") {
            len = v.trim().parse().unwrap();
        }
    }
    let mut buf = vec![0u8; len];
    r.read_exact(&mut buf).unwrap();
    String::from_utf8(buf).unwrap()
}

#[test]
fn full_session_over_stdio() {
    let mut child = Child(
        Command::new(env!("CARGO_BIN_EXE_maca-lsp"))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .expect("spawn maca-lsp"),
    );

    let mut stdin = child.0.stdin.take().unwrap();
    let mut stdout = BufReader::new(child.0.stdout.take().unwrap());

    stdin
        .write_all(
            frame(r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}"#).as_bytes(),
        )
        .unwrap();
    let init = read_frame(&mut stdout);
    assert!(
        init.contains("hoverProvider"),
        "initialize response: {init}"
    );

    let open = r#"{"jsonrpc":"2.0","method":"textDocument/didOpen","params":{"textDocument":{"uri":"file:///t.maca","text":"f() -> int => \"bad\"\n"}}}"#;
    stdin.write_all(frame(open).as_bytes()).unwrap();
    let diag = read_frame(&mut stdout);
    assert!(diag.contains("publishDiagnostics"), "diagnostics: {diag}");
    assert!(diag.contains("TypeMismatch"), "diagnostics: {diag}");

    let hover = r#"{"jsonrpc":"2.0","id":2,"method":"textDocument/hover","params":{"textDocument":{"uri":"file:///t.maca"},"position":{"line":0,"character":0}}}"#;
    stdin.write_all(frame(hover).as_bytes()).unwrap();
    let hov = read_frame(&mut stdout);
    assert!(hov.contains("-> int"), "hover: {hov}");

    stdin
        .write_all(
            frame(r#"{"jsonrpc":"2.0","id":3,"method":"shutdown","params":null}"#).as_bytes(),
        )
        .unwrap();
    let _ = read_frame(&mut stdout);
    stdin
        .write_all(frame(r#"{"jsonrpc":"2.0","method":"exit"}"#).as_bytes())
        .unwrap();

    let status = child.0.wait().expect("wait");
    assert!(
        status.success() || status.code().is_none(),
        "server exited abnormally: {status:?}"
    );
}

/// Drive `textDocument/references` and `textDocument/rename` over a real session.
#[test]
fn references_and_rename_over_stdio() {
    let mut child = Child(
        Command::new(env!("CARGO_BIN_EXE_maca-lsp"))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .expect("spawn maca-lsp"),
    );
    let mut stdin = child.0.stdin.take().unwrap();
    let mut stdout = BufReader::new(child.0.stdout.take().unwrap());

    stdin
        .write_all(
            frame(r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}"#).as_bytes(),
        )
        .unwrap();
    let init = read_frame(&mut stdout);
    assert!(init.contains("referencesProvider"), "initialize: {init}");
    assert!(init.contains("renameProvider"), "initialize: {init}");

    let text = "twice(n: int) -> int => n * 2\\n// twice helper\\nmain() -> int { twice(1) + twice(2) }\\n";
    let open = format!(
        r#"{{"jsonrpc":"2.0","method":"textDocument/didOpen","params":{{"textDocument":{{"uri":"file:///r.maca","text":"{text}"}}}}}}"#
    );
    stdin.write_all(frame(&open).as_bytes()).unwrap();
    let _diag = read_frame(&mut stdout);

    let refs = r#"{"jsonrpc":"2.0","id":2,"method":"textDocument/references","params":{"textDocument":{"uri":"file:///r.maca"},"position":{"line":0,"character":0}}}"#;
    stdin.write_all(frame(refs).as_bytes()).unwrap();
    let got = read_frame(&mut stdout);
    assert_eq!(
        got.matches("file:///r.maca").count(),
        3,
        "expected 3 reference locations (def + 2 uses): {got}"
    );

    let ren = r#"{"jsonrpc":"2.0","id":3,"method":"textDocument/rename","params":{"textDocument":{"uri":"file:///r.maca"},"position":{"line":0,"character":0},"newName":"double"}}"#;
    stdin.write_all(frame(ren).as_bytes()).unwrap();
    let edit = read_frame(&mut stdout);
    assert!(edit.contains("changes"), "rename response: {edit}");
    assert_eq!(
        edit.matches("\"double\"").count(),
        3,
        "expected one edit per occurrence: {edit}"
    );

    stdin
        .write_all(
            frame(r#"{"jsonrpc":"2.0","id":4,"method":"shutdown","params":null}"#).as_bytes(),
        )
        .unwrap();
    let _ = read_frame(&mut stdout);
    stdin
        .write_all(frame(r#"{"jsonrpc":"2.0","method":"exit"}"#).as_bytes())
        .unwrap();
}

/// A real workspace on disk: a module, two files that import it, and one that happens to define the same name without importing anything.
#[test]
fn renaming_a_top_level_name_reaches_every_importer() {
    let root = scratch("rename-ws");
    std::fs::create_dir_all(root.join("lib")).unwrap();
    std::fs::write(
        root.join("lib/util.maca"),
        "helper(n: int) -> int => n * 2\n",
    )
    .unwrap();
    std::fs::write(
        root.join("other.maca"),
        "import lib/util\n\ngo() -> int => helper(1)\n",
    )
    .unwrap();
    std::fs::write(
        root.join("unrelated.maca"),
        "helper(n: int) -> int => n\n\ngo2() -> int => helper(9)\n",
    )
    .unwrap();

    let main = root.join("main.maca");
    let text = "import lib/util\\n\\nmain() -> int => helper(3)\\n";
    std::fs::write(&main, text.replace("\\n", "\n")).unwrap();

    let mut child = Child(
        Command::new(env!("CARGO_BIN_EXE_maca-lsp"))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .expect("spawn maca-lsp"),
    );
    let mut stdin = child.0.stdin.take().unwrap();
    let mut stdout = BufReader::new(child.0.stdout.take().unwrap());

    let init = format!(
        r#"{{"jsonrpc":"2.0","id":1,"method":"initialize","params":{{"rootUri":"{}"}}}}"#,
        file_uri(&root)
    );
    stdin.write_all(frame(&init).as_bytes()).unwrap();
    let caps = read_frame(&mut stdout);
    assert!(caps.contains("prepareProvider"), "initialize: {caps}");
    assert!(caps.contains("documentHighlightProvider"), "{caps}");

    let uri = file_uri(&main);
    let open = format!(
        r#"{{"jsonrpc":"2.0","method":"textDocument/didOpen","params":{{"textDocument":{{"uri":"{uri}","text":"{text}"}}}}}}"#
    );
    stdin.write_all(frame(&open).as_bytes()).unwrap();
    let _diag = read_frame(&mut stdout);

    let ren = format!(
        r#"{{"jsonrpc":"2.0","id":2,"method":"textDocument/rename","params":{{"textDocument":{{"uri":"{uri}"}},"position":{{"line":2,"character":19}},"newName":"twice"}}}}"#
    );
    stdin.write_all(frame(&ren).as_bytes()).unwrap();
    let edit = read_frame(&mut stdout);

    assert!(edit.contains("lib/util.maca"), "the definition: {edit}");
    assert!(edit.contains("other.maca"), "the other importer: {edit}");
    assert!(
        !edit.contains("unrelated.maca"),
        "renamed a same-named definition in a file that never imported it: {edit}"
    );
    assert_eq!(
        edit.matches("\"twice\"").count(),
        3,
        "definition + two call sites: {edit}"
    );

    stdin
        .write_all(
            frame(r#"{"jsonrpc":"2.0","id":3,"method":"shutdown","params":null}"#).as_bytes(),
        )
        .unwrap();
    let _ = read_frame(&mut stdout);
    stdin
        .write_all(frame(r#"{"jsonrpc":"2.0","method":"exit"}"#).as_bytes())
        .unwrap();

    let _ = std::fs::remove_dir_all(&root);
}

/// `prepareRename` tells the editor what the cursor is on before it opens the rename box.
#[test]
fn prepare_rename_reports_the_name_under_the_cursor() {
    let mut child = Child(
        Command::new(env!("CARGO_BIN_EXE_maca-lsp"))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .expect("spawn maca-lsp"),
    );
    let mut stdin = child.0.stdin.take().unwrap();
    let mut stdout = BufReader::new(child.0.stdout.take().unwrap());

    stdin
        .write_all(
            frame(r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}"#).as_bytes(),
        )
        .unwrap();
    let _ = read_frame(&mut stdout);

    let open = r#"{"jsonrpc":"2.0","method":"textDocument/didOpen","params":{"textDocument":{"uri":"file:///p.maca","text":"twice(n: int) -> int => n * 2\n"}}}"#;
    stdin.write_all(frame(open).as_bytes()).unwrap();
    let _ = read_frame(&mut stdout);

    let prep = r#"{"jsonrpc":"2.0","id":2,"method":"textDocument/prepareRename","params":{"textDocument":{"uri":"file:///p.maca"},"position":{"line":0,"character":2}}}"#;
    stdin.write_all(frame(prep).as_bytes()).unwrap();
    let got = read_frame(&mut stdout);
    assert!(got.contains("\"placeholder\":\"twice\""), "prepare: {got}");

    let none = r#"{"jsonrpc":"2.0","id":3,"method":"textDocument/prepareRename","params":{"textDocument":{"uri":"file:///p.maca"},"position":{"line":0,"character":21}}}"#;
    stdin.write_all(frame(none).as_bytes()).unwrap();
    let got = read_frame(&mut stdout);
    assert!(got.contains("\"result\":null"), "expected null: {got}");

    stdin
        .write_all(frame(r#"{"jsonrpc":"2.0","method":"exit"}"#).as_bytes())
        .unwrap();
}

/// Spawn a server, initialize it (optionally with a root), and hand back the pipes.
fn session(
    root: Option<&std::path::Path>,
) -> (
    Child,
    std::process::ChildStdin,
    BufReader<std::process::ChildStdout>,
) {
    let mut child = Child(
        Command::new(env!("CARGO_BIN_EXE_maca-lsp"))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .expect("spawn maca-lsp"),
    );
    let mut stdin = child.0.stdin.take().unwrap();
    let mut stdout = BufReader::new(child.0.stdout.take().unwrap());
    let params = match root {
        Some(r) => format!(r#"{{"rootUri":"{}"}}"#, file_uri(r)),
        None => "{}".to_string(),
    };
    let init = format!(r#"{{"jsonrpc":"2.0","id":1,"method":"initialize","params":{params}}}"#);
    stdin.write_all(frame(&init).as_bytes()).unwrap();
    let _ = read_frame(&mut stdout);
    (child, stdin, stdout)
}

/// Every request carrying an `id` must get a response, including the ones that have nothing to say.
#[test]
fn every_request_gets_a_response() {
    let (_child, mut stdin, mut stdout) = session(None);

    let hover = r#"{"jsonrpc":"2.0","id":2,"method":"textDocument/hover","params":{"textDocument":{"uri":"file:///never-opened.maca"},"position":{"line":0,"character":0}}}"#;
    stdin.write_all(frame(hover).as_bytes()).unwrap();
    assert!(
        read_frame(&mut stdout).contains("\"id\":2"),
        "hover on an unknown document"
    );

    let src = "main() -> int {\n    if true {\n        1\n    } else {\n        0\n    }\n}\n";
    let open = format!(
        r#"{{"jsonrpc":"2.0","method":"textDocument/didOpen","params":{{"textDocument":{{"uri":"file:///k.maca","text":"{}"}}}}}}"#,
        src.replace('\n', "\\n")
    );
    stdin.write_all(frame(&open).as_bytes()).unwrap();
    let _ = read_frame(&mut stdout);

    let ren = r#"{"jsonrpc":"2.0","id":3,"method":"textDocument/rename","params":{"textDocument":{"uri":"file:///k.maca"},"position":{"line":1,"character":5},"newName":"x"}}"#;
    stdin.write_all(frame(ren).as_bytes()).unwrap();
    let got = read_frame(&mut stdout);
    assert!(got.contains("\"id\":3"), "rename on a keyword: {got}");

    for (id, method) in [
        (4, "references"),
        (5, "definition"),
        (6, "documentHighlight"),
    ] {
        let req = format!(
            r#"{{"jsonrpc":"2.0","id":{id},"method":"textDocument/{method}","params":{{"textDocument":{{"uri":"file:///gone.maca"}},"position":{{"line":0,"character":0}}}}}}"#
        );
        stdin.write_all(frame(&req).as_bytes()).unwrap();
        let got = read_frame(&mut stdout);
        assert!(got.contains(&format!("\"id\":{id}")), "{method}: {got}");
    }

    stdin
        .write_all(frame(r#"{"jsonrpc":"2.0","method":"exit"}"#).as_bytes())
        .unwrap();
}

/// A name typed into the rename box that isn't a name at all must be refused, and the refusal has to be visible.
#[test]
fn an_invalid_new_name_is_refused_out_loud() {
    let (_child, mut stdin, mut stdout) = session(None);
    let open = r#"{"jsonrpc":"2.0","method":"textDocument/didOpen","params":{"textDocument":{"uri":"file:///v.maca","text":"twice(n: int) -> int => n * 2\n"}}}"#;
    stdin.write_all(frame(open).as_bytes()).unwrap();
    let _ = read_frame(&mut stdout);

    let ren = r#"{"jsonrpc":"2.0","id":2,"method":"textDocument/rename","params":{"textDocument":{"uri":"file:///v.maca"},"position":{"line":0,"character":1},"newName":"if"}}"#;
    stdin.write_all(frame(ren).as_bytes()).unwrap();
    let told = read_frame(&mut stdout);
    assert!(told.contains("window/showMessage"), "no warning: {told}");
    let got = read_frame(&mut stdout);
    assert!(got.contains("\"result\":null"), "expected refusal: {got}");

    stdin
        .write_all(frame(r#"{"jsonrpc":"2.0","method":"exit"}"#).as_bytes())
        .unwrap();
}

/// Maca inlines imports transitively, so a definition two modules away is still the one a call resolves to.
#[test]
fn a_rename_follows_imports_transitively() {
    let root = scratch("transitive");
    std::fs::create_dir_all(root.join("lib")).unwrap();
    std::fs::write(
        root.join("lib/base.maca"),
        "helper(n: int) -> int => n * 2\n",
    )
    .unwrap();
    std::fs::write(
        root.join("lib/mid.maca"),
        "import lib/base\n\nwrap(n: int) -> int => helper(n)\n",
    )
    .unwrap();
    let main = root.join("main.maca");
    let text = "import lib/mid\\n\\nmain() -> int => helper(3) + wrap(4)\\n";
    std::fs::write(&main, text.replace("\\n", "\n")).unwrap();

    let (_child, mut stdin, mut stdout) = session(Some(&root));
    let uri = file_uri(&main);
    let open = format!(
        r#"{{"jsonrpc":"2.0","method":"textDocument/didOpen","params":{{"textDocument":{{"uri":"{uri}","text":"{text}"}}}}}}"#
    );
    stdin.write_all(frame(&open).as_bytes()).unwrap();
    let _ = read_frame(&mut stdout);

    let ren = format!(
        r#"{{"jsonrpc":"2.0","id":2,"method":"textDocument/rename","params":{{"textDocument":{{"uri":"{uri}"}},"position":{{"line":2,"character":19}},"newName":"twice"}}}}"#
    );
    stdin.write_all(frame(&ren).as_bytes()).unwrap();
    let edit = read_frame(&mut stdout);
    assert!(
        edit.contains("base.maca"),
        "the definition, two modules away: {edit}"
    );
    assert!(edit.contains("mid.maca"), "the module in between: {edit}");

    stdin
        .write_all(frame(r#"{"jsonrpc":"2.0","method":"exit"}"#).as_bytes())
        .unwrap();
    let _ = std::fs::remove_dir_all(&root);
}

/// A field rename is single-file, so a field whose record is declared in another module can only be renamed half-way.
#[test]
fn a_field_declared_elsewhere_is_not_renamed_half_way() {
    let root = scratch("field-elsewhere");
    std::fs::create_dir_all(root.join("lib")).unwrap();
    std::fs::write(root.join("lib/util.maca"), "Cfg = {\n    mode: str\n}\n").unwrap();
    let main = root.join("main.maca");
    let text = "import lib/util\\n\\nmain() -> int {\\n    c = Cfg { mode = \\\"x\\\" }\\n    c.mode.length()\\n}\\n";
    std::fs::write(&main, text.replace("\\n", "\n").replace("\\\"", "\"")).unwrap();

    let (_child, mut stdin, mut stdout) = session(Some(&root));
    let uri = file_uri(&main);
    let open = format!(
        r#"{{"jsonrpc":"2.0","method":"textDocument/didOpen","params":{{"textDocument":{{"uri":"{uri}","text":"{text}"}}}}}}"#
    );
    stdin.write_all(frame(&open).as_bytes()).unwrap();
    let _ = read_frame(&mut stdout);

    let ren = format!(
        r#"{{"jsonrpc":"2.0","id":2,"method":"textDocument/rename","params":{{"textDocument":{{"uri":"{uri}"}},"position":{{"line":3,"character":14}},"newName":"kind"}}}}"#
    );
    stdin.write_all(frame(&ren).as_bytes()).unwrap();
    let told = read_frame(&mut stdout);
    assert!(told.contains("window/showMessage"), "no warning: {told}");
    let got = read_frame(&mut stdout);
    assert!(got.contains("\"result\":null"), "expected refusal: {got}");

    stdin
        .write_all(frame(r#"{"jsonrpc":"2.0","method":"exit"}"#).as_bytes())
        .unwrap();
    let _ = std::fs::remove_dir_all(&root);
}

/// A project directory with a space or a non-ASCII name must survive the URI round-trip.
#[test]
fn a_workspace_path_survives_the_uri_round_trip() {
    for name in ["my proj", "프로젝트"] {
        let root = scratch("uri").join(name);
        std::fs::create_dir_all(root.join("lib")).unwrap();
        std::fs::write(
            root.join("lib/util.maca"),
            "helper(n: int) -> int => n * 2\n",
        )
        .unwrap();
        let main = root.join("main.maca");
        let text = "import lib/util\\n\\nmain() -> int => helper(3)\\n";
        std::fs::write(&main, text.replace("\\n", "\n")).unwrap();

        let (_child, mut stdin, mut stdout) = {
            let mut child = Child(
                Command::new(env!("CARGO_BIN_EXE_maca-lsp"))
                    .stdin(Stdio::piped())
                    .stdout(Stdio::piped())
                    .spawn()
                    .expect("spawn maca-lsp"),
            );
            let mut si = child.0.stdin.take().unwrap();
            let mut so = BufReader::new(child.0.stdout.take().unwrap());
            let init = format!(
                r#"{{"jsonrpc":"2.0","id":1,"method":"initialize","params":{{"rootUri":"{}"}}}}"#,
                file_uri(&root)
            );
            si.write_all(frame(&init).as_bytes()).unwrap();
            let _ = read_frame(&mut so);
            (child, si, so)
        };

        let uri = file_uri(&main);
        let open = format!(
            r#"{{"jsonrpc":"2.0","method":"textDocument/didOpen","params":{{"textDocument":{{"uri":"{uri}","text":"{text}"}}}}}}"#
        );
        stdin.write_all(frame(&open).as_bytes()).unwrap();
        let _ = read_frame(&mut stdout);

        let ren = format!(
            r#"{{"jsonrpc":"2.0","id":2,"method":"textDocument/rename","params":{{"textDocument":{{"uri":"{uri}"}},"position":{{"line":2,"character":19}},"newName":"twice"}}}}"#
        );
        stdin.write_all(frame(&ren).as_bytes()).unwrap();
        let edit = read_frame(&mut stdout);
        assert!(
            edit.contains("util.maca"),
            "the definition was never reached under {name:?}: {edit}"
        );
        assert!(
            edit.contains(&uri),
            "the open document's own URI changed under {name:?}: {edit}"
        );
        assert!(
            !edit.contains("file:///tmp/maca-lsp-uri") || !edit.contains(&format!("/{name}/")),
            "an unescaped path leaked into a key under {name:?}: {edit}"
        );

        stdin
            .write_all(frame(r#"{"jsonrpc":"2.0","method":"exit"}"#).as_bytes())
            .unwrap();
        let _ = std::fs::remove_dir_all(&root);
    }
}

/// A file that imports the module for a *different* name cannot see this one.
#[test]
fn a_selective_import_only_carries_the_names_it_asks_for() {
    let root = scratch("selective");
    std::fs::create_dir_all(root.join("lib")).unwrap();
    std::fs::write(
        root.join("lib/util.maca"),
        "helper(n: int) -> int => n * 2\n\nother(n: int) -> int => n\n",
    )
    .unwrap();
    std::fs::write(root.join("lib/two.maca"), "helper(s: str) -> str => s\n").unwrap();
    std::fs::write(
        root.join("sel.maca"),
        "import { other } from lib/util\nimport lib/two\n\ngo() -> str => helper(\"hi\")\n",
    )
    .unwrap();
    let main = root.join("main.maca");
    let text = "import lib/util\\n\\nmain() -> int => helper(3)\\n";
    std::fs::write(&main, text.replace("\\n", "\n")).unwrap();

    let (_child, mut stdin, mut stdout) = session(Some(&root));
    let uri = file_uri(&main);
    let open = format!(
        r#"{{"jsonrpc":"2.0","method":"textDocument/didOpen","params":{{"textDocument":{{"uri":"{uri}","text":"{text}"}}}}}}"#
    );
    stdin.write_all(frame(&open).as_bytes()).unwrap();
    let _ = read_frame(&mut stdout);

    let ren = format!(
        r#"{{"jsonrpc":"2.0","id":2,"method":"textDocument/rename","params":{{"textDocument":{{"uri":"{uri}"}},"position":{{"line":2,"character":19}},"newName":"twice"}}}}"#
    );
    stdin.write_all(frame(&ren).as_bytes()).unwrap();
    let edit = read_frame(&mut stdout);
    assert!(edit.contains("util.maca"), "the definition: {edit}");
    assert!(
        !edit.contains("sel.maca"),
        "renamed a different module's own `helper`: {edit}"
    );

    stdin
        .write_all(frame(r#"{"jsonrpc":"2.0","method":"exit"}"#).as_bytes())
        .unwrap();
    let _ = std::fs::remove_dir_all(&root);
}

/// The lightbulb, end to end: the capability has to be advertised or no editor ever asks, and the answer has to be a workspace edit the editor can apply as it stands.
#[test]
fn a_quick_fix_comes_back_as_a_workspace_edit() {
    let (_child, mut stdin, mut stdout) = session(None);
    let src = "f() -> int {\n    let x = 1\n    x\n}\n";
    let open = format!(
        r#"{{"jsonrpc":"2.0","method":"textDocument/didOpen","params":{{"textDocument":{{"uri":"file:///q.maca","text":"{}"}}}}}}"#,
        src.replace('\n', "\\n")
    );
    stdin.write_all(frame(&open).as_bytes()).unwrap();
    let diag = read_frame(&mut stdout);
    assert!(diag.contains("UndefinedName"), "diagnostics: {diag}");

    let ask = r#"{"jsonrpc":"2.0","id":2,"method":"textDocument/codeAction","params":{"textDocument":{"uri":"file:///q.maca"},"range":{"start":{"line":1,"character":4},"end":{"line":1,"character":4}},"context":{"diagnostics":[]}}}"#;
    stdin.write_all(frame(ask).as_bytes()).unwrap();
    let got = read_frame(&mut stdout);
    assert!(got.contains("drop the `let`"), "code actions: {got}");
    assert!(got.contains("\"quickfix\""), "no kind: {got}");
    assert!(got.contains("file:///q.maca"), "edit is not keyed: {got}");

    let none = r#"{"jsonrpc":"2.0","id":3,"method":"textDocument/codeAction","params":{"textDocument":{"uri":"file:///q.maca"},"range":{"start":{"line":2,"character":4},"end":{"line":2,"character":4}},"context":{"diagnostics":[]}}}"#;
    stdin.write_all(frame(none).as_bytes()).unwrap();
    let empty = read_frame(&mut stdout);
    assert!(empty.contains("\"result\":[]"), "expected none: {empty}");

    stdin
        .write_all(frame(r#"{"jsonrpc":"2.0","method":"exit"}"#).as_bytes())
        .unwrap();
}
