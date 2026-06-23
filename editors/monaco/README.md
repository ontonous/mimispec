# MimiSpec Monaco Editor 参考集成 / Monaco Editor Reference

Monaco Editor 的 MimiSpec 语言支持参考实现。
A reference implementation of MimiSpec language support for Monaco Editor.

## 文件说明 / Files

| 文件 / File | 说明 / Description |
|------|------|
| `mimispecLanguage.ts` | Monarch tokenizer + 语言配置（括号、自动闭合、缩进规则）/ Language definition (brackets, auto-closing, indentation) |
| `mimispecCompletion.ts` | 代码补全（关键词 + 片段 + 用户定义符号）/ Completion provider (keywords + snippets + user symbols) |

## 快速集成 / Quick Integration

```ts
import * as monaco from 'monaco-editor';
import { registerMimiSpecLanguage } from './mimispecLanguage';

registerMimiSpecLanguage(monaco);

const editor = monaco.editor.create(document.getElementById('container'), {
  language: 'mimispec',
});
```
