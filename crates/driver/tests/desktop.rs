//! Capstone 12b: Tauri desktop app. Maca UI (→ JS) + Maca native backend +
//! `bridge.js` (Tauri glue). Verifies the UI → backend → view round-trip
//! headlessly: a DOM stub, the real compiled backend binary as `invoke`.

use std::path::{Path, PathBuf};
use std::process::Command;

fn wsl_ready() -> bool {
    Command::new("wsl").arg("true").status().map(|s| s.success()).unwrap_or(false)
}
fn to_wsl(p: &Path) -> String {
    let s = p.to_string_lossy().replace('\\', "/");
    let b = s.as_bytes();
    if b.len() >= 2 && b[1] == b':' {
        format!("/mnt/{}{}", (b[0] as char).to_ascii_lowercase(), &s[2..])
    } else {
        s
    }
}
fn app_path(name: &str) -> String {
    format!("{}/../../apps/desktop/{name}", env!("CARGO_MANIFEST_DIR"))
}

struct BuildLock(PathBuf);
impl BuildLock {
    fn acquire() -> Self {
        let p = std::env::temp_dir().join("maca-it-build.lock");
        for _ in 0..1200 {
            if let Ok(m) = std::fs::metadata(&p) {
                if m.modified().ok().and_then(|t| t.elapsed().ok()).map(|e| e.as_secs() > 300).unwrap_or(false) {
                    let _ = std::fs::remove_file(&p);
                }
            }
            if std::fs::OpenOptions::new().write(true).create_new(true).open(&p).is_ok() {
                return BuildLock(p);
            }
            std::thread::sleep(std::time::Duration::from_millis(300));
        }
        BuildLock(p)
    }
}
impl Drop for BuildLock {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

const HARNESS: &str = r#"
function makeNode(tag){return{tagName:String(tag).toUpperCase(),className:"",_a:{},children:[],_l:{},value:"",textContent:"",id:"",
 setAttribute(k,v){this._a[k]=v; if(k==="id")this.id=v},getAttribute(k){return this._a[k]},
 addEventListener(e,f){(this._l[e]=this._l[e]||[]).push(f)},
 appendChild(c){this.children.push(c);c.parentNode=this;return c},
 dispatchEvent(ev){(this._l[ev.type]||[]).forEach(f=>f(ev))}};}
function makeText(t){return{nodeType:3,textContent:String(t),children:[]};}
const app=makeNode("div"); app.id="app";
function findId(n,id){ if(n.id===id) return n; for(const c of (n.children||[])){ const r=findId(c,id); if(r) return r; } return null; }
global.document={createElement:makeNode,createTextNode:makeText,getElementById:id=> id==="app"?app:findId(app,id)};
const cp=require("child_process");
globalThis.invoke=(cmd,arg)=> cp.execFileSync(process.env.BACKEND,[String(arg)]).toString().trim();
require("./app.js");
require("./bridge.js");
(async()=>{
  document.getElementById("name").value="Alfo";
  const go=document.getElementById("go");
  await go._l.click[0]();
  console.log(JSON.stringify({result: document.getElementById("result").textContent, tag: app.children[0].tagName}));
})();
"#;

#[test]
fn desktop_ui_backend_roundtrip() {
    if !wsl_ready() {
        eprintln!("skipping desktop capstone: wsl not available");
        return;
    }
    let _lk = BuildLock::acquire();
    let maca = env!("CARGO_BIN_EXE_maca");

    // Maca UI → web/, Maca backend → native binary
    let web = std::env::temp_dir().join("maca-desktop-web");
    let r = Command::new(maca)
        .args(["build", "--target", "js", &app_path("app.maca"), "-o", &web.to_string_lossy()])
        .output()
        .expect("build ui");
    assert!(r.status.success(), "ui build: {}", String::from_utf8_lossy(&r.stderr));

    let backend = std::env::temp_dir().join("maca-desktop-backend");
    let r = Command::new(maca)
        .args(["build", &app_path("backend.maca"), "-o", &backend.to_string_lossy()])
        .output()
        .expect("build backend");
    assert!(r.status.success(), "backend build: {}", String::from_utf8_lossy(&r.stderr));

    // bring the Tauri glue + harness into the web dir
    std::fs::copy(app_path("bridge.js"), web.join("bridge.js")).unwrap();
    std::fs::write(web.join("harness.js"), HARNESS).unwrap();

    let wsl_web = to_wsl(&web);
    let wsl_backend = to_wsl(&backend);
    let cmd = format!(
        "cd {wsl_web} && BACKEND={wsl_backend} nix shell nixpkgs#nodejs -c node harness.js"
    );
    let out = Command::new("wsl").args(["-e", "sh", "-c", &cmd]).output().expect("node");
    let s = String::from_utf8_lossy(&out.stdout);
    let line = s.lines().find(|l| l.contains("\"result\"")).unwrap_or("");
    assert!(line.contains("\"tag\":\"DIV\""), "UI should render a div: {s}\nerr {}", String::from_utf8_lossy(&out.stderr));
    assert!(
        line.contains("Hello, Alfo!"),
        "UI action should round-trip to the Maca backend and update the view: {s}\nerr {}",
        String::from_utf8_lossy(&out.stderr)
    );
}
