mod common;
use common::*;

use std::path::{Path, PathBuf};
use std::process::{Child, Command};
use std::time::{Duration, Instant};

/// A port nothing else in the workspace listens on.
const PORT: u16 = 34877;

fn fixture() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/programs/hmr")
}

/// Kill the dev server however the test ends.
struct Server(Child);

impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

#[test]
fn the_dev_mode_suites_pass() {
    if have_wsl() || !have("cc") {
        eprintln!("skipping dev-mode suites: needs a host cc and no wsl");
        return;
    }
    for suite in [
        "modules/signal/tests/live.maca",
        "modules/http/tests/dev.maca",
        "apps/tomo/tests/dev.maca",
    ] {
        let out = Command::new(maca())
            .current_dir(repo())
            .env("NO_COLOR", "1")
            .args(["test", suite])
            .output()
            .expect("spawn maca test");
        assert!(
            out.status.success(),
            "{suite}:\n{}\n{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr),
        );
    }
}

/// Everything that needs a running server, in one test.
#[test]
fn a_saved_chapter_reaches_the_browser_as_a_patch() {
    if have_wsl() || !have("cc") {
        eprintln!("skipping dev server: needs a host cc and no wsl");
        return;
    }

    let work = std::env::temp_dir().join("maca-hmr");
    let _ = std::fs::remove_dir_all(&work);
    let book = work.join("book-copy");
    copy_tree(&fixture(), &book);

    let bin = work.join("tomodev");
    {
        let _lk = BuildLock::acquire();
        let build = Command::new(maca())
            .current_dir(repo())
            .args(["build", "apps/tomo/dev.maca", "-o", &bin.to_string_lossy()])
            .output()
            .expect("spawn maca build");
        assert!(
            build.status.success(),
            "apps/tomo/dev.maca did not build:\n{}",
            String::from_utf8_lossy(&build.stderr)
        );
    }

    let site = work.join("run/site");
    let start = || {
        Server(
            Command::new(&bin)
                .current_dir(repo())
                .args([
                    PORT.to_string(),
                    work.join("run").to_string_lossy().into_owned(),
                    book.to_string_lossy().into_owned(),
                ])
                .spawn()
                .expect("spawn the dev server"),
        )
    };
    let server = start();

    let page = site.join("en/01-one.html");
    wait_for(&page, "the first build");

    let served = get("/en/01-one.html");
    assert!(
        served.contains("data-signal=\"article:en/01-one\""),
        "the served page has no article region:\n{served}"
    );
    assert!(
        served.contains("window.macaDevPath"),
        "the served page has no poller"
    );

    let before = std::fs::read_to_string(&page).expect("read the built page");
    edit(&book.join("book/en/01-one.md"), |md| {
        md.replace(
            "The paragraph the test edits.",
            "The paragraph the test edited.",
        )
    });

    let patch = poll(0);
    let keys = keys_of(&patch);
    assert_eq!(
        keys,
        vec!["article:en/01-one"],
        "a prose edit should patch the prose and nothing else: {patch}"
    );

    let after = std::fs::read_to_string(&page).expect("read the rebuilt page");
    assert_ne!(before, after, "the page file was not rewritten");
    let sent = value_of(&patch, "article:en/01-one");
    assert!(
        after.contains(&sent),
        "the op's value is not what the rebuilt page holds"
    );
    assert!(
        sent.contains("The paragraph the test edited."),
        "the edit did not reach the patch: {sent}"
    );

    if have("node") {
        applied_by_node(&served, &patch, "article:en/01-one", &sent);
        polled_by_node(&served);
    } else {
        eprintln!("skipping the node check: no node on this host");
    }

    let other = site.join("en/02-two.html");
    assert!(
        std::fs::read_to_string(&other)
            .unwrap()
            .contains("Chapter One"),
        "chapter two's sidebar should list chapter one by its heading"
    );

    edit(&book.join("book/en/01-one.md"), |md| {
        md.replace("# Chapter One", "# Chapter The First")
    });

    let patch = poll(1);
    let keys = keys_of(&patch);
    for want in [
        "article:en/01-one",
        "toc:en/01-one",
        "nav:en/01-one",
        "nav:en/02-two",
    ] {
        assert!(
            keys.contains(&want.to_string()),
            "a heading edit should patch {want}, got {keys:?}"
        );
    }
    assert!(
        value_of(&patch, "nav:en/02-two").contains("Chapter The First"),
        "chapter two's chapter list was not renamed"
    );

    for rel in [
        "index.html",
        "en/index.html",
        "en/home.html",
        "en/search-index.js",
        "en/01-one.html",
        "en/02-two.html",
        "ko/index.html",
        "ko/home.html",
        "ko/search-index.js",
        "ko/01-one.html",
        "ko/02-two.html",
    ] {
        assert!(site.join(rel).exists(), "the dev site has no {rel}");
    }

    edit(&book.join("book/ko/01-one.md"), |md| {
        md.replace("테스트가 고치는 문단.", "테스트가 고친 문단.")
    });

    let patch = poll(2);
    assert_eq!(
        keys_of(&patch),
        vec!["article:ko/01-one"],
        "a Korean prose edit should patch the Korean page alone: {patch}"
    );
    assert!(
        value_of(&patch, "article:ko/01-one").contains("테스트가 고친 문단."),
        "the edit did not reach the patch: {patch}"
    );

    edit(&book.join("book/en/02-two.md"), |md| {
        md.replace("more than one entry", "more than one entry (edited)")
    });

    let patch = poll(3);
    let keys = keys_of(&patch);
    for want in ["article:en/02-two", "article:ko/02-two"] {
        assert!(
            keys.contains(&want.to_string()),
            "an edit behind an untranslated page should patch {want}, got {keys:?}"
        );
    }

    let ko_two = std::fs::read_to_string(site.join("ko/02-two.html")).expect("the Korean chapter");
    assert!(
        ko_two.contains("more than one entry (edited)"),
        "the Korean chapter two was not rebuilt from the English source"
    );
    assert!(
        std::fs::read_to_string(site.join("ko/01-one.html"))
            .expect("the Korean chapter one")
            .contains("첫째 장"),
        "the Korean chapter one should be its translation, not the fallback"
    );

    let reached = generation_of(&work.join("run/hmr"));
    assert!(
        reached >= 2,
        "two edits should have published two generations"
    );
    drop(server);

    let other = site.join("en/02-two.html");
    std::fs::remove_file(&other).expect("remove a built page");
    let server = start();
    wait_for(&other, "the rebuild after the restart");

    edit(&book.join("book/en/01-one.md"), |md| {
        md.replace("The paragraph the test edited.", "Edited after a restart.")
    });
    let patch = poll(reached);
    assert!(
        value_of(&patch, "article:en/01-one").contains("Edited after a restart."),
        "an edit after a restart never reached a tab open from before it: {patch}"
    );

    drop(server);
    let _ = std::fs::remove_dir_all(&work);
}

