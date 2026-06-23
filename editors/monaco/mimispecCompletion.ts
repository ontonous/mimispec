/**
 * MimiSpec 代码补全提供者
 *
 * 功能：
 * 1. 关键词补全（根据上下文过滤）
 * 2. 代码片段补全（func/type/rule/flow/ui 等模板）
 * 3. 用户定义类型/函数/flow 状态补全（基于当前文件内容扫描）
 */

import type * as monaco from 'monaco-editor';

export type BlockContext = 'module' | 'func' | 'type' | 'rule' | 'flow' | 'ui' | 'steps' | 'unknown';

function inferBlockContext(line: string): BlockContext {
  if (line.match(/^\s*$/)) return 'unknown';
  if (line.match(/^module\s+/)) return 'module';
  if (line.match(/^func\s+/)) return 'func';
  if (line.match(/^type\s+/)) return 'type';
  if (line.match(/^rule\s+/)) return 'rule';
  if (line.match(/^flow\s+/)) return 'flow';
  if (line.match(/^ui\s+/)) return 'ui';
  if (line.match(/^steps:/)) return 'steps';
  if (line.match(/^parasteps/)) return 'steps';
  if (line.match(/^(if|else|for|while)\b/)) return 'steps';
  if (line.match(/^(requires|ensures|desc|on|with|done|exit|error)\b/)) return 'steps';
  return 'unknown';
}

function filterKeywordsByContext(keyword: string, context: BlockContext): boolean {
  const keywordsByContext: Record<BlockContext, string[]> = {
    module: ['type', 'rule', 'flow', 'func', 'ui', 'module', 'desc', 'math', 'and', 'or', 'not', 'in', 'true', 'false'],
    func: ['requires', 'ensures', 'steps', 'parasteps', 'desc', 'math', 'if', 'else', 'for', 'while', 'error', 'on', 'done', 'exit', 'and', 'or', 'not', 'in', 'true', 'false'],
    type: ['desc', 'math', 'and', 'or', 'not', 'in', 'true', 'false'],
    rule: ['desc', 'requires', 'and', 'or', 'not', 'in', 'true', 'false'],
    flow: ['desc', 'requires', 'and', 'or', 'not', 'in', 'true', 'false'],
    ui: ['stack', 'parallel', 'binds', 'desc', 'on', 'requires', 'with', 'and', 'or', 'not', 'in', 'true', 'false'],
    steps: ['desc', 'if', 'else', 'for', 'while', 'parasteps', 'error', 'on', 'done', 'exit', 'and', 'or', 'not', 'in', 'true', 'false'],
    unknown: ['module', 'type', 'rule', 'flow', 'func', 'ui', 'steps', 'parasteps', 'requires', 'ensures', 'math', 'desc', 'if', 'else', 'for', 'while', 'on', 'with', 'error', 'done', 'exit', 'and', 'or', 'not', 'in', 'true', 'false', 'parallel', 'stack', 'binds', '@import'],
  };
  return (keywordsByContext[context] || keywordsByContext.unknown).includes(keyword);
}

