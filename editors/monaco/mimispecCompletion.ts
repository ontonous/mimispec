/**
 * MimiSpec 代码补全提供者
 *
 * 功能：
 * 1. 关键词补全（根据上下文过滤）
 * 2. 代码片段补全（func/type/rule/flow/ui 等模板）
 * 3. 用户定义类型/函数/flow 状态补全（基于当前文件内容扫描）
 */

import type * as monaco from 'monaco-editor';

export type BlockContext = 'module' | 'func' | 'type' | 'rule' | 'flow' | 'ui' | 'steps' | 'root' | 'unknown';

function inferBlockContext(line: string): BlockContext {
  if (line.match(/^\s*$/)) return 'unknown';
  if (line.match(/^@import\b/)) return 'root';
  if (line.match(/^module[?$]*\s+/)) return 'module';
  if (line.match(/^func[?$]*\s+/)) return 'func';
  if (line.match(/^type[?$]*\s+/)) return 'type';
  if (line.match(/^rule[?$]*\s+/)) return 'rule';
  if (line.match(/^flow[?$]*(?:\s+|:)/)) return 'flow';
  if (line.match(/^ui[?$]*\s+/)) return 'ui';
  if (line.match(/^steps[?$]*:/)) return 'steps';
  if (line.match(/^parasteps[?$]*/)) return 'steps';
  if (line.match(/^(if|else|for|while|error)[?$]*\b/)) return 'steps';
  if (line.match(/^(requires|ensures|desc|on|with|done|exit)[?$]*\b/)) return 'steps';
  return 'unknown';
}

function filterKeywordsByContext(keyword: string, context: BlockContext): boolean {
  const keywordsByContext: Record<BlockContext, string[]> = {
    root: ['@import'],
    module: ['type', 'rule', 'flow', 'func', 'ui', 'module', 'steps', 'requires', 'ensures', 'desc', 'math', 'and', 'or', 'not', 'in', 'true', 'false'],
    func: ['requires', 'ensures', 'steps', 'parasteps', 'desc', 'math', 'if', 'else', 'for', 'while', 'error', 'on', 'done', 'exit', 'and', 'or', 'not', 'in', 'true', 'false'],
    type: ['desc', 'math', 'rule', 'and', 'or', 'not', 'in', 'true', 'false'],
    rule: ['desc', 'requires', 'and', 'or', 'not', 'in', 'true', 'false'],
    flow: ['rule', 'desc', 'on', 'requires', 'ensures', 'and', 'or', 'not', 'in', 'true', 'false'],
    ui: ['stack', 'parallel', 'binds', 'desc', 'on', 'requires', 'with', 'and', 'or', 'not', 'in', 'true', 'false'],
    steps: ['rule', 'desc', 'if', 'else', 'for', 'while', 'parasteps', 'error', 'on', 'done', 'exit', 'and', 'or', 'not', 'in', 'true', 'false'],
    unknown: ['@import', 'module', 'type', 'rule', 'flow', 'func', 'ui', 'steps', 'parasteps', 'requires', 'ensures', 'math', 'desc', 'if', 'else', 'for', 'while', 'on', 'with', 'error', 'done', 'exit', 'and', 'or', 'not', 'in', 'true', 'false', 'parallel', 'stack', 'binds'],
  };
  return (keywordsByContext[context] || keywordsByContext.unknown).includes(keyword);
}

