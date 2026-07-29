//! Embedded target: `maca build --target embedded` produces a valid bare-metal
//! Cortex-M firmware image. Skips when clang can't cross-compile.

mod common;
use common::*;

use std::path::PathBuf;
use std::process::Command;

fn have_clang() -> bool {
    Command::new("clang")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

#[test]
fn blink_builds_a_cortex_m_image() {
    if !have_clang() {
        eprintln!("skipping: no clang");
        return;
    }
    let blink = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../apps/blink/blink.maca");
    let out = std::env::temp_dir().join("maca-embedded-test");
    let _ = std::fs::remove_dir_all(&out);

    let build = Command::new(maca())
        .args(["build", "--target", "embedded", "--mcu", "cortex-m4"])
        .arg(&blink)
        .arg("-o")
        .arg(&out)
        .output()
        .expect("spawn maca");
    assert!(
        build.status.success(),
        "build failed: {}",
        String::from_utf8_lossy(&build.stderr)
    );

    let elf = out.join("firmware.elf");
    let bin = out.join("firmware.bin");
    assert!(elf.exists(), "no ELF produced");
    // The raw .bin is only produced when llvm-objcopy is present; the ELF is the
    // authoritative artifact, so skip the byte-level checks if it's missing.
    if !bin.exists() {
        eprintln!("skipping .bin vector-table check: no llvm-objcopy");
        return;
    }

    // The image begins with the Cortex-M vector table: word 0 = initial stack
    // pointer, word 1 = reset vector (address | Thumb bit).
    let img = std::fs::read(&bin).unwrap();
    assert!(img.len() >= 8, "image too small");
    let sp = u32::from_le_bytes([img[0], img[1], img[2], img[3]]);
    let reset = u32::from_le_bytes([img[4], img[5], img[6], img[7]]);
    // RAM origin 0x20000000 + 128K
    assert_eq!(
        sp, 0x2002_0000,
        "initial SP should be top of RAM, got {sp:#010x}"
    );
    assert_eq!(reset & 1, 1, "reset vector must have the Thumb bit set");
    assert_eq!(
        reset >> 24,
        0x08,
        "reset vector should point into flash (0x08…)"
    );
}

#[test]
fn mmio_lowers_to_read_modify_write() {
    if !have_clang() {
        eprintln!("skipping: no clang");
        return;
    }
    let blink = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../apps/blink/blink.maca");
    let out = std::env::temp_dir().join("maca-embedded-test2");
    let _ = std::fs::remove_dir_all(&out);
    let ok = Command::new(maca())
        .args(["build", "--target", "embedded"])
        .arg(&blink)
        .arg("-o")
        .arg(&out)
        .output()
        .expect("spawn maca")
        .status
        .success();
    assert!(ok);
    let c = std::fs::read_to_string(out.join("firmware.c")).unwrap();
    // set_bits(odr, bit(12)) → volatile |= (1 << 12)
    assert!(c.contains("volatile uint32_t"), "no MMIO in emitted C");
    assert!(
        c.contains("|= (uint32_t)((1u << (12u)))"),
        "set_bits not lowered:\n{c}"
    );
    assert!(c.contains("Reset_Handler"), "no reset handler");
}

/// A freestanding image has no libc and no console, and its `main` is called by
/// the reset handler rather than by a process. Both used to reach the user as C
/// compiler noise about a file they never wrote.
#[test]
fn a_hosted_program_is_refused_with_a_reason() {
    let dir = std::env::temp_dir().join("maca-embedded-hosted");
    std::fs::create_dir_all(&dir).expect("scratch dir");

    for (name, src, want) in [
        (
            "console",
            "main() {\n    info(\"hi\")\n}\n",
            "needs a console",
        ),
        // `panic` is an output builtin too. The check kept its own list of
        // them, that list was missing this one, and it reached the image.
        (
            "panic",
            "main() {\n    panic(\"boom\")\n}\n",
            "needs a console",
        ),
        (
            "exit_code",
            "main() -> int {\n    0\n}\n",
            "returns nothing on a freestanding target",
        ),
    ] {
        let file = dir.join(format!("{name}.maca"));
        std::fs::write(&file, src).expect("write source");

        let out = Command::new(env!("CARGO_BIN_EXE_maca"))
            .args([
                "build",
                "--target",
                "embedded",
                &file.to_string_lossy(),
                "-o",
                &dir.join(name).to_string_lossy(),
            ])
            .output()
            .expect("spawn maca build");

        let text = String::from_utf8_lossy(&out.stdout).to_string()
            + &String::from_utf8_lossy(&out.stderr);
        assert!(!out.status.success(), "{name} should not build:\n{text}");
        assert!(text.contains(want), "{name}: expected {want:?} in:\n{text}");
    }
}
