//! Real end-to-end backend test: drive the `maca` binary to compile and run an
//! example, then assert on its output. Requires WSL + zig; skips otherwise so
//! `cargo test` stays green on hosts without the native toolchain.

use std::path::{Path, PathBuf};
use std::process::Command;

fn wsl_ready() -> bool {
    Command::new("wsl")
        .arg("true")
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// Cross-process build lock: nix/zig can't be hammered by ~a dozen concurrent
/// invocations (lock contention), so integration tests serialize the compile
/// step. Held for the test body; released on drop, stale locks (>5min) stolen.
struct BuildLock(PathBuf);
impl BuildLock {
    fn acquire() -> Self {
        let p = std::env::temp_dir().join("maca-it-build.lock");
        for _ in 0..1200 {
            if let Ok(m) = std::fs::metadata(&p) {
                let stale = m
                    .modified()
                    .ok()
                    .and_then(|t| t.elapsed().ok())
                    .map(|e| e.as_secs() > 300)
                    .unwrap_or(false);
                if stale {
                    let _ = std::fs::remove_file(&p);
                }
            }
            if std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&p)
                .is_ok()
            {
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

fn to_wsl(p: &Path) -> String {
    let s = p.to_string_lossy().replace('\\', "/");
    let b = s.as_bytes();
    if b.len() >= 2 && b[1] == b':' {
        format!("/mnt/{}{}", (b[0] as char).to_ascii_lowercase(), &s[2..])
    } else {
        s
    }
}

fn example(name: &str) -> String {
    format!("{}/../../examples/{name}", env!("CARGO_MANIFEST_DIR"))
}

#[test]
fn hello_builds_and_runs() {
    if !wsl_ready() {
        eprintln!("skipping hello_builds_and_runs: wsl not available");
        return;
    }
    let _lk = BuildLock::acquire();
    let out = Command::new(env!("CARGO_BIN_EXE_maca"))
        .args(["run", &example("hello.maca")])
        .output()
        .expect("spawn maca");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("Hello, World"),
        "expected greeting.\nstdout: {stdout}\nstderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

fn taskr(args: &[&str]) -> String {
    let _lk = BuildLock::acquire();
    let mut a = vec!["run".to_string(), example("taskr.maca")];
    a.extend(args.iter().map(|s| s.to_string()));
    let out = Command::new(env!("CARGO_BIN_EXE_maca"))
        .args(&a)
        .output()
        .expect("spawn maca");
    assert!(
        out.status.success(),
        "taskr {args:?} failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout).into_owned()
}

#[test]
fn taskr_add_list_roundtrip() {
    if !wsl_ready() {
        eprintln!("skipping taskr_add_list_roundtrip: wsl not available");
        return;
    }
    // isolated, deterministic store
    Command::new("wsl")
        .args([
            "sh",
            "-c",
            "rm -f \"${XDG_DATA_HOME:-$HOME/.local/share}/store.json\"",
        ])
        .status()
        .ok();

    // empty state → usage
    assert!(taskr(&[]).contains("usage"), "no-args should print usage");

    // add one, it round-trips through JSON on the next list
    assert!(taskr(&["add", "buy milk"]).contains("added: buy milk"));
    let l1 = taskr(&["list"]);
    assert!(l1.contains("#1") && l1.contains("buy milk"), "list1: {l1}");
    assert!(l1.contains("[ ]"), "unchecked box expected: {l1}");

    // add a second, ids increment and both persist
    taskr(&["add", "walk dog"]);
    let l2 = taskr(&["list"]);
    assert!(l2.contains("#1") && l2.contains("buy milk"), "list2: {l2}");
    assert!(l2.contains("#2") && l2.contains("walk dog"), "list2: {l2}");
    // exactly two tasks
    assert_eq!(
        l2.lines().filter(|x| x.contains('#')).count(),
        2,
        "expected 2 tasks: {l2}"
    );
}

#[test]
fn parallel_runs() {
    if !wsl_ready() {
        eprintln!("skipping parallel_runs: wsl not available");
        return;
    }
    let out = Command::new(env!("CARGO_BIN_EXE_maca"))
        .args(["run", &example("parallel.maca")])
        .output()
        .expect("spawn maca");
    let s = String::from_utf8_lossy(&out.stdout);
    let got: Vec<&str> = s.lines().collect();
    assert_eq!(
        got,
        vec!["1", "4", "9", "16"],
        "stdout: {s}\nstderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

/// Compile an example WITHOUT stripping (keeps symbols for nm/objdump). Mirrors
/// the driver's pipeline including async and SIMD (LLVM IR) linkage.
fn build_unstripped(name: &str) -> PathBuf {
    let _lk = BuildLock::acquire();
    let src = std::fs::read_to_string(example(name)).unwrap();
    let parsed = maca_parser::parse(&src);
    assert!(parsed.errors.is_empty(), "{name}: {:?}", parsed.errors);
    let c = maca_backend_c::emit(&parsed.module);
    let use_async = maca_backend_c::needs_async(&c);
    let llvm = maca_backend_llvm::emit(&parsed.module);
    let use_simd = !llvm.simd_fns.is_empty();
    let dir = std::env::temp_dir().join(format!("maca-nm-{}", name.replace('.', "_")));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("main.c"), &c).unwrap();
    maca_runtime::write_to(&dir).unwrap();
    if use_async {
        maca_runtime::write_async(&dir).unwrap();
    }
    if use_simd {
        std::fs::write(dir.join("simd.ll"), &llvm.ir).unwrap();
    }
    let out: PathBuf = dir.join("prog");
    let mut args: Vec<String> = ["nix", "shell", "nixpkgs#zig", "-c", "zig", "cc"]
        .iter()
        .map(|s| s.to_string())
        .collect();
    args.push(to_wsl(&dir.join("main.c")));
    args.push(to_wsl(&dir.join("maca_runtime.c")));
    if use_async {
        args.push(to_wsl(&dir.join("maca_async.c")));
        args.push("-pthread".into());
    }
    if use_simd {
        args.push(to_wsl(&dir.join("simd.ll")));
        args.push("-mavx2".into());
    }
    args.push("-I".into());
    args.push(to_wsl(&dir));
    args.push("-o".into());
    args.push(to_wsl(&out));
    for f in ["-O2", "-static", "-target", "x86_64-linux-musl"] {
        args.push(f.into());
    }
    let r = Command::new("wsl").args(&args).output().unwrap();
    assert!(
        r.status.success(),
        "compile {name}: {}",
        String::from_utf8_lossy(&r.stderr)
    );
    out
}

fn nm_symbols(name: &str) -> String {
    let bin = build_unstripped(name);
    let nm = Command::new("wsl")
        .args([
            "sh",
            "-c",
            &format!("nix shell nixpkgs#binutils -c nm {}", to_wsl(&bin)),
        ])
        .output()
        .unwrap();
    String::from_utf8_lossy(&nm.stdout).into_owned()
}

fn objdump(name: &str) -> String {
    let bin = build_unstripped(name);
    let out = Command::new("wsl")
        .args([
            "sh",
            "-c",
            &format!("nix shell nixpkgs#binutils -c objdump -d {}", to_wsl(&bin)),
        ])
        .output()
        .unwrap();
    String::from_utf8_lossy(&out.stdout).into_owned()
}

#[test]
fn sequential_binary_has_no_scheduler() {
    if !wsl_ready() {
        eprintln!("skipping sequential_binary_has_no_scheduler: wsl not available");
        return;
    }
    let hello = nm_symbols("hello.maca");
    assert!(
        !hello.contains("maca_parallel_i64") && !hello.contains("pthread_create"),
        "sequential binary should carry no scheduler symbols"
    );
    let par = nm_symbols("parallel.maca");
    assert!(
        par.contains("maca_parallel_i64"),
        "parallel binary should link the scheduler"
    );
}

#[test]
fn simd_hybrid_correct() {
    if !wsl_ready() {
        eprintln!("skipping simd_hybrid_correct: wsl not available");
        return;
    }
    let _lk = BuildLock::acquire();
    // dot8(splat 2, splat 3) = 8 lanes * (2*3) = 48; C main calls the LLVM kernel
    let out = Command::new(env!("CARGO_BIN_EXE_maca"))
        .args(["run", &example("simd.maca")])
        .output()
        .expect("spawn maca");
    let s = String::from_utf8_lossy(&out.stdout);
    assert_eq!(
        s.trim(),
        "48",
        "stdout: {s}\nstderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn simd_uses_llvm_vector_instructions() {
    if !wsl_ready() {
        eprintln!("skipping simd_uses_llvm_vector_instructions: wsl not available");
        return;
    }
    // The LLVM path must produce real 256-bit AVX vector ops (8-wide → faster
    // than scalar). objdump the dot8 kernel and require a packed-vector mul.
    let dis = objdump(&"simd.maca");
    assert!(
        dis.contains("vmulps") || dis.contains("vfmadd") || dis.contains("%ymm"),
        "expected AVX vector instructions from the LLVM SIMD path"
    );
    // and the C path alone (no SIMD) must not
    let hello = objdump(&"hello.maca");
    assert!(
        !hello.contains("%ymm"),
        "scalar binary should have no 256-bit vector ops"
    );
}

#[test]
fn nix_config_accepted() {
    if !wsl_ready() {
        eprintln!("skipping nix_config_accepted: wsl not available");
        return;
    }
    let _lk = BuildLock::acquire();
    let out = std::env::temp_dir().join("maca-system.nix");
    let r = Command::new(env!("CARGO_BIN_EXE_maca"))
        .args([
            "build",
            "--target",
            "nix",
            &example("system.maca"),
            "-o",
            &out.to_string_lossy(),
        ])
        .output()
        .expect("spawn maca");
    assert!(
        r.status.success(),
        "build --target nix failed: {}",
        String::from_utf8_lossy(&r.stderr)
    );

    let nix = std::fs::read_to_string(&out).unwrap();
    assert!(nix.contains("enable = true"), "missing injected enable");
    assert!(
        nix.contains("fonts.packages"),
        "missing hoisted fonts.packages"
    );
    assert!(nix.contains("xdg.userDirs"), "missing xdg.userDirs");

    // nix-instantiate must accept the generated module
    let ni = Command::new("wsl")
        .args([
            "sh",
            "-c",
            &format!("nix-instantiate --parse {}", to_wsl(&out)),
        ])
        .output()
        .expect("nix-instantiate");
    assert!(
        ni.status.success(),
        "nix-instantiate rejected:\n{}",
        String::from_utf8_lossy(&ni.stderr)
    );
}

const JS_HARNESS: &str = r#"
function makeNode(tag){return{tagName:String(tag).toUpperCase(),className:"",_a:{},children:[],_l:{},value:"",
 setAttribute(k,v){this._a[k]=v},getAttribute(k){return this._a[k]},
 addEventListener(e,f){(this._l[e]=this._l[e]||[]).push(f)},
 appendChild(c){this.children.push(c);c.parentNode=this;return c},
 dispatchEvent(ev){(this._l[ev.type]||[]).forEach(f=>f(ev))}};}
const app=makeNode("div");
global.document={createElement:makeNode,createTextNode:t=>({nodeType:3,text:t}),getElementById:id=>id==="app"?app:null};
const mod=require("./app.js");
const root=app.children[0];
const nameI=root.children[0], ageI=root.children[1];
nameI.value="Alfo"; nameI.dispatchEvent({type:"input",target:nameI});
ageI.value="42"; ageI.dispatchEvent({type:"input",target:ageI});
console.log(JSON.stringify({tag:root.tagName,cls:root.className,kids:root.children.length,name:mod.state.name,age:mod.state.age}));
"#;

#[test]
fn build_auto_detects_ui_target() {
    // no --target on a view (`-> Element`) auto-selects js instead of failing
    // on the native path with confusing linker errors.
    let dir = std::env::temp_dir().join("maca-detect-ui");
    let _ = std::fs::remove_dir_all(&dir);
    let r = Command::new(env!("CARGO_BIN_EXE_maca"))
        .args([
            "build",
            &example("counter.maca"),
            "-o",
            &dir.to_string_lossy(),
        ])
        .output()
        .expect("spawn maca");
    let err = String::from_utf8_lossy(&r.stderr);
    assert!(
        err.contains("--target js"),
        "no js auto-detect note:\n{err}"
    );
    assert!(r.status.success(), "auto-detected js build failed:\n{err}");
}

#[test]
fn js_ui_renders_and_binds() {
    if !wsl_ready() {
        eprintln!("skipping js_ui_renders_and_binds: wsl not available");
        return;
    }
    let _lk = BuildLock::acquire();
    let dir = std::env::temp_dir().join("maca-counter-web");
    let r = Command::new(env!("CARGO_BIN_EXE_maca"))
        .args([
            "build",
            "--target",
            "js",
            &example("counter.maca"),
            "-o",
            &dir.to_string_lossy(),
        ])
        .output()
        .expect("spawn maca");
    assert!(
        r.status.success(),
        "build --target js failed: {}",
        String::from_utf8_lossy(&r.stderr)
    );

    // tree-shaken Tailwind: only used classes ship
    let css = std::fs::read_to_string(dir.join("app.css")).unwrap();
    for used in [".flex ", ".flex-col ", ".text-center "] {
        assert!(css.contains(used), "css missing {used}: {css}");
    }
    assert!(
        !css.contains(".grid ") && !css.contains(".hidden "),
        "css shipped unused classes: {css}"
    );

    // headless render + reactivity via Node with a DOM stub
    std::fs::write(dir.join("harness.js"), JS_HARNESS).unwrap();
    let wsl_dir = to_wsl(&dir);
    let out = Command::new("wsl")
        .args([
            "sh",
            "-c",
            &format!("cd {wsl_dir} && nix shell nixpkgs#nodejs -c node harness.js"),
        ])
        .output()
        .expect("node");
    let s = String::from_utf8_lossy(&out.stdout);
    let line = s.lines().find(|l| l.contains("\"tag\"")).unwrap_or("");
    assert!(line.contains("\"tag\":\"DIV\""), "expected a div root: {s}");
    assert!(line.contains("\"kids\":2"), "expected two inputs: {s}");
    assert!(
        line.contains("\"name\":\"Alfo\""),
        "bind:value should update name state: {s}"
    );
    assert!(
        line.contains("\"age\":42"),
        "bind:value setter should update age state: {s}"
    );
}

fn run_example(name: &str) -> String {
    let _lk = BuildLock::acquire();
    let out = Command::new(env!("CARGO_BIN_EXE_maca"))
        .args(["run", &example(name)])
        .output()
        .expect("spawn maca");
    assert!(
        out.status.success(),
        "{name} failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout).into_owned()
}

#[test]
fn ffi_c_sqlite_roundtrip() {
    if !wsl_ready() {
        eprintln!("skipping ffi_c_sqlite_roundtrip: wsl not available");
        return;
    }
    // import c "sqlite3.h": open :memory:, create, insert, then iterate the
    // full result set reading multiple columns per row.
    let out = run_example("ffi_sqlite.maca");
    assert!(out.contains("ada is 36"), "sqlite row 1: {out}");
    assert!(out.contains("alan is 41"), "sqlite row 2: {out}");
}

#[test]
fn ffi_nix_value() {
    if !wsl_ready() {
        eprintln!("skipping ffi_nix_value: wsl not available");
        return;
    }
    // import nix "./answer.nix" evaluates `21 * 2` at build time
    let out = run_example("ffi_nix.maca");
    assert_eq!(out.trim(), "42", "nix value: {out}");
}

#[test]
#[ignore = "feature-gated: embeds CPython (libpython, large binary). Run with `--ignored`."]
fn ffi_py_calls_python() {
    if !wsl_ready() {
        eprintln!("skipping ffi_py_calls_python: wsl not available");
        return;
    }
    // import py "platform": call platform.python_version() → a version string
    let out = run_example("ffi_py.maca");
    let v = out.trim();
    assert!(
        v.split('.').count() >= 2 && v.chars().next().is_some_and(|c| c.is_ascii_digit()),
        "expected a python version, got: {v}"
    );
}
