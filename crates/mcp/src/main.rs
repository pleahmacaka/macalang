//! Maca MCP server — newline-delimited JSON-RPC 2.0 over stdio. Exposes the
//! LLM-native tools (`maca.check`, `maca.fmt`, `maca.stdlib`, `maca.options`,
//! `maca.spec`) so an agent can run the generate → verify → fix loop.

use serde_json::{json, Value};
use std::io::{BufRead, Write};

fn main() {
    let stdin = std::io::stdin();
    let mut stdout = std::io::stdout();
    for line in stdin.lock().lines() {
        let Ok(line) = line else { break };
        if line.trim().is_empty() {
            continue;
        }
        let Ok(req) = serde_json::from_str::<Value>(&line) else { continue };
        let has_id = req.get("id").is_some();
        let id = req.get("id").cloned().unwrap_or(Value::Null);
        let method = req.get("method").and_then(|m| m.as_str()).unwrap_or("");

        let resp = match method {
            "initialize" => json!({
                "jsonrpc": "2.0", "id": id,
                "result": {
                    "protocolVersion": "2024-11-05",
                    "capabilities": { "tools": {} },
                    "serverInfo": { "name": "maca", "version": "0.1.0" }
                }
            }),
            "tools/list" => json!({ "jsonrpc": "2.0", "id": id, "result": { "tools": tool_defs() } }),
            "tools/call" => {
                let params = req.get("params").cloned().unwrap_or(Value::Null);
                let name = params.get("name").and_then(|n| n.as_str()).unwrap_or("");
                let args = params.get("arguments").cloned().unwrap_or(json!({}));
                let text = call_tool(name, &args);
                json!({
                    "jsonrpc": "2.0", "id": id,
                    "result": { "content": [ { "type": "text", "text": text } ] }
                })
            }
            _ if !has_id => continue, // a notification (e.g. notifications/initialized)
            _ => json!({
                "jsonrpc": "2.0", "id": id,
                "error": { "code": -32601, "message": "method not found" }
            }),
        };

        if has_id {
            let _ = writeln!(stdout, "{resp}");
            let _ = stdout.flush();
        }
    }
}

fn arg<'a>(a: &'a Value, key: &str) -> &'a str {
    a.get(key).and_then(|v| v.as_str()).unwrap_or("")
}

fn call_tool(name: &str, a: &Value) -> String {
    match name {
        "maca.check" => {
            let config = a.get("config").and_then(|v| v.as_bool()).unwrap_or(false);
            let d = maca_mcp::check(arg(a, "code"), config);
            if d.is_empty() {
                "ok: no diagnostics".into()
            } else {
                d.join("\n")
            }
        }
        "maca.fmt" => match maca_mcp::fmt(arg(a, "code")) {
            Ok(s) => s,
            Err(e) => format!("parse errors:\n{}", e.join("\n")),
        },
        "maca.stdlib" => maca_mcp::stdlib(arg(a, "query")).join("\n"),
        "maca.options" => maca_mcp::options(arg(a, "prefix")).join("\n"),
        "maca.spec" => maca_mcp::spec(arg(a, "section")),
        other => format!("unknown tool: {other}"),
    }
}

fn tool_defs() -> Value {
    let str_prop = |desc: &str| json!({ "type": "string", "description": desc });
    json!([
        { "name": "maca.check", "description": "Type/effect-check Maca code; returns diagnostics.",
          "inputSchema": { "type": "object", "properties": { "code": str_prop("Maca source"), "config": { "type": "boolean" } }, "required": ["code"] } },
        { "name": "maca.fmt", "description": "Format Maca code canonically.",
          "inputSchema": { "type": "object", "properties": { "code": str_prop("Maca source") }, "required": ["code"] } },
        { "name": "maca.stdlib", "description": "Search stdlib/prelude signatures.",
          "inputSchema": { "type": "object", "properties": { "query": str_prop("substring") } } },
        { "name": "maca.options", "description": "Known NixOS option namespaces by prefix.",
          "inputSchema": { "type": "object", "properties": { "prefix": str_prop("prefix") } } },
        { "name": "maca.spec", "description": "Spec reference (syntax|effects|modes|types).",
          "inputSchema": { "type": "object", "properties": { "section": str_prop("section") } } }
    ])
}
