# MimiSpec Monaco Editor 参考集成

Monaco Editor（VS Code 核心编辑器）的 MimiSpec 语言支持参考实现。

## 文件说明

| 文件 | 说明 |
|------|------|
| `mimispecLanguage.ts` | Monarch tokenizer + 语言配置（括号、自动闭合、缩进规则） |
| `mimispecCompletion.ts` | 代码补全（关键词 + 片段 + 用户定义符号） |

## 快速集成

```ts
import * as monaco from 'monaco-editor';
import { registerMimiSpecLanguage } from './mimispecLanguage';

registerMimiSpecLanguage(monaco);

const editor = monaco.editor.create(document.getElementById('container'), {
  language: 'mimispec',
});
```
