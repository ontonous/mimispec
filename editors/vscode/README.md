# MimiSpec for VS Code

[MimiSpec](https://github.com/ontonous/mimispec) language support for Visual Studio Code.

## Features

- **Syntax highlighting** for `.mms` files.
- **File icon**: `.mms` files show an orange cat-paw icon in the Explorer.
- **Diagnostics**: parse errors from the official `mimispec` CLI are shown in the Problems panel.
- **Validation on save / open**.
- **Command**: `MimiSpec: Validate Current File`.

## Requirements

The extension needs the `mimispec` CLI binary. It will look for it automatically at:

- `target/release/mimispec` (relative to the workspace root)
- `target/debug/mimispec`

Or you can set the absolute path in VS Code settings:

```json
{
  "mimispec.binaryPath": "/absolute/path/to/mimispec"
}
```

Build the binary from the project root:

```bash
cargo build --release
```

## Extension Settings

| Setting | Default | Description |
|---------|---------|-------------|
| `mimispec.binaryPath` | `null` | Absolute path to the `mimispec` CLI. |
| `mimispec.validateOnSave` | `true` | Validate `.mms` files on save. |
| `mimispec.validateOnOpen` | `true` | Validate `.mms` files when opened. |

## Development

```bash
cd mimispec-vscode
npm install
npm run compile
```

To test locally, open this folder in VS Code and press `F5` to launch the Extension Development Host.

## Packaging

```bash
cd mimispec-vscode
npm install
npm run compile
npx vsce package
```

This produces a `.vsix` file that can be installed manually:

```bash
code --install-extension mimispec-vscode-0.1.0.vsix
```
