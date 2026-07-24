//! End-to-end: spawn the real `maca-lsp` binary and drive a full LSP session
//! over stdio (Content-Length framing), asserting the responses.

use std::io::{BufRead, BufReader, Read as _, Write};
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
