// tree-sitter grammar for Maca (scaffold). Covers the token classes the
// highlighter needs; the full significant-newline layout is delegated to an
// external scanner (scanner.c) — a future extension.
module.exports = grammar({
  name: 'maca',
  extras: $ => [/\s/, $.comment],
  rules: {
    source_file: $ => repeat($._item),
    _item: $ => choice($.import, $.binding, $.function, $.comment),
    import: $ => seq('import', $._import_target),
    _import_target: $ => choice($.ident, $.string, seq($.ident, repeat(seq('/', $.ident)))),
    function: $ => seq($.ident, '(', optional($.params), ')', optional(seq('->', $.type)),
                      optional(choice($.block, seq('=>', $._expr)))),
    params: $ => seq($.param, repeat(seq(',', $.param))),
    param: $ => seq(optional('...'), $.ident, optional(seq(':', $.type))),
    binding: $ => seq($._path, optional(seq(':', $.type)), '=', $._expr),
    _path: $ => seq($.ident, repeat(seq('.', $.ident))),
    type: $ => seq($.ident, repeat(choice('[]', '?'))),
    block: $ => seq('{', repeat($._expr), '}'),
    _expr: $ => choice($.ident, $.number, $.string, $.block),
    ident: $ => /[A-Za-z_][A-Za-z0-9_]*(-[A-Za-z][A-Za-z0-9_]*)*/,
    // decimal (with `_` separators) or hex / binary / octal radix literals
    number: $ => /0[xX][0-9a-fA-F_]+|0[bB][01_]+|0[oO][0-7_]+|[0-9][0-9_]*(\.[0-9_]+)?/,
    string: $ => /"([^"\\]|\\.)*"/,
    // line `//` and block `/* … */` comments
    comment: $ => choice(token(seq('//', /.*/)), token(seq('/*', /[^*]*\*+([^/*][^*]*\*+)*/, '/'))),
  }
});
