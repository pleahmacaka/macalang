# Releasing

Releases are produced by GitHub Actions ([`.github/workflows/release.yml`](../.github/workflows/release.yml)),
triggered by pushing a plain semver tag — **no `v` prefix** (`0.1.0`, not
`v0.1.0`); pre-releases like `0.1.0-rc1` also match:

```sh
git tag -a 0.1.0 -m "maca 0.1.0"
git push origin 0.1.0
```

On that tag the workflow:

1. builds `maca` + `maca-lsp` in release mode on five targets —
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
  branch first). Cutting a tag is the only step a maintainer performs by hand —
  everything else is automated.
- Bump `version` in the root `Cargo.toml`, `extension.toml`, and this repo's
  `install.*` banner to match the tag before releasing.
