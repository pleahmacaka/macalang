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
    assert!(nix.contains("environment.systemPackages = [ pkgs.git"), "{nix}");
    // smart value: fonts hoist
    assert!(nix.contains("fonts.packages = [ pkgs.d2coding pkgs.noto-fonts ]"), "{nix}");
    // implicit enable on a service block
    assert!(nix.contains("services.openssh = {") && nix.contains("enable = true"), "{nix}");
    // user.* -> home-manager
    assert!(nix.contains("home-manager.users.alice"), "{nix}");
    assert!(nix.contains("home.packages = [ pkgs.fish"), "{nix}");
    // typed program merge
    assert!(nix.contains("programs.zed = {"), "{nix}");
    assert!(nix.contains("extensions = [ \"nix\" \"rust\" ]"), "{nix}");
    // non-destructive xdg.userDirs
    assert!(nix.contains("xdg.userDirs = {") && nix.contains("createDirectories = true"), "{nix}");
    assert!(nix.contains("download = \"$HOME/Downloads\""), "{nix}");
}
