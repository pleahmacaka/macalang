//! maca-lsp: a Language Server for `.maca`, speaking LSP over stdio with
//! `Content-Length` framing. Wraps the pure analysis functions in `lib.rs`:
//! live diagnostics (parse + type/effect), hover (signatures/types), and
//! completion (config option namespaces or top-level names).
//!
//! Editors launch this binary and talk JSON-RPC to it — see `editor/zed-maca`.

use serde_json::{json, Value};
use std::collections::HashMap;
use std::io::{self, BufRead, Write};

fn main() {
    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut server = Server { docs: HashMap::new() };
    server.run(&mut stdin.lock(), &mut stdout.lock());
}

struct Server {
    docs: HashMap<String, String>, // uri → text
}

impl Server {
    fn run(&mut self, input: &mut impl BufRead, out: &mut impl Write) {
        while let Some(msg) = read_message(input) {
            let method = msg.get("method").and_then(|m| m.as_str()).unwrap_or("");
            if let Some(resp) = self.handle(&msg, out) {
                write_message(out, &resp);
            }
            if method == "exit" {
                break;
            }
        }
    }

    fn handle(&mut self, req: &Value, out: &mut impl Write) -> Option<Value> {
        let method = req.get("method")?.as_str()?;
        let id = req.get("id").cloned();
        match method {
            "initialize" => Some(reply(
                id,
                json!({
                    "capabilities": {
                        "textDocumentSync": 1, // Full
                        "hoverProvider": true,
                        "completionProvider": { "triggerCharacters": ["."] }
                    },
                    "serverInfo": { "name": "maca-lsp", "version": env!("CARGO_PKG_VERSION") }
                }),
            )),
            "textDocument/didOpen" => {
                let td = req.pointer("/params/textDocument")?;
                let uri = td.get("uri")?.as_str()?.to_string();
                let text = td.get("text")?.as_str()?.to_string();
                self.publish(&uri, &text, out);
                self.docs.insert(uri, text);
                None
            }
            "textDocument/didChange" => {
                let uri = req.pointer("/params/textDocument/uri")?.as_str()?.to_string();
                // Full sync: the last content change holds the whole document.
                let changes = req.pointer("/params/contentChanges")?.as_array()?;
                let text = changes.last()?.get("text")?.as_str()?.to_string();
                self.publish(&uri, &text, out);
                self.docs.insert(uri, text);
                None
            }
            "textDocument/hover" => {
                let text = self.doc_at(req)?;
                let off = self.offset_at(req, &text)?;
                let value = maca_lsp::hover(&text, off).unwrap_or_default();
                Some(reply(id, json!({ "contents": { "kind": "plaintext", "value": value } })))
            }
            "textDocument/completion" => {
                let text = self.doc_at(req).unwrap_or_default();
                let off = self.offset_at(req, &text).unwrap_or(0);
                let prefix = maca_lsp::prefix_at(&text, off);
                let items = self.completions(&text, &prefix);
                Some(reply(id, json!(items.into_iter().map(|l| json!({ "label": l })).collect::<Vec<_>>())))
            }
            "shutdown" => Some(reply(id, Value::Null)),
            _ if id.is_some() => Some(error(id, -32601, "method not found")),
            _ => None, // an unhandled notification
        }
    }

    fn doc_at(&self, req: &Value) -> Option<String> {
        let uri = req.pointer("/params/textDocument/uri")?.as_str()?;
        self.docs.get(uri).cloned()
    }

    fn offset_at(&self, req: &Value, text: &str) -> Option<usize> {
        let pos = req.pointer("/params/position")?;
        let line = pos.get("line")?.as_u64()? as usize;
        let ch = pos.get("character")?.as_u64()? as usize;
        Some(maca_lsp::position_to_offset(text, line, ch))
    }

    fn completions(&self, text: &str, prefix: &str) -> Vec<String> {
        // a dotted prefix's last segment drives config namespace completion
        let last = prefix.rsplit('.').next().unwrap_or(prefix);
        if maca_lsp::is_config_source(text) {
            maca_lsp::config_completions(last)
        } else {
            maca_lsp::program_completions(text, last)
        }
    }

    fn publish(&self, uri: &str, text: &str, out: &mut impl Write) {
        let config = maca_lsp::is_config_source(text);
        let diags: Vec<Value> = maca_lsp::diagnostics(text, config)
            .into_iter()
            .map(|m| {
                json!({
                    "range": { "start": {"line": 0, "character": 0}, "end": {"line": 0, "character": 1} },
                    "severity": 1, // Error
                    "source": "maca",
                    "message": m,
                })
            })
            .collect();
        let note = json!({
            "jsonrpc": "2.0",
            "method": "textDocument/publishDiagnostics",
            "params": { "uri": uri, "diagnostics": diags }
        });
        write_message(out, &note);
    }
}