const SNIPPETS: Record<string, { label: string; detail: string; insertText: string; documentation: string }> = {
  func: {
    label: 'func',
    detail: '函数模板',
    insertText: 'func ${1:name}(${2:params}):\n    requires: ${3:condition}\n    ensures: ${4:result}\n    steps:\n        ${5:action} >>> done',
    documentation: '完整函数模板：参数、前置条件、后置条件、执行步骤',
  },
  funcSimple: {
    label: 'func (simple)',
    detail: '简单函数',
    insertText: 'func ${1:name}:\n    steps:\n        ${2:action} >>> done',
    documentation: '无参函数模板',
  },
  type: {
    label: 'type (enum)',
    detail: '枚举类型',
    insertText: 'type ${1:Name}: ${2:A} | ${3:B} | ${4:C}',
    documentation: '枚举类型：变体必须单行，用 | 分隔',
  },
  typeRecord: {
    label: 'type (record)',
    detail: '记录类型',
    insertText: 'type ${1:Name}:\n    ${2:field1}: ${3:typeHint}\n    ${4:field2}: ${5:typeHint}',
    documentation: '记录类型：缩进字段，后跟类型提示',
  },
  rule: {
    label: 'rule',
    detail: '业务规则',
    insertText: 'rule "${1:业务规则描述}"',
    documentation: '约束修饰符，无标签、无冒号，必须带自然语言描述字符串',
  },
  flow: {
    label: 'flow',
    detail: '状态机',
    insertText: 'flow ${1:Lifecycle}:\n    ${2:A} >>> ${3:B}: desc "${4:转移说明}"',
    documentation: '状态机：描述合法状态转移路径',
  },
  ui: {
    label: 'ui',
    detail: 'UI 视图',
    insertText: 'ui ${1:ViewName} binds ${2:Model}:\n    stack "${3:主面板}":\n        "${4:标题}" desc "${5:描述}"',
    documentation: 'UI 视图骨架：stack/parallel 布局与事件绑定',
  },
  steps: {
    label: 'steps',
    detail: '步骤块',
    insertText: 'steps:\n    ${1:action} >>> done',
    documentation: '函数执行步骤块',
  },
  parasteps: {
    label: 'parasteps',
    detail: '并行步骤',
    insertText: 'parasteps "${1:并行描述}":\n    ${2:action1}\n    ${3:action2}',
    documentation: '并行步骤块：内部动作同时执行',
  },
  if: {
    label: 'if',
    detail: '条件分支',
    insertText: 'if ${1:condition}:\n    ${2:action} >>> done\nelse:\n    ${3:alternative}',
    documentation: '条件分支：if/else 结构',
  },
  for: {
    label: 'for',
    detail: '遍历循环',
    insertText: 'for ${1:item} in ${2:collection}:\n    ${3:process}',
    documentation: '遍历循环：遍历集合中的每个元素',
  },
  while: {
    label: 'while',
    detail: '条件循环',
    insertText: 'while ${1:condition}:\n    ${2:action}\n    desc "${3:终止条件}"',
    documentation: '条件循环：建议始终带 desc 说明终止条件',
  },
  on: {
    label: 'on',
    detail: '补偿块',
    insertText: '${1:action}\non ${2:error_type}:\n    ${3:compensate} desc "${4:补偿说明}"\n    error "${5:错误信息}" >>> exit',
    documentation: '补偿/回滚块：紧跟在可能失败的步骤之后',
  },
  flowState: {
    label: 'flow state block',
    detail: '多出口状态',
    insertText: '${1:State}:\n    >>> ${2:NextState1}: desc "${3:分支1}"\n    >>> ${4:NextState2}: desc "${5:分支2}"',
    documentation: '多出口状态块：缩进成块的多条转移',
  },
  module: {
    label: 'module',
    detail: '模块',
    insertText: 'module ${1:ModuleName}:\n    ${2:type} ${3:Name}: ${4:A} | ${5:B}',
    documentation: '模块：命名空间，用于组织类型、规则、函数和 UI 视图',
  },
  requires: {
    label: 'requires',
    detail: '前置条件',
    insertText: 'requires: ${1:condition}',
    documentation: '前置条件：函数调用前必须满足的约束',
  },
  ensures: {
    label: 'ensures',
    detail: '后置条件',
    insertText: 'ensures: ${1:condition}',
    documentation: '后置条件：函数结束后保证成立的约束',
  },
  desc: {
    label: 'desc',
    detail: '描述',
    insertText: 'desc "${1:描述}"',
    documentation: '自然语言描述，是给 AI 的意图提示',
  },
  math: {
    label: 'math',
    detail: '数学块',
    insertText: 'math:\n    ${1:a} = ${2:b} + ${3:c}',
    documentation: '结构化数学表达式块，用于精确描述数值、张量与推导',
  },
};