const SNIPPETS: Record<string, { label: string; detail: string; insertText: string; documentation: string }> = {
  importDirective: {
    label: '@import',
    detail: '跨文件引用',
    insertText: '@import "${1:path/to/file.mms}"',
    documentation: '跨文件引用指令，位于所有 Fragment 之前',
  },
  func: {
    label: 'func',
    detail: '函数模板',
    insertText: 'func ${1:name}(${2:params}):\n    requires: ${3:condition}\n    ensures: ${4:result}\n    steps:\n        "${5:action}" >>> done',
    documentation: '完整函数模板；包含空格、连字符或 on/desc/error 等结构词的动作标签应整体加引号',
  },
  funcSimple: {
    label: 'func (simple)',
    detail: '简单函数',
    insertText: 'func ${1:name}:\n    steps:\n        "${2:action}" >>> done',
    documentation: '无参函数模板；自由动作标签默认加引号，避免与结构关键字冲突',
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
    insertText: 'flow ${1:Lifecycle}:\n    ${2:A}:\n        on ${3:Event} >>> ${4:B}: desc "${5:转移说明}"',
    documentation: '开放世界 Flow：用可选事件标签描述当前已知状态转移',
  },
  flowAnonymous: {
    label: 'flow (anonymous)',
    detail: '匿名状态意图',
    insertText: 'flow:\n    ${1:A}:\n        on ${2:Event} >>> ${3:B}: desc "${4:转移说明}"',
    documentation: '匿名 Context Flow：无需伪造或重复外部名称',
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
    insertText: 'steps:\n    "${1:action}" >>> done',
    documentation: '函数执行步骤块；自由文本动作应加引号，或使用 desc "..."',
  },
  actionQuoted: {
    label: 'action (quoted)',
    detail: '安全的动作标签',
    insertText: '"${1:动作标签}"',
    documentation: '将动作标签整体加引号；适合包含连字符、on、desc、error 等结构词的文本',
  },
  actionDesc: {
    label: 'action (natural language desc)',
    detail: '自然语言步骤',
    insertText: 'desc "${1:自然语言步骤}"',
    documentation: '用 desc 明确表达自然语言步骤，不会被解释为控制块或赋值',
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
    insertText: '${1:State}:\n    on ${2:Event1} >>> ${3:NextState1}: desc "${4:分支1}"\n    on ${5:Event2} >>> ${6:NextState2}: desc "${7:分支2}"',
    documentation: '多出口状态块：事件与目标保持为独立意图槽位',
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
    documentation: '可重复前置条款；同类多条按逻辑合取保留',
  },
  ensures: {
    label: 'ensures',
    detail: '后置条件',
    insertText: 'ensures: ${1:condition}',
    documentation: '可重复后置条款；同类多条按逻辑合取保留',
  },
  desc: {
    label: 'desc',
    detail: '描述',
    insertText: 'desc "${1:描述}"',
    documentation: '自然语言一等意图；无后缀不表示委托给 AI',
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
        '@import', 'module', 'type', 'rule', 'flow', 'func', 'ui', 'steps', 'parasteps',
        'requires', 'ensures', 'math', 'desc', 'if', 'else', 'for', 'while', 'on', 'with',
        'error', 'done', 'exit', 'parallel', 'stack', 'binds',
        'and', 'or', 'not', 'in', 'true', 'false',
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
        if (context === 'root' && key !== 'importDirective') continue;
        if (context === 'steps' && !['if', 'for', 'while', 'on', 'steps', 'parasteps', 'desc', 'actionQuoted', 'actionDesc'].includes(key)) continue;
        if (context === 'func' && !['func', 'funcSimple', 'requires', 'ensures', 'math', 'steps', 'desc'].includes(key)) continue;
        if (context === 'type' && !['type', 'typeRecord', 'rule', 'math'].includes(key)) continue;
        if (context === 'flow' && !['flow', 'flowAnonymous', 'flowState', 'rule', 'desc', 'requires', 'ensures'].includes(key)) continue;
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
          kind: monacoInstance.languages.CompletionItemKind.EnumMember,
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

      for (const uiName of userDefined.uiViews) {
        suggestions.push({
          label: uiName,
          kind: monacoInstance.languages.CompletionItemKind.Interface,
          insertText: uiName,
          documentation: `UI 视图: ${uiName}`,
          detail: '[ui]',
          range: {
            startLineNumber: position.lineNumber,
            startColumn: position.column,
            endLineNumber: position.lineNumber,
            endColumn: position.column,
          },
        });
      }

      for (const flowName of userDefined.flowNames) {
        suggestions.push({
          label: flowName,
          kind: monacoInstance.languages.CompletionItemKind.Module,
          insertText: flowName,
          documentation: `Flow 定义: ${flowName}`,
          detail: '[flow]',
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

function scanUserDefined(content: string): { types: string[]; functions: string[]; flowStates: string[]; uiViews: string[]; flowNames: string[] } {
  const types: string[] = [];
  const functions: string[] = [];
  const flowStates: string[] = [];
  const uiViews: string[] = [];
  const flowNames: string[] = [];

  for (const line of content.split('\n')) {
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

    const flowMatch = trimmed.match(/^flow[?$]*\s+(\w+)[?$]*\s*:/);
    if (flowMatch) {
      flowNames.push(flowMatch[1]);
      continue;
    }

    const uiMatch = trimmed.match(/^ui\s+(\w+)/);
    if (uiMatch) {
      uiViews.push(uiMatch[1]);
      continue;
    }

    const flowStateMatch = trimmed.match(/^(\w+)[?$]*\s+>>>[?$]*\s+(\w+)[?$]*/);
    if (flowStateMatch) {
      if (!flowStates.includes(flowStateMatch[1])) {
        flowStates.push(flowStateMatch[1]);
      }
      if (!flowStates.includes(flowStateMatch[2])) {
        flowStates.push(flowStateMatch[2]);
      }
    }
  }

  return { types, functions, flowStates, uiViews, flowNames };
}

export function registerMimiSpecCompletionProvider(monacoInstance: typeof monaco, languageId: string): void {
  const provider = createMimiSpecCompletionProvider(monacoInstance);
  monacoInstance.languages.registerCompletionItemProvider(languageId, provider);
}
