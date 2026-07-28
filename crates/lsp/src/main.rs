//! maca-lsp: a Language Server for `.maca`, speaking LSP over stdio with
//! `Content-Length` framing. Wraps the pure analysis functions in `lib.rs`:
//! live diagnostics (parse + type/effect), hover (signatures/types), and
//! completion (config option namespaces or top-level names), go-to-definition,
//! document symbols, find-references, rename, and formatting.
//!
//! Editors launch this binary and talk JSON-RPC to it — see `editor/zed-maca`.

use serde_json::{Value, json};
use std::collections::HashMap;
use std::io::{self, BufRead, Write};

fn main() {
    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut server = Server {
        docs: HashMap::new(),
    };
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
                        "completionProvider": { "triggerCharacters": ["."] },
                        "documentSymbolProvider": true,
                        "definitionProvider": true,
                        "referencesProvider": true,
                        "renameProvider": true,
                        "documentFormattingProvider": true
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
                let uri = req
                    .pointer("/params/textDocument/uri")?
                    .as_str()?
                    .to_string();
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
                Some(reply(
                    id,
                    json!({ "contents": { "kind": "plaintext", "value": value } }),
                ))
            }
            "textDocument/completion" => {
                let text = self.doc_at(req).unwrap_or_default();
                let off = self.offset_at(req, &text).unwrap_or(0);
                let prefix = maca_lsp::prefix_at(&text, off);
                let items = self.completions(&text, &prefix);
                Some(reply(
                    id,
                    json!(
                        items
                            .into_iter()
                            .map(|l| json!({ "label": l }))
                            .collect::<Vec<_>>()
                    ),
                ))
            }
            "textDocument/documentSymbol" => {
                let text = self.doc_at(req).unwrap_or_default();
                let syms: Vec<Value> = maca_lsp::document_symbols(&text)
                    .into_iter()
                    .map(|s| {
                        let r = range(&text, s.start, s.end);
                        json!({ "name": s.name, "kind": s.kind, "range": r, "selectionRange": r })
                    })
                    .collect();
                Some(reply(id, json!(syms)))
            }
            "textDocument/definition" => {
                let text = self.doc_at(req)?;
                let off = self.offset_at(req, &text)?;
                let uri = req.pointer("/params/textDocument/uri")?.as_str()?;
                match maca_lsp::definition(&text, off) {
                    Some((s, e)) => Some(reply(
                        id,
                        json!({ "uri": uri, "range": range(&text, s, e) }),
                    )),
                    None => Some(reply(id, Value::Null)),
                }
            }
            "textDocument/references" => {
                let text = self.doc_at(req)?;
                let off = self.offset_at(req, &text)?;
                let uri = req.pointer("/params/textDocument/uri")?.as_str()?;
                let locs: Vec<Value> = maca_lsp::references(&text, off)
                    .into_iter()
                    .map(|(s, e)| json!({ "uri": uri, "range": range(&text, s, e) }))
                    .collect();
                Some(reply(id, json!(locs)))
            }
            "textDocument/rename" => {
                let text = self.doc_at(req)?;
                let off = self.offset_at(req, &text)?;
                let uri = req.pointer("/params/textDocument/uri")?.as_str()?;
                let new_name = req.pointer("/params/newName")?.as_str()?;
                // one edit per occurrence — comments and strings are excluded,
                // so prose mentioning the name is never rewritten.
                let edits: Vec<Value> = maca_lsp::references(&text, off)
                    .into_iter()
                    .map(|(s, e)| json!({ "range": range(&text, s, e), "newText": new_name }))
                    .collect();
                if edits.is_empty() {
                    return Some(reply(id, Value::Null));
                }
                Some(reply(id, json!({ "changes": { uri: edits } })))
            }
            "textDocument/formatting" => {
                let text = self.doc_at(req).unwrap_or_default();
                let parsed = maca_parser::parse(&text);
                // never reformat source that doesn't parse
                if !parsed.errors.is_empty() {
                    return Some(reply(id, json!([])));
                }
                let formatted = maca_parser::print_module(&parsed.module);
                if formatted == text {
                    return Some(reply(id, json!([])));
                }
                let end = range(&text, text.len(), text.len());
                Some(reply(
                    id,
                    json!([{ "range": { "start": {"line": 0, "character": 0}, "end": end["end"] }, "newText": formatted }]),
                ))
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
        let diags: Vec<Value> = maca_lsp::diagnostics_located(text, config)
            .into_iter()
            .map(|d| {
                json!({
                    "range": range(text, d.start, d.end),
                    "severity": 1, // Error
                    "source": "maca",
                    "message": d.message,
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

/// An LSP `Range` json for a `[start, end)` byte span into `text`.
fn range(text: &str, start: usize, end: usize) -> Value {
    let (sl, sc) = maca_lsp::offset_to_position(text, start);
    let (el, ec) = maca_lsp::offset_to_position(text, end);
    json!({ "start": {"line": sl, "character": sc}, "end": {"line": el, "character": ec} })
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
        let mut s = Server {
            docs: HashMap::new(),
        };
        let mut out = Vec::new();
        let resp = s
            .handle(
                &json!({ "jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {} }),
                &mut out,
            )
            .unwrap();
        assert_eq!(resp["result"]["capabilities"]["hoverProvider"], true);
        assert!(
            resp["result"]["capabilities"]
                .get("completionProvider")
                .is_some()
        );
    }

    #[test]
    fn didopen_publishes_diagnostics_for_bad_code() {
        let mut s = Server {
            docs: HashMap::new(),
        };
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
        assert!(
            text.contains("publishDiagnostics"),
            "no diagnostics notification: {text}"
        );
        assert!(
            text.contains("TypeMismatch"),
            "expected a type error: {text}"
        );
    }

    #[test]
    fn hover_returns_a_signature() {
        let mut s = Server {
            docs: HashMap::new(),
        };
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

    fn open(s: &mut Server, out: &mut Vec<u8>, uri: &str, src: &str) {
        s.handle(
            &json!({ "method": "textDocument/didOpen",
                "params": { "textDocument": { "uri": uri, "text": src } } }),
            out,
        );
    }

    #[test]
    fn diagnostics_point_at_the_offending_code() {
        let mut s = Server {
            docs: HashMap::new(),
        };
        let mut out = Vec::new();
        // `slugify` is undefined; the marker must land on line 1, not line 0.
        let src = "main() -> int {\n    x = slugify(1)\n    0\n}\n";
        open(&mut s, &mut out, "file:///t.maca", src);
        let text = String::from_utf8(out).unwrap();
        assert!(text.contains("UndefinedName"), "no diagnostic: {text}");
        assert!(
            text.contains("\"line\":1"),
            "diagnostic not anchored on line 1: {text}"
        );
    }

    #[test]
    fn document_symbols_lists_definitions() {
        let mut s = Server {
            docs: HashMap::new(),
        };
        let mut out = Vec::new();
        let src = "helper() -> int => 1\nPoint = {\n    x: int\n}\nmain() -> int => 0\n";
        open(&mut s, &mut out, "file:///t.maca", src);
        let resp = s
            .handle(
                &json!({ "id": 9, "method": "textDocument/documentSymbol",
                    "params": { "textDocument": { "uri": "file:///t.maca" } } }),
                &mut out,
            )
            .unwrap();
        let names: Vec<String> = resp["result"]
            .as_array()
            .unwrap()
            .iter()
            .map(|s| s["name"].as_str().unwrap().to_string())
            .collect();
        assert!(
            names.contains(&"helper".into()) && names.contains(&"main".into()),
            "{names:?}"
        );
        assert!(names.contains(&"Point".into()), "type missing: {names:?}");
    }

    #[test]
    fn definition_jumps_to_the_defining_line() {
        let mut s = Server {
            docs: HashMap::new(),
        };
        let mut out = Vec::new();
        // `helper` is called on line 1 and defined on line 4.
        let src = "main() -> int {\n    helper()\n    0\n}\nhelper() -> int => 1\n";
        open(&mut s, &mut out, "file:///t.maca", src);
        let resp = s
            .handle(
                &json!({ "id": 10, "method": "textDocument/definition",
                    "params": { "textDocument": { "uri": "file:///t.maca" }, "position": { "line": 1, "character": 5 } } }),
                &mut out,
            )
            .unwrap();
        assert_eq!(
            resp["result"]["range"]["start"]["line"], 4,
            "def not on line 4: {resp}"
        );
    }

    #[test]
    fn formatting_returns_a_full_document_edit() {
        let mut s = Server {
            docs: HashMap::new(),
        };
        let mut out = Vec::new();
        // messy spacing the canonical printer will normalize
        let src = "main( )   ->int=>0\n";
        open(&mut s, &mut out, "file:///t.maca", src);
        let resp = s
            .handle(
                &json!({ "id": 11, "method": "textDocument/formatting",
                    "params": { "textDocument": { "uri": "file:///t.maca" } } }),
                &mut out,
            )
            .unwrap();
        let edits = resp["result"].as_array().unwrap();
        assert!(!edits.is_empty(), "no formatting edit produced");
        assert!(
            edits[0]["newText"].as_str().unwrap().contains("main()"),
            "not normalized: {resp}"
        );
    }

    #[test]
    fn config_completion_after_import() {
        let mut s = Server {
            docs: HashMap::new(),
        };
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
