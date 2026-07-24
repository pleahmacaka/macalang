//! Capstone: the Tauri desktop app. `maca build --target tauri` scaffolds a
//! complete, buildable Tauri v2 project from a Maca UI + a Maca native backend.
//! Verified without WSL/Tauri: the scaffold structure, the compiled backend
//! binary's output, and (via a Node DOM + `__TAURI__` stub) the UI → bridge →
//! backend → view round-trip.

use std::path::Path;
use std::process::Command;

fn have(cmd: &str) -> bool {
    Command::new(cmd)
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}
fn wsl() -> bool {
    Command::new("wsl")
        .arg("true")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}
fn app_path(name: &str) -> String {
    format!("{}/../../apps/desktop/{name}", env!("CARGO_MANIFEST_DIR"))
}

#[test]
fn tauri_scaffold_is_complete_and_runs() {
    // build_tauri compiles the backend with the host cc, so needs cc and no wsl.
    if wsl() || !have("cc") {
        eprintln!("skipping tauri scaffold: needs a native cc and no wsl");
        return;
    }
    let out = std::env::temp_dir().join("maca-tauri-scaffold");
    let _ = std::fs::remove_dir_all(&out);
    let r = Command::new(env!("CARGO_BIN_EXE_maca"))
        .args([
            "build",
            "--target",
            "tauri",
            &app_path("app.maca"),
            "-o",
            &out.to_string_lossy(),
        ])
        .output()
        .expect("spawn maca");
    assert!(
        r.status.success(),
        "tauri build failed: {}",
        String::from_utf8_lossy(&r.stderr)
    );

    // every piece of a buildable Tauri v2 project is present
    for rel in [
        "dist/index.html",
        "dist/app.js",
        "dist/bridge.js",
        "src-tauri/Cargo.toml",
        "src-tauri/tauri.conf.json",
        "src-tauri/build.rs",
        "src-tauri/src/main.rs",
        "src-tauri/bin/backend",
    ] {
        assert!(out.join(rel).exists(), "missing generated file: {rel}");
    }

    // the config points at the UI; the shell registers the maca_run command
    let conf = std::fs::read_to_string(out.join("src-tauri/tauri.conf.json")).unwrap();
    assert!(
        conf.contains("\"frontendDist\": \"../dist\""),
        "conf: {conf}"
    );
    let main_rs = std::fs::read_to_string(out.join("src-tauri/src/main.rs")).unwrap();
    assert!(
        main_rs.contains("#[tauri::command]") && main_rs.contains("maca_run"),
        "main.rs: {main_rs}"
    );
    let index = std::fs::read_to_string(out.join("dist/index.html")).unwrap();
    assert!(
        index.contains("bridge.js"),
        "index.html doesn't load the bridge"
    );

    // the bundled backend binary runs and produces the greeting
    let b = Command::new(out.join("src-tauri/bin/backend"))
        .arg("Ada")
        .output()
        .expect("run backend");
    assert!(
        String::from_utf8_lossy(&b.stdout).contains("Hello, Ada!"),
        "backend output: {}",
        String::from_utf8_lossy(&b.stdout)
    );

    // headless round-trip: Node loads the UI + bridge with a __TAURI__ stub whose
    // invoke runs the same backend, clicks Greet, and checks the view updated.
    if have("node") {
        headless_roundtrip(&out);
    }
}

fn headless_roundtrip(out: &Path) {
    let backend = out.join("src-tauri/bin/backend");
    let harness = format!(
        r#"
function node(tag){{return{{tagName:String(tag).toUpperCase(),className:"",_a:{{}},children:[],_l:{{}},value:"",textContent:"",id:"",
 setAttribute(k,v){{this._a[k]=v;if(k==="id")this.id=v}},getAttribute(k){{return this._a[k]}},
 addEventListener(e,f){{(this._l[e]=this._l[e]||[]).push(f)}},
 appendChild(c){{this.children.push(c);c.parentNode=this;return c}}}};}}
function text(t){{return{{nodeType:3,textContent:String(t),children:[]}};}}
const app=node("div");app.id="app";
function find(n,id){{if(n.id===id)return n;for(const c of(n.children||[])){{const r=find(c,id);if(r)return r;}}return null;}}
global.document={{createElement:node,createTextNode:text,getElementById:id=>id==="app"?app:find(app,id)}};
const cp=require("child_process");
global.window=global;
globalThis.__TAURI__={{core:{{invoke:(cmd,a)=>cp.execFileSync({backend:?},[String(a.arg)]).toString().trim()}}}};
require({appjs:?});
require({bridge:?});
(async()=>{{
  document.getElementById("name").value="Alfo";
  await new Promise(r=>setTimeout(r,5));
  await document.getElementById("go")._l.click[0]();
  console.log(JSON.stringify({{result:document.getElementById("result").textContent}}));
}})();
"#,
        backend = backend.to_string_lossy(),
        appjs = out.join("dist/app.js").to_string_lossy(),
        bridge = out.join("dist/bridge.js").to_string_lossy(),
    );
    let hpath = out.join("harness.js");
    std::fs::write(&hpath, harness).unwrap();
    let o = Command::new("node").arg(&hpath).output().expect("node");
    let s = String::from_utf8_lossy(&o.stdout);
    assert!(
        s.contains("Hello, Alfo!"),
        "UI → bridge → backend round-trip failed: {s}\nerr {}",
        String::from_utf8_lossy(&o.stderr)
    );
}
