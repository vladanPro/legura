# Source formatting V1

Hand-written app sources use the compiler-owned Axonyx formatter:

```bash
bash scripts/format-sources.sh
bash scripts/format-sources.sh --check
```

Use `AXONYX_FORMATTER` to select a development CLI executable. The published CLI
must contain the current readable ASX formatting rules before replacing the
immutable CI tooling pin. This is a source-tooling requirement, not a new CMS
runtime dependency.

The wrapper excludes `app/generated` and installed `app/components/legura-editor`
assets. Do not reformat content-addressed editor generations: their bytes belong
to the pinned package and its verified inventory. Parser/acceptance fixtures are
also outside this source baseline.

VS Code format-on-save uses `vladanpro.axonyx-vscode`, backed by the same Rust
formatter. A hook can optionally check staged content without fixing files or
staging unrelated edits:

```bash
git config core.hooksPath .githooks
# Optional: git config axonyx.formatterPath F:/CargoTarget/debug/cargo-ax.exe
# Disable: git config --unset core.hooksPath
```

Git configuration can be shared by worktrees. The formatting rollout does not
enable the hook or change configuration in an existing operator checkout.
Source formatting does not migrate a database, upgrade editor assets, or activate
an existing installation. Follow the normal backup-first operator procedure for
actual upgrades.