/// The generation the feed has reached, as the builder left it on disk.
fn generation_of(hmr: &Path) -> u32 {
    std::fs::read_to_string(hmr.join("generation"))
        .expect("read the generation counter")
        .trim()
        .parse()
        .expect("the counter is a number")
}

/// Run the page's own patcher under node against a stub of the one node the op names, and return what it did to it.
fn applied_by_node(page: &str, patch: &str, key: &str, expected: &str) {
    let mut scripts = inline_scripts(page).into_iter();
    let runtime = scripts.next().expect("the served page has no script");
    assert!(
        runtime.contains("window.macaSignal"),
        "the first script on the page is not the patcher"
    );
    let poller = scripts
        .next()
        .expect("the served page has no second script");
    assert!(
        poller.contains("macaDevPath"),
        "the second script on the page is not the poller"
    );
    let harness = format!(
        r#"
const key = {key:?};
const node = {{ innerHTML: "", textContent: "", value: "",
  getAttribute: k => k === "data-signal-as" ? "html" : null,
  setAttribute: () => {{}} }};
global.document = {{ querySelectorAll: s =>
  s === '[data-signal="' + key + '"]' ? [node] : [] }};
global.window = global;
{runtime}
new Function({poller:?});
window.macaSignal({patch}.ops);
console.log(JSON.stringify({{ html: node.innerHTML, text: node.textContent }}));
"#
    );
    let dir = std::env::temp_dir().join("maca-hmr-node");
    let _ = std::fs::create_dir_all(&dir);
    let path = dir.join("apply.js");
    std::fs::write(&path, harness).expect("write the node harness");
    let out = Command::new("node").arg(&path).output().expect("run node");
    let printed = String::from_utf8_lossy(&out.stdout);
    assert!(
        printed.contains("\"html\":"),
        "node did not run the patcher: {printed}\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let got: serde_json::Value = serde_json::from_str(printed.trim()).expect("node's json");
    assert_eq!(
        got["html"].as_str().unwrap_or_default(),
        expected,
        "the patcher did not set innerHTML to the op's value"
    );
    assert_eq!(
        got["text"].as_str().unwrap_or_default(),
        "",
        "a region must not be written as text"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// Run the page's own poll loop under node against a stubbed `XMLHttpRequest`, and check the three things the server's side of the protocol assumes.
fn polled_by_node(page: &str) {
    let poller = inline_scripts(page)
        .into_iter()
        .find(|s| s.contains("macaDevPath"))
        .expect("the served page has no poller");
    let harness = format!(
        r#"
const urls = [];
const replies = ['{{"gen":20}}',
                 '{{"gen":21,"ops":[{{"key":"k","value":"v"}}]}}',
                 '{{"reload":true}}'];
let applied = null, reloaded = false;
global.window = global;
global.location = {{ reload: () => {{ reloaded = true; }} }};
global.document = {{ readyState: "complete", addEventListener: () => {{}} }};
global.macaSignal = ops => {{ applied = ops; }};
global.setTimeout = f => {{ if (urls.length < replies.length) f(); }};
global.XMLHttpRequest = function () {{
  this.open = (m, u) => {{ urls.push(u); this.i = urls.length - 1; }};
  this.send = () => {{ this.responseText = replies[this.i]; this.onload(); }};
}};
{poller}
console.log(JSON.stringify({{ urls, applied, reloaded }}));
"#
    );
    let dir = std::env::temp_dir().join("maca-hmr-poll");
    let _ = std::fs::create_dir_all(&dir);
    let path = dir.join("poll.js");
    std::fs::write(&path, harness).expect("write the node harness");
    let out = Command::new("node").arg(&path).output().expect("run node");
    let printed = String::from_utf8_lossy(&out.stdout);
    let got: serde_json::Value = serde_json::from_str(printed.trim()).unwrap_or_else(|_| {
        panic!(
            "node did not run the poller: {printed}\n{}",
            String::from_utf8_lossy(&out.stderr)
        )
    });
    let urls: Vec<&str> = got["urls"]
        .as_array()
        .expect("the poller made no request")
        .iter()
        .map(|u| u.as_str().unwrap_or_default())
        .collect();
    assert_eq!(
        urls,
        vec!["/_maca/hmr", "/_maca/hmr?gen=20", "/_maca/hmr?gen=21"],
        "the first poll must name no generation and every later one the last it was told"
    );
    assert_eq!(
        got["applied"][0]["key"].as_str(),
        Some("k"),
        "the poller did not hand its ops to the patcher"
    );
    assert_eq!(
        got["reloaded"].as_bool(),
        Some(true),
        "the poller ignored a reload"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// The body of every inline `<script>` on a page, in order.
fn inline_scripts(page: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = page;
    while let Some(i) = rest.find("<script>") {
        rest = &rest[i + "<script>".len()..];
        let Some(end) = rest.find("</script>") else {
            break;
        };
        out.push(rest[..end].to_string());
        rest = &rest[end..];
    }
    out
}

/// Poll the dev channel for a generation newer than `since`, and return the body.
fn poll(since: u32) -> String {
    let body = get(&format!("/_maca/hmr?gen={since}"));
    assert!(
        body.contains("\"ops\""),
        "the poll came back with no patch: {body:?}"
    );
    body
}

/// One `GET`, and the body of the reply.
fn get(path: &str) -> String {
    use std::io::{Read, Write};

    let mut sock = std::net::TcpStream::connect(("127.0.0.1", PORT)).expect("connect");
    sock.set_read_timeout(Some(Duration::from_secs(40)))
        .expect("set a read timeout");
    write!(
        sock,
        "GET {path} HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n"
    )
    .expect("send the request");
    let mut reply = Vec::new();
    sock.read_to_end(&mut reply).expect("read the reply");
    let reply = String::from_utf8_lossy(&reply).into_owned();
    match reply.split_once("\r\n\r\n") {
        Some((_, body)) => body.to_string(),
        None => panic!("no blank line in the reply: {reply:?}"),
    }
}

fn keys_of(patch: &str) -> Vec<String> {
    let v: serde_json::Value = serde_json::from_str(patch).expect("the patch is json");
    v["ops"]
        .as_array()
        .expect("ops is an array")
        .iter()
        .map(|o| o["key"].as_str().unwrap_or_default().to_string())
        .collect()
}

fn value_of(patch: &str, key: &str) -> String {
    let v: serde_json::Value = serde_json::from_str(patch).expect("the patch is json");
    v["ops"]
        .as_array()
        .expect("ops is an array")
        .iter()
        .find(|o| o["key"].as_str() == Some(key))
        .map(|o| o["value"].as_str().unwrap_or_default().to_string())
        .unwrap_or_else(|| panic!("no op for {key} in {patch}"))
}

/// Rewrite a source file through `f`, and make sure it really changed.
fn edit(path: &Path, f: impl Fn(String) -> String) {
    let before = std::fs::read_to_string(path).expect("read the chapter");
    let after = f(before.clone());
    assert_ne!(before, after, "the edit changed nothing in {path:?}");
    std::fs::write(path, after).expect("write the chapter");
}

fn wait_for(path: &Path, what: &str) {
    let deadline = Instant::now() + Duration::from_secs(30);
    while Instant::now() < deadline {
        if path.exists() {
            return;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    panic!("timed out waiting for {what}: {path:?}");
}

fn copy_tree(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).expect("create the copy");
    for e in std::fs::read_dir(from).expect("read the fixture").flatten() {
        let src = e.path();
        let dst = to.join(e.file_name());
        if src.is_dir() {
            copy_tree(&src, &dst);
        } else {
            std::fs::copy(&src, &dst).expect("copy a fixture file");
        }
    }
}
