# Change Log

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
