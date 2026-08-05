# Releasing

A release is a tag. The tag is the bare version, with no `v` prefix:

```sh
git tag -a 0.3.2 -m "maca 0.3.2"
git push origin 0.3.2
```

Where a tag push is not available, run the **release** workflow by hand
(Actions, release, Run workflow) with a `version` input. It creates the tag and
the release from the current commit.

## What the tag builds

[`.github/workflows/release.yml`](../.github/workflows/release.yml) runs two
matrices over the same five targets, Linux and macOS on `x86_64` and
`aarch64` plus Windows `x86_64`:

* **the toolchain**, `maca` and `maca-lsp`, packaged as
  `maca-<os>-<arch>.tar.gz` (`.zip` on Windows);
* **the installer**, `maca-install-<os>-<arch>`, one binary per platform.

The Linux legs also compile and run a program that imports `std/text` from a
temp directory, which is the only place the carried standard library can be
seen to be missing: inside a checkout the source tree answers instead.

## Bumping the version

Five files carry it, and
[`crates/driver/tests/version.rs`](../crates/driver/tests/version.rs) fails
when they disagree:

* `Cargo.toml`, which every crate inherits and `maca --version` prints
* `maca.toml`
* `apps/npm/package.json`
* `apps/editor/zed-maca/extension.toml`
* `apps/editor/zed-maca/Cargo.toml`, which has to match the line above it or
  Zed refuses the extension

The newest `## x.y.z` heading in [CHANGELOG.md](CHANGELOG.md) has to agree too,
and a sixth test refuses a version literal anywhere else in `crates/`, because
a copy nobody compares is a copy that goes stale.

The installer carries no version of its own. It asks the binary it just
installed.
