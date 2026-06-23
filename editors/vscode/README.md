# MimiSpec for VS Code

[MimiSpec](https://github.com/ontonous/mimispec) language support for Visual Studio Code.

## Features

- **Syntax highlighting** for `.mms` files.
- **File icon**: `.mms` files show a MimiSpec icon in the Explorer.
- **Diagnostics**: parse errors from the official `mimispec` CLI shown in the Problems panel.
- **Validation on save / open**.

## Requirements

The extension needs the `mimispec` CLI binary. It looks for it automatically at:

- `target/release/mimispec` (relative to workspace root)
- `target/debug/mimispec`

Or set the path in settings:

```json
{
  "mimispec.binaryPath": "/path/to/mimispec"
}
```

Build the binary:

```bash
cargo build --release
```

## Extension Settings

| Setting | Default | Description |
|---------|---------|-------------|
| `mimispec.binaryPath` | `null` | Absolute path to `mimispec` CLI. |
| `mimispec.validateOnSave` | `true` | Validate `.mms` on save. |
| `mimispec.validateOnOpen` | `true` | Validate `.mms` on open. |

## Development

```bash
cd editors/vscode
npm install
npm run compile
```

Open this folder in VS Code and press `F5` to launch Extension Development Host.

## Packaging

```bash
cd editors/vscode
npm install
npm run compile
npx vsce package
code --install-extension mimispec-vscode-*.vsix
```
