{
  description = "macalang dev environment — generated from dev.maca by `maca dev`";

  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";

  outputs = { self, nixpkgs }:
    let
      systems = [ "x86_64-linux" "aarch64-linux" "x86_64-darwin" "aarch64-darwin" ];
      forAllSystems = f: nixpkgs.lib.genAttrs systems (system: f nixpkgs.legacyPackages.${system});
    in {
      devShells = forAllSystems (pkgs: {
        default = pkgs.mkShell {
          packages = [ pkgs.rustc pkgs.cargo pkgs.rust-analyzer pkgs.clippy pkgs.rustfmt pkgs.clang pkgs.lld pkgs.llvm pkgs.jdk21 pkgs.nixpkgs-fmt ];
          RUST_BACKTRACE = "1";
          shellHook = "echo 'maca dev shell — cargo build | cargo test | maca --version'";
        };
      });
    };
}
