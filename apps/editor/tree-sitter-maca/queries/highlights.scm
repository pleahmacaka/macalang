; Syntax highlighting for Maca (tree-sitter).

; comments
;
; `///` is a doc comment: the marker `tools/macadoc.maca` reads to decide an
; item is API. The grammar has a single `comment` node (a third slash is not a
; token, exactly as it is not one to the compiler), so the distinction is drawn
; by a predicate. The specific pattern has to come first.
((comment) @comment.doc
  (#match? @comment.doc "^///([^/]|$)"))
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