export function createMimiSpecCompletionProvider(monacoInstance: typeof monaco): monaco.languages.CompletionItemProvider {
  const provider: monaco.languages.CompletionItemProvider = {
    triggerCharacters: [' ', ':', '\n', '.', '"', '(', '@'],

    provideCompletionItems: async (model, position) => {
      const lineContent = model.getLineContent(position.lineNumber);
      const context = inferBlockContext(lineContent);
      const userDefined = scanUserDefined(model.getValue());

      const suggestions: monaco.languages.CompletionItem[] = [];

      const allKeywords = [
        'module', 'type', 'rule', 'flow', 'func', 'ui', 'steps', 'parasteps',
        'requires', 'ensures', 'math', 'desc', 'if', 'else', 'for', 'while', 'on', 'with',
        'error', 'done', 'exit', 'parallel', 'stack', 'binds',
        'and', 'or', 'not', 'in', 'true', 'false', '@import',
      ];

      for (const keyword of allKeywords) {
        if (!filterKeywordsByContext(keyword, context)) continue;
        suggestions.push({
          label: keyword,
          kind: monacoInstance.languages.CompletionItemKind.Keyword,
          insertText: keyword,
          range: {
            startLineNumber: position.lineNumber,
            startColumn: position.column,
            endLineNumber: position.lineNumber,
            endColumn: position.column,
          },
        });
      }

      for (const [key, snippet] of Object.entries(SNIPPETS)) {
        if (context === 'steps' && !['if', 'for', 'while', 'on', 'steps', 'parasteps', 'desc'].includes(key)) continue;
        if (context === 'func' && !['func', 'funcSimple', 'requires', 'ensures', 'math', 'steps', 'desc'].includes(key)) continue;
        if (context === 'type' && !['type', 'typeRecord', 'math'].includes(key)) continue;
        if (context === 'flow' && !['flow', 'flowState', 'desc', 'requires'].includes(key)) continue;
        if (context === 'ui' && !['ui', 'stack', 'parallel', 'desc', 'on', 'requires'].includes(key)) continue;
        if (context === 'module' && !['module', 'type', 'typeRecord', 'rule', 'flow', 'func', 'ui', 'math'].includes(key)) continue;

        suggestions.push({
          label: snippet.label,
          kind: monacoInstance.languages.CompletionItemKind.Snippet,
          insertText: snippet.insertText,
          insertTextRules: monacoInstance.languages.CompletionItemInsertTextRule.InsertAsSnippet,
          documentation: { value: snippet.documentation },
          detail: snippet.detail,
          range: {
            startLineNumber: position.lineNumber,
            startColumn: position.column,
            endLineNumber: position.lineNumber,
            endColumn: position.column,
          },
        });
      }

      for (const typeName of userDefined.types) {
        suggestions.push({
          label: typeName,
          kind: monacoInstance.languages.CompletionItemKind.TypeParameter,
          insertText: typeName,
          documentation: `用户定义类型: ${typeName}`,
          detail: '[type]',
          range: {
            startLineNumber: position.lineNumber,
            startColumn: position.column,
            endLineNumber: position.lineNumber,
            endColumn: position.column,
          },
        });
      }

      for (const funcName of userDefined.functions) {
        suggestions.push({
          label: funcName,
          kind: monacoInstance.languages.CompletionItemKind.Function,
          insertText: funcName,
          documentation: `用户定义函数: ${funcName}`,
          detail: '[func]',
          range: {
            startLineNumber: position.lineNumber,
            startColumn: position.column,
            endLineNumber: position.lineNumber,
            endColumn: position.column,
          },
        });
      }

      for (const stateName of userDefined.flowStates) {
        suggestions.push({
          label: stateName,
          kind: monacoInstance.languages.CompletionItemKind.Enum,
          insertText: stateName,
          documentation: `Flow 状态: ${stateName}`,
          detail: '[flow state]',
          range: {
            startLineNumber: position.lineNumber,
            startColumn: position.column,
            endLineNumber: position.lineNumber,
            endColumn: position.column,
          },
        });
      }

      return { suggestions };
    },
  };

  return provider;
}

function scanUserDefined(content: string): { types: string[]; functions: string[]; flowStates: string[] } {
  const types: string[] = [];
  const functions: string[] = [];
  const flowStates: string[] = [];

  const lines = content.split('\n');

  for (const line of lines) {
    const trimmed = line.trim();

    const typeMatch = trimmed.match(/^type\s+(\w+)\s*:/);
    if (typeMatch) {
      types.push(typeMatch[1]);
      if (trimmed.includes('|')) {
        const variants = trimmed.split(':')[1]?.split('|').map(v => v.trim()).filter(v => v) || [];
        for (const variant of variants) {
          const variantName = variant.replace(/[\$\?]/g, '').trim();
          if (variantName && !flowStates.includes(variantName)) {
            flowStates.push(variantName);
          }
        }
      }
      continue;
    }

    const funcMatch = trimmed.match(/^func\s+(\w+)/);
    if (funcMatch) {
      functions.push(funcMatch[1]);
      continue;
    }

    const flowStateMatch = trimmed.match(/^(\w+)\s+>>>\s+(\w+)/);
    if (flowStateMatch && !flowStates.includes(flowStateMatch[1])) {
      flowStates.push(flowStateMatch[1]);
    }
  }

  return { types, functions, flowStates };
}

export function registerMimiSpecCompletionProvider(monacoInstance: typeof monaco, languageId: string): void {
  const provider = createMimiSpecCompletionProvider(monacoInstance);
  monacoInstance.languages.registerCompletionItemProvider(languageId, provider);
}
