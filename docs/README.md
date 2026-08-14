# docs

What is here is for people working **on** Maca. What is for people working
**in** Maca is the handbook, built from `apps/tomo/book/{en,ko}` and published
at <https://pleahmacaka.github.io/macalang/>.

| file | what it answers |
|---|---|
| [SPEC.md](SPEC.md) | what the language is. The authority: when the code and this disagree, this wins, and both change together. |
| [CHANGELOG.md](CHANGELOG.md) | what changed in each release. |
| [BOOTSTRAP.md](BOOTSTRAP.md) | how the Maca compiler written in Maca is built and gated. |
| [LAYOUT.md](LAYOUT.md) | where everything is, and how an `import` finds a file. |
| [INTERNALS.md](INTERNALS.md) | the language surface, and the compiler's own structure. |
| [BACKENDS.md](BACKENDS.md) | what each back end emits, and what it refuses. |
| [DEVENV.md](DEVENV.md) | the development shell, which is itself a Maca file. |
| [RELEASING.md](RELEASING.md) | how a release is cut and what it contains. |
| [check-json.schema.json](check-json.schema.json) | the shape `maca check --json` prints, for a tool that reads it. |

The repository's own working rules, for a contributor or an agent, are in
`AGENT.md` at the root, which `CLAUDE.md` imports.
