# MimiSpec for VS Code

[MimiSpec](https://github.com/ontonous/mimispec) language support for Visual Studio Code.

## Features / 特性

- **Syntax highlighting** for `.mms` files (keywords, strings, comments, operators, commitment suffixes)
- **File icon**: `.mms` files show a MimiSpec icon in the Explorer
- **Diagnostics**: parse errors from the official `mimispec` CLI shown in the Problems panel
- **Validation on save / open** — real-time feedback as you edit

## Requirements / 前置条件

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

## Extension Settings / 设置

| Setting | Default | Description |
|---------|---------|-------------|
| `mimispec.binaryPath` | `null` | Absolute path to `mimispec` CLI |
| `mimispec.validateOnSave` | `true` | Validate `.mms` on save |
| `mimispec.validateOnOpen` | `true` | Validate `.mms` on open |

## Development / 开发

```bash
cd editors/vscode
npm install
npm run compile
```

Open this folder in VS Code and press `F5` to launch Extension Development Host.

## Packaging / 打包

```bash
cd editors/vscode
npm install
npm run compile
npx vsce package
code --install-extension mimispec-vscode-*.vsix
```

## Related / 相关

- [Monaco Editor integration](../monaco/) — browser-based editor support
- [Syntax specification](../../docs/specification.md) — full language reference
