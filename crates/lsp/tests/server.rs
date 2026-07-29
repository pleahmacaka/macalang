//! End-to-end: spawn the real `maca-lsp` binary and drive a full LSP session
//! over stdio (Content-Length framing), asserting the responses.

use std::io::{BufRead, BufReader, Write};
use std::process::{Command, Stdio};

fn frame(body: &str) -> String {
    format!("Content-Length: {}\r\n\r\n{}", body.len(), body)
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
    let mut child = Command::new(env!("CARGO_BIN_EXE_maca-lsp"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("spawn maca-lsp");

    let mut stdin = child.stdin.take().unwrap();
    let mut stdout = BufReader::new(child.stdout.take().unwrap());

    // initialize
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

    // didOpen a program with a type error
    let open = r#"{"jsonrpc":"2.0","method":"textDocument/didOpen","params":{"textDocument":{"uri":"file:///t.maca","text":"f() -> int => \"bad\"\n"}}}"#;
    stdin.write_all(frame(open).as_bytes()).unwrap();
    let diag = read_frame(&mut stdout);
    assert!(diag.contains("publishDiagnostics"), "diagnostics: {diag}");
    assert!(diag.contains("TypeMismatch"), "diagnostics: {diag}");

    // hover over `f`
    let hover = r#"{"jsonrpc":"2.0","id":2,"method":"textDocument/hover","params":{"textDocument":{"uri":"file:///t.maca"},"position":{"line":0,"character":0}}}"#;
    stdin.write_all(frame(hover).as_bytes()).unwrap();
    let hov = read_frame(&mut stdout);
    assert!(hov.contains("-> int"), "hover: {hov}");

    // shutdown + exit
    stdin
        .write_all(
            frame(r#"{"jsonrpc":"2.0","id":3,"method":"shutdown","params":null}"#).as_bytes(),
        )
        .unwrap();
    let _ = read_frame(&mut stdout);
    stdin
        .write_all(frame(r#"{"jsonrpc":"2.0","method":"exit"}"#).as_bytes())
        .unwrap();

    let status = child.wait().expect("wait");
    assert!(
        status.success() || status.code().is_none(),
        "server exited abnormally: {status:?}"
    );
}

/// Drive `textDocument/references` and `textDocument/rename` over a real
/// session: a symbol used twice must yield three locations, and renaming it
/// must produce one edit per location (and none for the comment mention).
#[test]
fn references_and_rename_over_stdio() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_maca-lsp"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("spawn maca-lsp");
    let mut stdin = child.stdin.take().unwrap();
    let mut stdout = BufReader::new(child.stdout.take().unwrap());

    stdin
        .write_all(
            frame(r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}"#).as_bytes(),
        )
        .unwrap();
    let init = read_frame(&mut stdout);
    assert!(init.contains("referencesProvider"), "initialize: {init}");
    assert!(init.contains("renameProvider"), "initialize: {init}");

    // `twice` on line 0 (definition), called twice on line 2; line 1 is a
    // comment mentioning it, which must be ignored.
    let text = "twice(n: int) -> int => n * 2\\n// twice helper\\nmain() -> int { twice(1) + twice(2) }\\n";
    let open = format!(
        r#"{{"jsonrpc":"2.0","method":"textDocument/didOpen","params":{{"textDocument":{{"uri":"file:///r.maca","text":"{text}"}}}}}}"#
    );
    stdin.write_all(frame(&open).as_bytes()).unwrap();
    let _diag = read_frame(&mut stdout);

    // references at the definition
    let refs = r#"{"jsonrpc":"2.0","id":2,"method":"textDocument/references","params":{"textDocument":{"uri":"file:///r.maca"},"position":{"line":0,"character":0}}}"#;
    stdin.write_all(frame(refs).as_bytes()).unwrap();
    let got = read_frame(&mut stdout);
    assert_eq!(
        got.matches("file:///r.maca").count(),
        3,
        "expected 3 reference locations (def + 2 uses): {got}"
    );

    // rename to `double`
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
    let _ = child.wait();
}

/// A real workspace on disk: a module, two files that import it, and one that
/// happens to define the same name without importing anything.
///
/// Renaming a top-level definition has to reach every importer. Editing only
/// the open file leaves them calling a name that no longer exists — the editor
/// reports success and the build breaks, which is the one outcome a rename must
/// not have. The file that never imported it must be left alone.
#[test]
fn renaming_a_top_level_name_reaches_every_importer() {
    let root = std::env::temp_dir().join("maca-lsp-rename-ws");
    let _ = std::fs::remove_dir_all(&root);
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
    // same name, no import — a different `helper` entirely
    std::fs::write(
        root.join("unrelated.maca"),
        "helper(n: int) -> int => n\n\ngo2() -> int => helper(9)\n",
    )
    .unwrap();

    let main = root.join("main.maca");
    let text = "import lib/util\\n\\nmain() -> int => helper(3)\\n";
    std::fs::write(&main, text.replace("\\n", "\n")).unwrap();

    let mut child = Command::new(env!("CARGO_BIN_EXE_maca-lsp"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("spawn maca-lsp");
    let mut stdin = child.stdin.take().unwrap();
    let mut stdout = BufReader::new(child.stdout.take().unwrap());

    let init = format!(
        r#"{{"jsonrpc":"2.0","id":1,"method":"initialize","params":{{"rootUri":"file://{}"}}}}"#,
        root.display()
    );
    stdin.write_all(frame(&init).as_bytes()).unwrap();
    let caps = read_frame(&mut stdout);
    assert!(caps.contains("prepareProvider"), "initialize: {caps}");
    assert!(caps.contains("documentHighlightProvider"), "{caps}");

    let uri = format!("file://{}", main.display());
    let open = format!(
        r#"{{"jsonrpc":"2.0","method":"textDocument/didOpen","params":{{"textDocument":{{"uri":"{uri}","text":"{text}"}}}}}}"#
    );
    stdin.write_all(frame(&open).as_bytes()).unwrap();
    let _diag = read_frame(&mut stdout);

    // the cursor is on the *call*, not the definition — the rename still has to
    // find the module that defines it
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
    let _ = child.wait();
    let _ = std::fs::remove_dir_all(&root);
}

/// `prepareRename` tells the editor what the cursor is on before it opens the
/// rename box, so the box arrives pre-filled and never opens over a keyword.
#[test]
fn prepare_rename_reports_the_name_under_the_cursor() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_maca-lsp"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("spawn maca-lsp");
    let mut stdin = child.stdin.take().unwrap();
    let mut stdout = BufReader::new(child.stdout.take().unwrap());

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

    // on whitespace there is nothing to rename, and the editor must be told so
    let none = r#"{"jsonrpc":"2.0","id":3,"method":"textDocument/prepareRename","params":{"textDocument":{"uri":"file:///p.maca"},"position":{"line":0,"character":21}}}"#;
    stdin.write_all(frame(none).as_bytes()).unwrap();
    let got = read_frame(&mut stdout);
    assert!(got.contains("\"result\":null"), "expected null: {got}");

    stdin
        .write_all(frame(r#"{"jsonrpc":"2.0","method":"exit"}"#).as_bytes())
        .unwrap();
    let _ = child.wait();
}
