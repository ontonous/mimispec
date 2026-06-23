import type * as monaco from 'monaco-editor';
import { registerMimiSpecCompletionProvider } from './mimispecCompletion';

export const MIMISPEC_LANGUAGE_ID = 'mimispec';

/* ============================================================
   Language definition (Monarch tokenizer)
   ============================================================ */

export const mimispecLanguage = {
  defaultToken: 'invalid',
  tokenPostfix: '.mms',

  keywords: [
    'module',
    'type',
    'rule',
    'flow',
    'func',
    'ui',
    'stack',
    'parallel',
    'binds',
    'parasteps',
    'requires',
    'ensures',
    'math',
    'steps',
    'if',
    'else',
    'for',
    'while',
    'desc',
    'on',
    'with',
    'error',
    'and',
    'or',
    'not',
    'in',
    'done',
    'exit',
    'true',
    'false',
  ],

  operators: ['==', '!=', '<=', '>=', '<', '>', '=', '|', '+', '-', '*', '/', '**', '&', '^', '~', '<<', '>>', '>>>'],

  symbols: /[=><!:|\?]+/,

  tokenizer: {
    root: [
      [/@@import\b/, 'keyword'],
      [
        /[a-zA-Z_]\w*/,
        {
          cases: {
            '@keywords': 'keyword',
            '@default': 'identifier',
          },
        },
      ],
      [/"/, { token: 'string.quote', bracket: '@open', next: '@string' }],
      [/[0-9]+(?:\.[0-9]+)?(?:[eE][+-]?[0-9]+)?/, 'number'],
      [/[(),:\[\]]/, 'delimiter'],
      [/\.\.\./, 'annotation'],
      [/\./, 'operator'],
      [/\$\$?/, 'capture'],
      [/\?\?|\?/, 'question'],
      [/@/, 'operator'],
      [/[=<>!|+\-*\/&^~]+/, { cases: { '@operators': 'operator', '@default': 'delimiter' } }],
      [/\/\/.*$/, 'comment'],
      [/\s+/, 'white'],
    ],

    string: [
      [/[^\\"\n]+/, 'string'],
      [/\\./, 'string.escape'],
      [/"/, { token: 'string.quote', bracket: '@close', next: '@pop' }],
      [/\n/, { token: 'invalid', next: '@pop' }],
    ],
  },
} as monaco.languages.IMonarchLanguage;

export const mimispecLanguageConfiguration: monaco.languages.LanguageConfiguration = {
  comments: {
    lineComment: '//',
  },
  brackets: [
    ['(', ')'],
    ['[', ']'],
  ],
  autoClosingPairs: [
    { open: '"', close: '"' },
    { open: '(', close: ')' },
    { open: '[', close: ']' },
  ],
  surroundingPairs: [
    { open: '"', close: '"' },
    { open: '(', close: ')' },
    { open: '[', close: ']' },
  ],
  indentationRules: {
    increaseIndentPattern:
      /^\s*(module|type|flow|func|ui|steps|if|else|for|while|parasteps|math|requires|ensures|stack|parallel|on)\b.*:\s*$/,
    decreaseIndentPattern: /^\s*else\b.*$/,
  },
  wordPattern: /([a-zA-Z_][a-zA-Z0-9_]*)|\?+|\$+/,
};

/* ============================================================
   Registration
   ============================================================ */

export function registerMimiSpecLanguage(monacoInstance: typeof monaco) {
  if (monacoInstance.languages.getLanguages().some((l) => l.id === MIMISPEC_LANGUAGE_ID)) {
    return;
  }
  monacoInstance.languages.register({ id: MIMISPEC_LANGUAGE_ID, extensions: ['.mms'] });
  monacoInstance.languages.setMonarchTokensProvider(MIMISPEC_LANGUAGE_ID, mimispecLanguage);
  monacoInstance.languages.setLanguageConfiguration(MIMISPEC_LANGUAGE_ID, mimispecLanguageConfiguration);
  registerMimiSpecCompletionProvider(monacoInstance, MIMISPEC_LANGUAGE_ID);
}
