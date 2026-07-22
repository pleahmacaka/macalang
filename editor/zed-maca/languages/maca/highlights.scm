; Tree-sitter highlight queries for Maca (Zed).
; Targets the grammar in editor/tree-sitter-maca.

(comment) @comment
(string) @string
(number) @number

; keywords (anonymous literal nodes from the grammar rules)
"import" @keyword
"from" @keyword

; operators / punctuation
"->" @operator
"=>" @operator
"/" @operator
":" @punctuation.delimiter
"." @punctuation.delimiter
"," @punctuation.delimiter
"(" @punctuation.bracket
")" @punctuation.bracket
"{" @punctuation.bracket
"}" @punctuation.bracket

; primitives and sized-numeric / SIMD types
((ident) @type.builtin
  (#match? @type.builtin "^(int|float|str|bool|bytes|unit|[iuf](8|16|32|64)(x[0-9]+)?)$"))

; nominal types / constructors are capitalized
((ident) @type
  (#match? @type "^[A-Z]"))

; language keywords that the scaffold grammar lexes as idents
((ident) @keyword
  (#match? @keyword "^(let|if|else|for|in|match|with|try|fail|alias)$"))

((ident) @constant.builtin
  (#match? @constant.builtin "^(true|false)$"))

; a call target `name(` reads as a function
(function (ident) @function)

; everything else is a plain identifier
(ident) @variable
