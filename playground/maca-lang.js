// Maca language support for Monaco: a Monarch tokenizer, language config, and
// two themes. Mirrors the lexer's keyword/type model and the TextMate grammar
// in `editor/maca.tmLanguage.json`. `registerMaca(monaco)` wires it all up.

export const MACA_KEYWORDS = [
  'let', 'if', 'else', 'for', 'in', 'match',
  'import', 'from', 'with', 'fail', 'try', 'alias',
];

export const MACA_PRIMITIVES = [
  'int', 'float', 'str', 'bool', 'bytes', 'unit',
  'f32', 'f64', 'i8', 'i16', 'i32', 'i64', 'u8', 'u16', 'u32', 'u64',
];

export const macaLanguage = {
  defaultToken: '',
  tokenPostfix: '.maca',

  keywords: MACA_KEYWORDS,
  typeKeywords: MACA_PRIMITIVES,
  constants: ['true', 'false'],

  operators: [
    '->', '=>', '++', '|>', '==', '!=', '<=', '>=', '&&', '||',
    '+', '-', '*', '/', '<', '>', '=', '|', '?', ':', '.',
  ],

  symbols: /[=><!~?:&|+\-*/^%.]+/,

  tokenizer: {
    root: [
      // path literals: /tmp ./x ../x ~/x  (first char after the slash prefix
      // must be a letter/underscore, so `1/2` and `a / b` stay division)
      [/(?:\.\.\/|\.\/|~\/|\/)[A-Za-z_][\w./-]*/, 'string.path'],

      // SIMD lane types: f32x8, i32x4, …
      [/[iuf](?:8|16|32|64)x(?:2|4|8|16|32|64)\b/, 'type'],

      // lowercase-leading identifiers / keywords (kebab segments allowed)
      [/[a-z_][A-Za-z0-9_]*(?:-[A-Za-z][A-Za-z0-9_]*)*/, {
        cases: {
          '@keywords': 'keyword',
          '@typeKeywords': 'type',
          '@constants': 'constant',
          '@default': 'identifier',
        },
      }],

      // Capitalized identifiers are nominal types / constructors
      [/[A-Z][A-Za-z0-9_]*/, 'type'],

      // numbers
      [/\d+\.\d+/, 'number.float'],
      [/\d+/, 'number'],

      { include: '@whitespace' },

      // strings
      [/"/, { token: 'string.quote', bracket: '@open', next: '@string' }],

      // delimiters and operators
      [/[{}()[\]]/, '@brackets'],
      [/@symbols/, { cases: { '@operators': 'operator', '@default': '' } }],
      [/,/, 'delimiter'],
    ],

    whitespace: [
      [/[ \t\r\n]+/, ''],
      [/\/\*/, 'comment', '@comment'],
      [/\/\/.*$/, 'comment'],
    ],

    comment: [
      [/[^/*]+/, 'comment'],
      [/\*\//, 'comment', '@pop'],
      [/[/*]/, 'comment'],
    ],

    string: [
      [/\{\{|\}\}/, 'string.escape'],
      [/\{/, { token: 'delimiter.bracket', next: '@interp' }],
      [/[^"\\{]+/, 'string'],
      [/\\./, 'string.escape'],
      [/"/, { token: 'string.quote', bracket: '@close', next: '@pop' }],
    ],

    // string interpolation `{ expr }` — re-enter expression rules
    interp: [
      [/\}/, { token: 'delimiter.bracket', next: '@pop' }],
      { include: 'root' },
    ],
  },
};

export const macaConfig = {
  comments: { lineComment: '//', blockComment: ['/*', '*/'] },
  brackets: [['{', '}'], ['[', ']'], ['(', ')']],
  autoClosingPairs: [
    { open: '{', close: '}' },
    { open: '[', close: ']' },
    { open: '(', close: ')' },
    { open: '"', close: '"' },
  ],
  surroundingPairs: [
    { open: '{', close: '}' },
    { open: '[', close: ']' },
    { open: '(', close: ')' },
    { open: '"', close: '"' },
  ],
};

const SHARED_RULES = (c) => [
  { token: 'keyword', foreground: c.keyword, fontStyle: 'bold' },
  { token: 'type', foreground: c.type },
  { token: 'constant', foreground: c.constant },
  { token: 'number', foreground: c.number },
  { token: 'number.float', foreground: c.number },
  { token: 'string', foreground: c.string },
  { token: 'string.quote', foreground: c.string },
  { token: 'string.escape', foreground: c.escape },
  { token: 'string.path', foreground: c.path },
  { token: 'comment', foreground: c.comment, fontStyle: 'italic' },
  { token: 'operator', foreground: c.operator },
  { token: 'delimiter.bracket', foreground: c.escape },
  { token: 'identifier', foreground: c.identifier },
];

export function registerMaca(monaco) {
  monaco.languages.register({ id: 'maca', extensions: ['.maca'], aliases: ['Maca', 'maca'] });
  monaco.languages.setMonarchTokensProvider('maca', macaLanguage);
  monaco.languages.setLanguageConfiguration('maca', macaConfig);

  monaco.editor.defineTheme('maca-dark', {
    base: 'vs-dark',
    inherit: true,
    rules: SHARED_RULES({
      keyword: 'c586c0', type: '4ec9b0', constant: '569cd6', number: 'b5cea8',
      string: 'ce9178', escape: 'd7ba7d', path: 'd7a3ff', comment: '6a9955',
      operator: 'd4d4d4', identifier: '9cdcfe',
    }),
    colors: { 'editor.background': '#1e1e2e' },
  });

  monaco.editor.defineTheme('maca-light', {
    base: 'vs',
    inherit: true,
    rules: SHARED_RULES({
      keyword: 'af00db', type: '267f99', constant: '0000ff', number: '098658',
      string: 'a31515', escape: 'b07d00', path: '7a3ecc', comment: '008000',
      operator: '333333', identifier: '001080',
    }),
    colors: {},
  });
}
