# MimiSpec Monaco Editor 参考集成 / Monaco Editor Reference

Monaco Editor 的 MimiSpec 语言支持参考实现。
A reference implementation of MimiSpec language support for [Monaco Editor](https://microsoft.github.io/monaco-editor/).

## 文件说明 / Files

| File | Description |
|------|-------------|
| `mimispecLanguage.ts` | Monarch tokenizer + language configuration (brackets, auto-closing, indentation) |
| `mimispecCompletion.ts` | Completion provider (Context clauses/rules, anonymous/event Flow snippets, and user symbols) |

The tokenizer recognizes all nine commitment suffix spellings. Completion is
heuristic and target-neutral; the canonical Rust parser remains authoritative.
Free-form Action labels that contain `on`, `desc`, `error`, hyphens, or other
structural punctuation should use the quoted-action snippet, or be written as
`desc "..."`. Monaco intentionally does not broaden the bare Action grammar.

## 快速集成 / Quick Integration

```ts
import * as monaco from 'monaco-editor';
import { registerMimiSpecLanguage } from './mimispecLanguage';

registerMimiSpecLanguage(monaco);

const editor = monaco.editor.create(document.getElementById('container'), {
  language: 'mimispec',
});
```

## Related / 相关

- [VS Code extension](../vscode/) — desktop editor support
- [Syntax specification](../../docs/specification.md) — full language reference