fn reply(id: Option<Value>, result: Value) -> Value {
    json!({ "jsonrpc": "2.0", "id": id.unwrap_or(Value::Null), "result": result })
}

fn error(id: Option<Value>, code: i64, message: &str) -> Value {
    json!({ "jsonrpc": "2.0", "id": id.unwrap_or(Value::Null), "error": { "code": code, "message": message } })
}

/// Read one `Content-Length`-framed JSON-RPC message from `input`.
fn read_message(input: &mut impl BufRead) -> Option<Value> {
    let mut len = 0usize;
    loop {
        let mut line = String::new();
        if input.read_line(&mut line).ok()? == 0 {
            return None; // EOF
        }
        let trimmed = line.trim_end_matches(['\r', '\n']);
        if trimmed.is_empty() {
            break; // end of headers
        }
        if let Some(v) = trimmed.strip_prefix("Content-Length:") {
            len = v.trim().parse().ok()?;
        }
    }
    if len == 0 {
        return None;
    }
    let mut buf = vec![0u8; len];
    input.read_exact(&mut buf).ok()?;
    serde_json::from_slice(&buf).ok()
}

/// Write a JSON value as a `Content-Length`-framed message.
fn write_message(out: &mut impl Write, msg: &Value) {
    let body = msg.to_string();
    let _ = write!(out, "Content-Length: {}\r\n\r\n{}", body.len(), body);
    let _ = out.flush();
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    fn frame(msg: &Value) -> Vec<u8> {
        let body = msg.to_string();
        format!("Content-Length: {}\r\n\r\n{}", body.len(), body).into_bytes()
    }

    #[test]
    fn frames_roundtrip() {
        let m = json!({ "jsonrpc": "2.0", "id": 1, "method": "initialize" });
        let mut cur = Cursor::new(frame(&m));
        let got = read_message(&mut cur).expect("a message");
        assert_eq!(got["method"], "initialize");
    }

    #[test]
    fn initialize_advertises_capabilities() {
        let mut s = Server { docs: HashMap::new() };
        let mut out = Vec::new();
        let resp = s
            .handle(&json!({ "jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {} }), &mut out)
            .unwrap();
        assert_eq!(resp["result"]["capabilities"]["hoverProvider"], true);
        assert!(resp["result"]["capabilities"].get("completionProvider").is_some());
    }

    #[test]
    fn didopen_publishes_diagnostics_for_bad_code() {
        let mut s = Server { docs: HashMap::new() };
        let mut out = Vec::new();
        // a type error: return type int, body is a string
        let src = "f() -> int => \"oops\"\n";
        s.handle(
            &json!({
                "jsonrpc": "2.0",
                "method": "textDocument/didOpen",
                "params": { "textDocument": { "uri": "file:///t.maca", "text": src } }
            }),
            &mut out,
        );
        let text = String::from_utf8(out).unwrap();
        assert!(text.contains("publishDiagnostics"), "no diagnostics notification: {text}");
        assert!(text.contains("TypeMismatch"), "expected a type error: {text}");
    }

    #[test]
    fn hover_returns_a_signature() {
        let mut s = Server { docs: HashMap::new() };
        let mut out = Vec::new();
        let src = "add(a: int, b: int) -> int => a + b\n";
        s.handle(
            &json!({ "method": "textDocument/didOpen",
                "params": { "textDocument": { "uri": "file:///t.maca", "text": src } } }),
            &mut out,
        );
        // hover over `add` (line 0, char 1)
        let resp = s
            .handle(
                &json!({ "id": 2, "method": "textDocument/hover",
                    "params": { "textDocument": { "uri": "file:///t.maca" }, "position": { "line": 0, "character": 1 } } }),
                &mut out,
            )
            .unwrap();
        let v = resp["result"]["contents"]["value"].as_str().unwrap_or("");
        assert!(v.contains("add(") && v.contains("-> int"), "hover: {v}");
    }

    #[test]
    fn config_completion_after_import() {
        let mut s = Server { docs: HashMap::new() };
        let mut out = Vec::new();
        let src = "import nixpkgs\nsys";
        s.handle(
            &json!({ "method": "textDocument/didOpen",
                "params": { "textDocument": { "uri": "file:///c.maca", "text": src } } }),
            &mut out,
        );
        let resp = s
            .handle(
                &json!({ "id": 3, "method": "textDocument/completion",
                    "params": { "textDocument": { "uri": "file:///c.maca" }, "position": { "line": 1, "character": 3 } } }),
                &mut out,
            )
            .unwrap();
        let labels: Vec<String> = resp["result"]
            .as_array()
            .unwrap()
            .iter()
            .map(|i| i["label"].as_str().unwrap().to_string())
            .collect();
        assert!(labels.contains(&"system".to_string()), "labels: {labels:?}");
    }
}
