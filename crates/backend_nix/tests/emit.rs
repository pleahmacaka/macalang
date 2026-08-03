use std::fs;
use std::path::PathBuf;

fn system_nix() -> String {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples/system.maca");
    let src = fs::read_to_string(p).unwrap();
    let parsed = maca_parser::parse(&src);
    assert!(parsed.errors.is_empty(), "{:?}", parsed.errors);
    maca_backend_nix::emit(&parsed.module)
}

#[test]
fn routing_and_injections() {
    let nix = system_nix();
    // system.* stays NixOS top-level
    assert!(nix.contains("networking.hostName = \"rigel\""), "{nix}");
    assert!(nix.contains("system.stateVersion = \"24.11\""));
    // system.packages -> environment.systemPackages
    assert!(
        nix.contains("environment.systemPackages = [ pkgs.git"),
        "{nix}"
    );
    // smart value: fonts hoist
    assert!(
        nix.contains("fonts.packages = [ pkgs.d2coding pkgs.noto-fonts ]"),
        "{nix}"
    );
    // implicit enable on a service block
    assert!(
        nix.contains("services.openssh = {") && nix.contains("enable = true"),
        "{nix}"
    );
    // user.* -> home-manager
    assert!(nix.contains("home-manager.users.alice"), "{nix}");
    assert!(nix.contains("home.packages = [ pkgs.fish"), "{nix}");
    // typed program merge
    assert!(nix.contains("programs.zed = {"), "{nix}");
    assert!(nix.contains("extensions = [ \"nix\" \"rust\" ]"), "{nix}");
    // non-destructive xdg.userDirs
    assert!(
        nix.contains("xdg.userDirs = {") && nix.contains("createDirectories = true"),
        "{nix}"
    );
    assert!(nix.contains("download = \"$HOME/Downloads\""), "{nix}");
}

#[test]
fn dev_flake_from_maca() {
    let src = "import nixpkgs\n\
               dev.name = \"proj\"\n\
               dev.packages = rustc, cargo, clang\n\
               dev.env = {\n    RUST_BACKTRACE = \"1\"\n}\n\
               dev.shellHook = \"echo hi\"\n";
    let parsed = maca_parser::parse(src);
    assert!(parsed.errors.is_empty(), "{:?}", parsed.errors);
    let flake = maca_backend_nix::emit_flake(&parsed.module);
    assert!(
        flake.contains("description = \"proj dev environment"),
        "{flake}"
    );
    assert!(flake.contains("inputs.nixpkgs.url"), "{flake}");
    assert!(flake.contains("devShells = forAllSystems"), "{flake}");
    assert!(
        flake.contains("packages = [ pkgs.rustc pkgs.cargo pkgs.clang ];"),
        "{flake}"
    );
    assert!(flake.contains("RUST_BACKTRACE = \"1\";"), "{flake}");
    assert!(flake.contains("shellHook = \"echo hi\";"), "{flake}");
    assert!(flake.contains("legacyPackages.${system}"), "{flake}");
    // balanced braces/brackets: a cheap validity check
    assert_eq!(
        flake.matches('{').count(),
        flake.matches('}').count(),
        "unbalanced braces"
    );
    assert_eq!(
        flake.matches('[').count(),
        flake.matches(']').count(),
        "unbalanced brackets"
    );
}

#[test]
fn windows_dev_scripts_from_maca() {
    let src = "dev.name = \"proj\"\n\
               dev.env = {\n    RUST_BACKTRACE = \"1\"\n}\n\
               scoop.buckets = main, java\n\
               scoop.packages = main.rust, java.temurin21-jdk\n\
               choco.packages = git\n\
               winget.packages = \"Nix.Nix\"\n";
    let parsed = maca_parser::parse(src);
    assert!(parsed.errors.is_empty(), "{:?}", parsed.errors);
    let win = maca_backend_nix::emit_windows_dev(&parsed.module).expect("some windows dev");
    assert_eq!(win.managers, vec!["scoop", "choco", "winget"]);

    // setup.ps1: portable scoop under .maca\dev, bucket-qualified installs
    assert!(
        win.setup.contains("$env:SCOOP = Join-Path $dev \"scoop\""),
        "{}",
        win.setup
    );
    assert!(win.setup.contains("scoop bucket add main"), "{}", win.setup);
    assert!(win.setup.contains("scoop bucket add java"), "{}", win.setup);
    assert!(
        win.setup
            .contains("scoop install main/rust java/temurin21-jdk"),
        "{}",
        win.setup
    );
    assert!(win.setup.contains("choco install git -y"), "{}", win.setup);
    assert!(
        win.setup.contains("winget install --id Nix.Nix"),
        "{}",
        win.setup
    );

    // activate.ps1: PATH, env injection, JDK auto-detect for JAVA_HOME
    assert!(
        win.activate
            .contains("$env:PATH = \"$env:SCOOP\\shims;$env:PATH\""),
        "{}",
        win.activate
    );
    assert!(
        win.activate.contains("$env:RUST_BACKTRACE = \"1\""),
        "{}",
        win.activate
    );
    assert!(win.activate.contains("$env:JAVA_HOME"), "{}", win.activate);
    assert!(win.activate.contains("temurin"), "{}", win.activate);

    // the flake ignores scoop/choco/winget entirely, so Nix hosts see none of it
    let flake = maca_backend_nix::emit_flake(&parsed.module);
    assert!(!flake.contains("scoop"), "{flake}");
    assert!(!flake.contains("choco"), "{flake}");
    assert!(!flake.contains("winget"), "{flake}");
    assert!(!flake.contains("temurin"), "{flake}");
}

#[test]
fn no_windows_config_means_no_scripts() {
    let src = "dev.name = \"proj\"\ndev.packages = rustc, cargo\n";
    let parsed = maca_parser::parse(src);
    assert!(parsed.errors.is_empty(), "{:?}", parsed.errors);
    assert!(maca_backend_nix::emit_windows_dev(&parsed.module).is_none());
}
