# Change Log

## [Unreleased]

- Replaced spawn-on-save validation with the long-lived MimiSpec 0.3 stdio
  language server while retaining an explicit 0.2.1 fallback.
- Added workspace-level `advisory`/`strict` collaboration configuration.

- Accept and verify the `mimispec.parse/0.3` CLI JSON envelope while retaining
  compatibility with unversioned 0.2.1 output.
- Read explicit complete/partial parse status instead of inferring only from a
  legacy success flag.
- Keep indentation rules aligned with actual block constructs.

## [0.2.0]

- Sync syntax highlighting and validation with MimiSpec v0.3.1.
- Highlight `math` keyword.
- Highlight arithmetic operators: `+`, `-`, `*`, `/`, `**`, `@`.
- Highlight bitwise operators: `&`, `|`, `^`, `~`, `<<`, `>>`.
- Highlight scientific-notation numbers (e.g. `1e-4`).
- Update auto-indent rules for `math`, `requires`, `ensures`, `stack`, `parallel`, `on`, `to` blocks.
- Diagnostics remain powered by the official `mimispec` CLI (`--json`).

## [0.1.0]

- Initial release.
- Language support for `.mms` files.
- Orange cat-paw file icon for `.mms` files.
- TextMate grammar for syntax highlighting.
- Diagnostics powered by the `mimispec` CLI (`--json`).
- Validate on save / open.
- Command: `MimiSpec: Validate Current File`.
