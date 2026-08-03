//! maca-lsp: a Language Server for `.maca`, speaking LSP over stdio with
//! `Content-Length` framing. Wraps the pure analysis functions in `lib.rs`:
//! live diagnostics (parse + type/effect), hover (signatures/types), and
//! completion (config option namespaces or top-level names), go-to-definition,
//! document symbols, find-references, rename, signature help, and formatting.
//!
//! Editors launch this binary and talk JSON-RPC to it. See `editor/zed-maca`.

use serde_json::{Value, json};
use std::collections::HashMap;
use std::io::{self, BufRead, Write};
use std::path::{Path, PathBuf};

fn main() {
    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut server = Server::default();
    server.run(&mut stdin.lock(), &mut stdout.lock());
}

#[derive(Default)]
struct Server {
    docs: HashMap<String, String>, // uri → text
    /// The folder the editor opened, from `initialize`. Renaming a top-level
    /// name has to reach the modules that import it, and this is where they
    /// are looked for.
    root: Option<PathBuf>,
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
            "initialize" => {
                self.root = req
                    .pointer("/params/workspaceFolders/0/uri")
                    .or_else(|| req.pointer("/params/rootUri"))
                    .and_then(Value::as_str)
                    .and_then(path_of);
                Some(reply(
                    id,
                    json!({
                        "capabilities": {
                            "textDocumentSync": 1, // Full
                            "hoverProvider": true,
                            "completionProvider": { "triggerCharacters": ["."] },
                            "documentSymbolProvider": true,
                            "definitionProvider": true,
                            "referencesProvider": true,
                            "documentHighlightProvider": true,
                            // `prepareSupport` lets the editor ask what the cursor
                            // is on before it opens the rename box, so the box is
                            // pre-filled with the name and refuses to open at all
                            // on a keyword or a comment.
                            "renameProvider": { "prepareProvider": true },
                            "signatureHelpProvider": { "triggerCharacters": ["(", ","] },
                            "documentFormattingProvider": true
                        },
                        "serverInfo": { "name": "maca-lsp", "version": env!("CARGO_PKG_VERSION") }
                    }),
                ))
            }
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
                let (Some(text), Some(off)) = self.where_at(req) else {
                    return Some(reply(id, Value::Null));
                };
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
                let (Some(text), Some(off)) = self.where_at(req) else {
                    return Some(reply(id, Value::Null));
                };
                let uri = req.pointer("/params/textDocument/uri")?.as_str()?;
                match maca_lsp::definition(&text, off) {
                    Some((s, e)) => Some(reply(
                        id,
                        json!({ "uri": uri, "range": range(&text, s, e) }),
                    )),
                    None => Some(reply(id, Value::Null)),
                }
            }
            "textDocument/signatureHelp" => {
                let (Some(text), Some(off)) = self.where_at(req) else {
                    return Some(reply(id, Value::Null));
                };
                match maca_lsp::signature_help(&text, off) {
                    Some((sig, labels, active)) => Some(reply(
                        id,
                        json!({
                            "signatures": [{
                                "label": sig,
                                "parameters": labels
                                    .into_iter()
                                    .map(|l| json!({ "label": l }))
                                    .collect::<Vec<_>>()
                            }],
                            "activeSignature": 0,
                            "activeParameter": active
                        }),
                    )),
                    None => Some(reply(id, Value::Null)),
                }
            }
            "textDocument/references" => {
                let (Some(text), Some(off)) = self.where_at(req) else {
                    return Some(reply(id, json!([])));
                };
                let uri = req.pointer("/params/textDocument/uri")?.as_str()?;
                let locs: Vec<Value> = maca_lsp::references(&text, off)
                    .into_iter()
                    .map(|(s, e)| json!({ "uri": uri, "range": range(&text, s, e) }))
                    .collect();
                Some(reply(id, json!(locs)))
            }
            "textDocument/documentHighlight" => {
                let (Some(text), Some(off)) = self.where_at(req) else {
                    return Some(reply(id, json!([])));
                };
                // kind 1 = Text. The protocol also has Read and Write, which
                // would need to know which occurrence is the binding site;
                // `binding` knows the scope but not yet the direction.
                let spans: Vec<Value> = maca_lsp::references(&text, off)
                    .into_iter()
                    .map(|(s, e)| json!({ "range": range(&text, s, e), "kind": 1 }))
                    .collect();
                Some(reply(id, json!(spans)))
            }
            "textDocument/prepareRename" => {
                let (Some(text), Some(off)) = self.where_at(req) else {
                    return Some(reply(id, Value::Null));
                };
                match maca_lsp::binding::resolve(&text, off) {
                    Some(b) => Some(reply(
                        id,
                        json!({
                            "range": range(&text, b.at.0, b.at.1),
                            "placeholder": b.name,
                        }),
                    )),
                    // Null is the protocol's "nothing renameable here", which
                    // is what stops the editor opening a rename box over a
                    // keyword or a comment.
                    None => Some(reply(id, Value::Null)),
                }
            }
            "textDocument/rename" => {
                let (Some(text), Some(off)) = self.where_at(req) else {
                    return Some(reply(id, Value::Null));
                };
                let uri = req.pointer("/params/textDocument/uri")?.as_str()?;
                let new_name = req
                    .pointer("/params/newName")
                    .and_then(Value::as_str)
                    .unwrap_or("");
                let Some(binding) = maca_lsp::binding::resolve(&text, off) else {
                    return Some(reply(id, Value::Null));
                };
                // The editor hands over whatever was typed. `1x` or `if` turns
                // a working file into one that doesn't parse, and a rename that
                // reports success is the wrong way to find that out.
                if !maca_lsp::binding::is_renameable_to(new_name) {
                    return Some(self.refuse(id, out, &format!("`{new_name}` is not a name")));
                }
                // A field is renamed in one file, so a field whose record lives
                // in another module can only be renamed half-way: the literal
                // here, not the declaration there. Half is worse than none, it
                // reports success and breaks the build.
                if binding.scope == maca_lsp::Scope::Field
                    && !maca_lsp::binding::declares_field(&text, &binding.name)
                {
                    return Some(self.refuse(
                        id,
                        out,
                        &format!(
                            "`{}` is declared in another module; \
                             rename it where the record is",
                            binding.name
                        ),
                    ));
                }

                // A top-level name is visible to every module that imports it,
                // so the edit spans files: renaming only the open one leaves
                // those callers naming something that no longer exists, and
                // the editor reports success while the build breaks.
                let mut changes = serde_json::Map::new();
                match (self.root.as_deref(), path_of(uri)) {
                    (Some(root), Some(file)) => {
                        for (path, spans) in
                            maca_lsp::workspace::rename_edits(root, &file, &text, &binding)
                        {
                            // The open document's own URI is whatever the
                            // editor sent; re-deriving it from the path would
                            // not necessarily match, and an unmatched URI is a
                            // dropped buffer edit.
                            let (key, src) = if path == file {
                                (uri.to_string(), text.clone())
                            } else {
                                (
                                    uri_of(&path),
                                    std::fs::read_to_string(&path).unwrap_or_default(),
                                )
                            };
                            changes.insert(key, edits_in(&src, &spans, new_name));
                        }
                    }
                    // No workspace: the open document is all there is.
                    _ => {
                        let spans = maca_lsp::binding::spans(&text, &binding);
                        changes.insert(uri.to_string(), edits_in(&text, &spans, new_name));
                    }
                }
                if changes.is_empty() {
                    return Some(reply(id, Value::Null));
                }
                Some(reply(id, json!({ "changes": changes })))
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

    /// The document and cursor offset a positional request names.
    ///
    /// Returned as a pair rather than propagated with `?`, because the caller
    /// must still answer: `handle` returning `None` writes no reply at all, and
    /// a request with an `id` and no response hangs the client until it gives
    /// up. A hover over a document the server never saw `didOpen` for wedged
    /// the editor for exactly that reason.
    fn where_at(&self, req: &Value) -> (Option<String>, Option<usize>) {
        let Some(text) = self.doc_at(req) else {
            return (None, None);
        };
        let off = self.offset_at(req, &text);
        (Some(text), off)
    }

    /// Answer a rename with "nothing happened", and say why in the editor's
    /// status area, because silence would read as success.
    fn refuse(&self, id: Option<Value>, out: &mut impl Write, why: &str) -> Value {
        write_message(
            out,
            &json!({
                "jsonrpc": "2.0",
                "method": "window/showMessage",
                // 2 = Warning
                "params": { "type": 2, "message": why }
            }),
        );
        reply(id, Value::Null)
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

/// One `TextEdit` per span, all replacing with the same new name.
fn edits_in(src: &str, spans: &[(usize, usize)], new_name: &str) -> Value {
    json!(
        spans
            .iter()
            .map(|(s, e)| json!({ "range": range(src, *s, *e), "newText": new_name }))
            .collect::<Vec<_>>()
    )
}

/// `file:///a/b` → `/a/b`. Percent-escapes are decoded because a path with a
/// space arrives as `%20` and would otherwise name a file that isn't there.
///
/// `file:///c%3A/proj/x.maca` → `c:/proj/x.maca`: a Windows drive letter comes
/// through as an absolute path whose first segment is the drive, and the
/// leading slash has to go or nothing on that host resolves.
fn path_of(uri: &str) -> Option<PathBuf> {
    let rest = uri.strip_prefix("file://")?;
    let rest = rest.strip_prefix("localhost").unwrap_or(rest);
    let decoded = percent_decode(rest);
    let bytes = decoded.as_bytes();
    if bytes.len() >= 3 && bytes[0] == b'/' && bytes[1].is_ascii_alphabetic() && bytes[2] == b':' {
        return Some(PathBuf::from(&decoded[1..]));
    }
    Some(PathBuf::from(decoded))
}

/// The inverse. Everything outside the URI unreserved set is escaped, so a
/// project directory with a space produces a key the editor can match: without
/// this the round-trip was asymmetric (`path_of` decoded and `uri_of` did not
/// encode) and an edit keyed by a raw-space URI was silently dropped.
fn uri_of(path: &Path) -> String {
    let s = path.to_string_lossy().replace('\\', "/");
    let s = if s.as_bytes().first().is_some_and(u8::is_ascii_alphabetic)
        && s.as_bytes().get(1) == Some(&b':')
    {
        format!("/{s}")
    } else {
        s
    };
    let mut out = String::from("file://");
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' | b'/' => {
                out.push(b as char);
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

/// Decode `%XX` escapes back into bytes, then into text.
///
/// Bytes, not `char`s: a Korean directory arrives as three escapes per
/// character, and pushing each byte as a `char` re-encoded it as three
/// characters of mojibake: a path that named nothing, so the workspace walk
/// found no files and the rename silently shrank to the open buffer.
fn percent_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(s.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%'
            && i + 2 < b.len()
            && let Ok(byte) = u8::from_str_radix(&s[i + 1..i + 3], 16)
        {
            out.push(byte);
            i += 3;
        } else {
            out.push(b[i]);
            i += 1;
        }
    }
    String::from_utf8_lossy(&out).into_owned()
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
        let mut s = Server::default();
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
        let mut s = Server::default();
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
        let mut s = Server::default();
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
        let mut s = Server::default();
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
        let mut s = Server::default();
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
        let mut s = Server::default();
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
        let mut s = Server::default();
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
        let mut s = Server::default();
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
