// tree-sitter grammar for Maca.
//
// Significant newlines are handled by the external scanner (src/scanner.c),
// which emits `_newline` between items and statements but suppresses it inside
// `(` / `[` groups. The grammar below covers the language surface a highlighter
// needs: imports, functions, bindings, sum/record type declarations, control
// flow (if / match / for / while), and the expression grammar (operators,
// ternary, calls, field/index, lambdas, ranges, string interpolation).

module.exports = grammar({
  name: 'maca',

  externals: $ => [$._newline],
  // The external scanner emits `_newline` at a significant line break and
  // declines at a continuation (a line starting with an operator, `.`, a closer,
  // …). A declined break falls through to `extras` here, so it's ignored, and
  // that is what lets an expression span lines. Declared `conflicts` let GLR
  // try both the separator and the continuation; the scanner decides at
  // runtime.
  extras: $ => [/\s/, $.comment],
  word: $ => $.ident,

  precedences: $ => [
    ['unary', 'mul', 'add', 'shift', 'compare', 'and', 'or', 'range', 'ternary'],
  ],

  conflicts: $ => [
    [$.record, $.block],
    // `name(args)` is a call or a function definition, distinguished only by
    // what follows `)` (`->` / `{` / `=>`). GLR explores both.
    [$.function, $._primary],
    [$.function, $.call],
    [$.record, $._primary],
    [$.lambda, $._primary],
    [$.lambda, $.param],
    [$.lambda, $.params],
    [$._path, $._primary],
    [$._lvalue, $._primary],
    [$._path, $.field],
    [$._unary_expr, $.try_post],
    [$._field, $._primary],
    [$.field_value, $.binding],
    [$.field_value, $._path],
    [$.field_value, $._lvalue],
    [$._stmt, $._item],
    [$.record, $.block, $._primary],
    [$.param, $._primary],
    [$.params, $._primary],
    // an item/statement boundary vs. continuing an expression onto the next
    // token; the scanner's `_newline` decides at runtime.
    [$._item, $.binary], [$._item, $.range], [$._item, $.ternary],
    [$._item, $.with_update], [$._item, $.try_post],
    [$._stmt, $.binary], [$._stmt, $.range], [$._stmt, $.ternary],
    [$._stmt, $.with_update], [$._stmt, $.try_post],
    // postfix continuation (`f(x)`, `a.b`, `xs[i]`) vs a new juxtaposed item.
    [$._unary_expr, $.call], [$._unary_expr, $.field],
    [$._unary_expr, $.index], [$._unary_expr, $.with_update],
    [$.import],
    [$.function],
    [$.variant],
    [$.sum_body],
    [$._item, $.sum_body],
    // a binding's value expression vs. continuing that expression.
    [$.binding, $.binary], [$.binding, $.range], [$.binding, $.ternary],
    [$.binding, $.with_update], [$.binding, $.try_post],
    [$.binding, $.call], [$.binding, $.field], [$.binding, $.index],
    // likewise a field value and a match-arm body.
    [$.field_value, $.binary], [$.field_value, $.range], [$.field_value, $.ternary],
    [$.field_value, $.with_update], [$.field_value, $.try_post],
    [$.field_value, $.call], [$.field_value, $.field], [$.field_value, $.index],
    [$.match_arm, $.binary], [$.match_arm, $.range], [$.match_arm, $.ternary],
    [$.match_arm, $.with_update], [$.match_arm, $.try_post],
    [$.match_arm, $.call], [$.match_arm, $.field], [$.match_arm, $.index],
    // an arrow-body function / lambda vs. continuing its body expression.
    [$.function, $.binary], [$.function, $.range], [$.function, $.ternary],
    [$.function, $.with_update], [$.function, $.try_post],
    [$.function, $.call], [$.function, $.field], [$.function, $.index],
    [$.lambda, $.binary], [$.lambda, $.range], [$.lambda, $.ternary],
    [$.lambda, $.with_update], [$.lambda, $.try_post],
    [$.lambda, $.call], [$.lambda, $.field], [$.lambda, $.index],
    [$.bare_list, $.binary], [$.bare_list, $.range], [$.bare_list, $.ternary],
    [$.bare_list, $.with_update], [$.bare_list, $.try_post],
    [$.bare_list, $.call], [$.bare_list, $.field], [$.bare_list, $.index],
    // a juxtaposed UI argument vs. continuing the previous arg as an expression.
    [$._arg, $.binary], [$._arg, $.range], [$._arg, $.ternary],
    [$._arg, $.with_update], [$._arg, $.try_post],
    [$._arg, $.call], [$._arg, $.field], [$._arg, $.index],
    [$.named_arg, $.binding], [$.directive, $._expr],
    [$.named_arg, $.binary], [$.named_arg, $.range], [$.named_arg, $.ternary],
    [$.named_arg, $.with_update], [$.named_arg, $.try_post],
    [$.named_arg, $.call], [$.named_arg, $.field], [$.named_arg, $.index],
    [$.directive, $.binary], [$.directive, $.range], [$.directive, $.ternary],
    [$.directive, $.with_update], [$.directive, $.try_post],
    [$.directive, $.call], [$.directive, $.field], [$.directive, $.index],
    [$.field_value, $.bare_list],
  ],

  rules: {
    source_file: $ => repeat(choice($._item, $._newline)),

    _item: $ => choice(
      $.import,
      $.type_decl,
      $.function,
      $.binding,
      $._expr,
    ),

    // ---- imports ----
    import: $ => seq(
      'import',
      choice(
        $.string,
        seq(optional(field('lang', $.ident)), $.string),
        seq($.ident, repeat(seq('/', $.ident))),
        seq('{', sepBy(',', $.ident), '}', 'from', $._module_path),
      ),
    ),
    _module_path: $ => seq($.ident, repeat(seq('/', $.ident))),

    // ---- type declarations ----
    // `Name = A | B | C`  (sum)   or   `Name = { field: T ... }` (record)
    type_decl: $ => seq(
      field('name', $.type_ident),
      '=',
      choice($.sum_body, $.record_type),
    ),
    sum_body: $ => seq(
      optional('|'),
      $.variant,
      repeat(seq(repeat($._newline), '|', repeat($._newline), $.variant)),
    ),
    variant: $ => seq($.type_ident, optional(seq('(', sepBy(',', $.type), ')'))),
    record_type: $ => seq(
      '{',
      repeat(choice($.field_decl, ',', $._newline)),
      '}',
    ),
    field_decl: $ => seq(field('name', $.ident), ':', $.type),

    // ---- functions ----
    function: $ => seq(
      field('name', $.ident),
      '(', optional($.params), ')',
      optional(seq('->', field('return', $.type))),
      optional(field('effects', $.effect_row)),
      // a `{…}` block or `=> expr` body, or none at all (an FFI/extern
      // declaration like `sqlite_open(path: str) -> int`). Prefer attaching a
      // following body over reading it as a separate block item.
      optional(prec.dynamic(1, choice($.block, seq('=>', $._expr)))),
    ),
    params: $ => sepBy1(',', $.param),
    param: $ => seq(optional('...'), field('name', $.ident), optional(seq(':', $.type))),
    effect_row: $ => seq('/', '<', sepBy(',', $.ident), '>'),

    // ---- bindings ----
    // `x = e`, `x: T = e`, or the config-mode layered form `name: Type: Base = e`.
    binding: $ => seq(
      optional('const'),
      field('target', $._lvalue),
      repeat(seq(':', $.type)),
      '=',
      choice($._expr, $.bare_list),
      optional(seq('as', 'const')),
    ),
    // a bracketless comma list as a value: `xs = 5, 3, 1`.
    bare_list: $ => prec.right(seq($._expr, repeat1(seq(',', $._expr)))),
    _lvalue: $ => choice($._path, $.index),
    _path: $ => seq($.ident, repeat(seq('.', $.ident))),

    // ---- types ----
    // a name, a dotted module type (`nixpkgs.zed`), with `[]` / `?` suffixes.
    type: $ => prec.left(seq(
      choice($.type_ident, $.ident),
      repeat(seq('.', choice($.type_ident, $.ident))),
      repeat(choice('[]', '?')),
    )),

    // ---- statements inside a block ----
    block: $ => seq(
      '{',
      repeat(choice($._stmt, $._newline)),
      '}',
    ),
    _stmt: $ => choice($.binding, $.return_like, $._expr),
    return_like: $ => choice($.break, $.continue),
    break: $ => 'break',
    continue: $ => 'continue',

    // ---- expressions ----
    _expr: $ => choice(
      $.ternary,
      $._binary,
      $._unary_expr,
    ),

    ternary: $ => prec.right('ternary', seq(
      field('cond', $._expr), '?', field('then', $._expr), ':', field('else', $._expr),
    )),

    _binary: $ => choice(
      $.binary,
      $.range,
    ),
    binary: $ => choice(
      prec.left('mul', seq($._expr, choice('*', '/', '%'), $._expr)),
      prec.left('add', seq($._expr, choice('+', '-', '++'), $._expr)),
      prec.left('shift', seq($._expr, choice('<<', '>>'), $._expr)),
      prec.left('compare', seq($._expr, choice('==', '!=', '<', '>', '<=', '>='), $._expr)),
      prec.left('and', seq($._expr, '&&', $._expr)),
      prec.left('or', seq($._expr, '||', $._expr)),
    ),
    range: $ => prec.left('range', seq($._expr, '..', $._expr)),

    _unary_expr: $ => choice(
      $.unary,
      $.await, $.spawn, $.fail, $.reify, $.try_post,
      $._postfix,
    ),
    unary: $ => prec('unary', seq(choice('-', '!'), $._unary_expr)),
    await: $ => prec('unary', seq('await', $._unary_expr)),
    spawn: $ => prec('unary', seq('spawn', $._unary_expr)),
    fail: $ => prec('unary', seq('fail', $._unary_expr)),
    reify: $ => prec('unary', seq('try', $._unary_expr)),
    try_post: $ => prec('unary', seq($._postfix, '?')),

    _postfix: $ => choice(
      $.call,
      $.field,
      $.index,
      $.with_update,
      $._primary,
    ),
    call: $ => prec.left('unary', seq(
      field('callee', $._postfix),
      '(',
      // arguments separated by commas or, in the reactive-UI DSL, juxtaposition.
      optional(seq($._arg, repeat(seq(optional(','), $._arg)))),
      ')',
    )),
    _arg: $ => choice($.directive, $.named_arg, $._expr),
    named_arg: $ => seq(field('name', $.ident), '=', $._expr),
    // reactive-UI directives: `bind:value=x`, `on:click=handler`.
    directive: $ => seq(field('kind', $.ident), ':', field('prop', $.ident), '=', $._expr),
    field: $ => prec.left('unary', seq($._postfix, '.', $.ident)),
    index: $ => prec.left('unary', seq($._postfix, '[', $._expr, ']')),
    with_update: $ => prec.left(seq($._postfix, 'with', $.record)),

    _primary: $ => choice(
      $.if,
      $.match,
      $.for,
      $.while,
      $.lambda,
      $.record,
      $.list,
      $.number,
      $.string,
      $.bool,
      $.ident,
      $.type_ident,
      seq('(', $._expr, ')'),
    ),

    if: $ => prec.right(seq(
      'if', field('cond', $._expr), $.block,
      optional(seq('else', choice($.if, $.block))),
    )),
    match: $ => seq(
      'match', field('scrutinee', $._expr),
      '{', repeat(choice($.match_arm, $._newline)), '}',
    ),
    match_arm: $ => seq(
      field('pattern', $.pattern),
      optional(seq('if', field('guard', $._expr))),
      '=>',
      field('body', choice($._expr, $.block)),
    ),
    // an or-pattern (`A | B`), or a comma sequence with an optional `..rest`
    // (`"add", ..rest`) as used by list/argument matches.
    pattern: $ => choice(
      seq($._pattern, repeat(seq('|', $._pattern))),
      seq($._pattern, repeat(seq(',', $._pattern)), optional(seq(',', $.rest_pattern))),
    ),
    rest_pattern: $ => seq('..', optional($.ident)),
    _pattern: $ => choice(
      $.ctor_pattern,
      $.record_pattern,
      $.number, $.string, $.bool, $.ident, $.type_ident, $.list, $.wildcard,
    ),
    // `Leaf(n)` / `Cons(x, rest)`: a variant pattern that binds its payload.
    ctor_pattern: $ => seq($.type_ident, '(', sepBy(',', $._pattern), ')'),
    // `{ head, tail }`: a record destructuring pattern.
    record_pattern: $ => seq('{', sepBy(',', $.ident), '}'),
    wildcard: $ => '_',
    for: $ => seq('for', field('pat', $.ident), 'in', field('iter', $._expr), $.block),
    while: $ => seq('while', field('cond', $._expr), $.block),

    lambda: $ => prec.right(seq(
      choice($.ident, seq('(', optional($.params), ')')),
      '=>',
      choice($.assign, $._expr, $.block),
    )),
    // a UI setter used as a lambda body: `v => age = int(v)`.
    assign: $ => prec.right(seq(field('target', $._lvalue), '=', $._expr)),

    record: $ => seq(
      optional(field('ctor', $.type_ident)),
      '{',
      repeat(choice($._field, ',', $._newline)),
      '}',
    ),
    _field: $ => choice($.field_value, $.ident),
    field_value: $ => seq(field('name', $.ident), '=', choice($._expr, $.bare_list)),

    list: $ => seq('[', optional(sepBy(',', $._expr)), ']'),

    // ---- terminals ----
    ident: $ => /[a-z_][A-Za-z0-9_]*(-[A-Za-z][A-Za-z0-9_]*)*/,
    type_ident: $ => /[A-Z][A-Za-z0-9_]*/,
    bool: $ => choice('true', 'false'),
    number: $ => /0[xX][0-9a-fA-F_]+|0[bB][01_]+|0[oO][0-7_]+|[0-9][0-9_]*(\.[0-9_]+)?/,
    string: $ => choice(
      seq('"""', repeat(choice(/[^"]/, /"[^"]/, /""[^"]/)), '"""'),
      seq('"', repeat(choice($._string_char, $._brace_escape, $.interpolation)), '"'),
    ),
    // literal text runs (excluding `"`, `\`, and braces), `\"`-style escapes,
    // and the `{{` / `}}` doubled-brace escapes for a literal brace.
    _string_char: $ => token.immediate(prec(1, /([^"\\{}]|\\.)+/)),
    _brace_escape: $ => token.immediate(choice('{{', '}}')),
    interpolation: $ => seq('{', $._expr, '}'),
    comment: $ => token(choice(
      seq('//', /.*/),
      seq('/*', /[^*]*\*+([^/*][^*]*\*+)*/, '/'),
    )),
  },
});

function sepBy1(sep, rule) {
  return seq(rule, repeat(seq(sep, rule)));
}
function sepBy(sep, rule) {
  return optional(sepBy1(sep, rule));
}
