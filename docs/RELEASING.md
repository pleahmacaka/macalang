# Releasing

Releases are produced by GitHub Actions ([`.github/workflows/release.yml`](../.github/workflows/release.yml)),
triggered by pushing a plain semver tag, with **no `v` prefix** (`0.1.0`, not
`v0.1.0`); pre-releases like `0.1.0-rc1` also match:

```sh
git tag -a 0.1.0 -m "maca 0.1.0"
git push origin 0.1.0
```

Alternatively, run the **release** workflow manually (Actions → release → Run
workflow) with a `version` input (e.g. `0.1.0`); it creates the tag and release
from the current commit. Use this when a tag push isn't available.

On that tag the workflow:

1. builds `maca` + `maca-lsp` in release mode on five targets:
   Linux `x86_64` / `aarch64`, macOS `x86_64` / `aarch64`, Windows `x86_64`;
2. packages each as `maca-<os>-<arch>.tar.gz` (Unix) or `.zip` (Windows);
3. uploads the archives **plus `install.sh` and `install.ps1`** to the GitHub
   release for the tag.

Once the release exists, the one-liners in the README resolve automatically:

```sh
# Linux / macOS
curl -fsSL https://raw.githubusercontent.com/pleahmacaka/macalang/main/install.sh | bash
# Windows (PowerShell)
irm https://raw.githubusercontent.com/pleahmacaka/macalang/main/install.ps1 | iex
```

The installers download `maca-<os>-<arch>.{tar.gz,zip}` from
`releases/latest/download/…`; with no matching asset they fall back to building
from a source checkout.

## Notes

- The tag should point at a commit on the default branch (merge the release
  branch first). Cutting a tag is the only step a maintainer performs by hand;
  everything else is automated.
- Bump `version` to match the tag before releasing, in all five files that
  carry one: the root `Cargo.toml` (which every crate inherits, and which
  `maca --version` prints), `maca.toml`, `packages/macalang/package.json`,
  `editor/zed-maca/extension.toml` and `editor/zed-maca/Cargo.toml`. The last
  two have to agree with each other or Zed refuses the extension. The
  installers carry no version of their own: they ask the binary they just
  installed, so there is nothing to bump there.
