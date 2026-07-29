//! Capstone 12a: MQTT broker + client (pure `.maca`, std/mqtt engine).
//! Pub/sub roundtrip with a `test/#` wildcard, and ≥100 concurrent subscribers.

mod common;
use common::*;

use std::path::PathBuf;
use std::process::Command;

fn app(name: &str) -> String {
    format!("{}/../../apps/mqtt/{name}", env!("CARGO_MANIFEST_DIR"))
}

fn build(app_file: &str, out_name: &str) -> PathBuf {
    let _lk = BuildLock::acquire();
    let out = std::env::temp_dir().join(out_name);
    let r = Command::new(env!("CARGO_BIN_EXE_maca"))
        .args(["build", &app(app_file), "-o", &out.to_string_lossy()])
        .output()
        .expect("spawn maca");
    assert!(
        r.status.success(),
        "build {app_file}: {}",
        String::from_utf8_lossy(&r.stderr)
    );
    out
}

#[test]
fn mqtt_broker_serves_pubsub_and_100_clients() {
    if !have_wsl() {
        eprintln!("skipping mqtt capstone: wsl not available");
        return;
    }
    let broker = to_wsl(&build("broker.maca", "maca-mqbroker"));
    let client = to_wsl(&build("client.maca", "maca-mqclient"));

    // One broker; a 1-client roundtrip, then 100 concurrent subscribers.
    let script = format!(
        r#"
pkill -x maca-mqbroker 2>/dev/null || true
sleep 0.5
cd /tmp && rm -f mqc_*.out sub1.out
{broker} >/dev/null 2>&1 & BR=$!
sleep 0.8
{client} sub > sub1.out 2>&1 &
sleep 0.4
{client} pub hello-roundtrip
sleep 0.7
echo "ROUNDTRIP: $(cat sub1.out)"
i=1; while [ $i -le 100 ]; do {client} sub > mqc_$i.out 2>&1 & i=$((i+1)); done
sleep 2.5
{client} pub concurrent-msg
sleep 2.5
echo "CONCURRENT: $(grep -l concurrent-msg mqc_*.out 2>/dev/null | wc -l)"
kill $BR 2>/dev/null || true
"#
    );
    // `-e` runs the command directly; backgrounded broker persists across the
    // script's steps (plain `wsl sh -c` tears it down).
    let out = Command::new("wsl")
        .args(["-e", "sh", "-c", &script])
        .output()
        .expect("wsl");
    let log = String::from_utf8_lossy(&out.stdout);

    assert!(
        log.contains("ROUNDTRIP: hello-roundtrip"),
        "pub/sub roundtrip failed.\n{log}\nstderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let count: usize = log
        .lines()
        .find_map(|l| l.strip_prefix("CONCURRENT: "))
        .and_then(|n| n.trim().parse().ok())
        .unwrap_or(0);
    assert!(
        count >= 100,
        "broker should serve ≥100 concurrent clients, got {count}\n{log}"
    );
}
