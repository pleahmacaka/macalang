# Maca desktop capstone (Tauri)

A native desktop app: the front end is a Maca UI (`app.maca` → JS/HTML/CSS), the
back end is a Maca native command (`backend.maca` → binary), and Tauri provides
the cross-platform window + webview. The whole app is Maca; Tauri is just the
shell, and `maca build --target tauri` generates it.

## One-shot build

```sh
maca build --target tauri app.maca -o dist-tauri
```

This scaffolds a complete, `cargo tauri build`-able project:

```
dist-tauri/
  dist/            the compiled UI (index.html, app.js, app.css) + bridge.js
  src-tauri/
    Cargo.toml     tauri v2 deps
    tauri.conf.json  points the webview at ../dist
    build.rs
    src/main.rs    the shell, registers the `maca_run` command
    bin/backend    the compiled backend.maca (run by maca_run)
```

## Round-trip

`app.maca` renders a name field, a **Greet** button, and a result area, and
carries its own glue in a JavaScript block: clicking **Greet** calls
`macaInvoke(name)`, which the generated `bridge.js` routes to the Tauri command
`maca_run`, which runs the bundled `backend` binary; its output
(`Hello, <name>!`) is written back into the view. Verified headlessly in
`crates/driver/tests/desktop.rs` (a DOM + `__TAURI__` stub + the real backend).

## Package a window

```sh
cd dist-tauri/src-tauri
cargo tauri build     # needs the Tauri CLI + a system webview (WebView2 / WKWebView / webkit2gtk)
```
