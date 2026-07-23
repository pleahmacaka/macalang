# Maca desktop capstone (Tauri)

Front end in Maca UI (`app.maca` → JS/HTML/CSS), back end in Maca native
(`backend.maca` → binary). `bridge.js` is the Tauri glue that wires a UI action
to the native command.

## Round-trip

`app.maca` renders a name field, a **Greet** button, and a result area.
Clicking **Greet** calls `invoke("greet", name)`, which runs the Maca native
`backend` binary; its output (`Hello, <name>!`) is written back into the view.
This is verified headlessly in `crates/driver/tests/desktop.rs` (a DOM stub +
the real backend binary).

## Build

```
maca build --target js app.maca -o ../web     # UI → web/{index.html,app.js,app.css}
maca build backend.maca -o backend            # native command binary
```

## Package a window (cross-platform)

`tauri.conf.json` points Tauri at `../web`. A packaged build wraps the web
front end in a native webview window and exposes `greet` as a Tauri command
that shells out to the `backend` binary:

```
cargo tauri build     # needs the Tauri CLI + system webview (WebView2 / WKWebView / webkit2gtk)
```

Tauri provides the cross-platform window + webview; Maca provides the UI and the
command. In real Tauri, `invoke` is `window.__TAURI__.invoke`; the headless test
supplies an equivalent stub, so the same `bridge.js` runs unchanged.
