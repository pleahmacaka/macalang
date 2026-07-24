; Syntax highlighting for Maca (tree-sitter).

; comments
(comment) @comment

; literals
(number) @number
(string) @string
(interpolation "{" @punctuation.special "}" @punctuation.special)
(bool) @constant.builtin

; types
(type_ident) @type
(type (type_ident) @type)
(type (ident) @type)
(variant (type_ident) @constructor)
(ctor_pattern (type_ident) @constructor)

; declarations
(function name: (ident) @function)
(call callee: (ident) @function.call)
(field (ident) @property)
(param name: (ident) @variable.parameter)
(field_decl name: (ident) @property)
(field_value name: (ident) @property)
(named_arg name: (ident) @property)

; identifiers
(ident) @variable

; keywords
[
  "import" "from" "const" "as" "with"
  "if" "else" "match" "for" "in" "while"
  "await" "spawn" "fail" "try"
] @keyword
[ (break) (continue) ] @keyword

; operators
[
  "+" "-" "*" "/" "%" "++"
  "==" "!=" "<" ">" "<=" ">="
  "&&" "||" "!" "<<" ">>"
  "=" "->" "=>" ".." "?" ":" "|"
] @operator

; punctuation
[ "(" ")" "[" "]" "{" "}" ] @punctuation.bracket
[ "," "." ] @punctuation.delimiter
