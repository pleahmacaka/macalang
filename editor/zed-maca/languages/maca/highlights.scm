; Tree-sitter highlight queries for Maca (Zed).
; Targets the grammar in editor/tree-sitter-maca.

; `///` is a doc comment — what `tools/macadoc.maca` reads as the marker that
; makes an item API. The grammar has one `comment` node, so the distinction is
; drawn by a predicate here rather than by regenerating the parser. The
; specific pattern has to come first.
((comment) @comment.doc
  (#match? @comment.doc "^///([^/]|$)"))
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
  (#match? @keyword "^(const|as|if|else|for|in|while|break|continue|match|with|try|fail|alias|await|spawn)$"))

((ident) @constant.builtin
  (#match? @constant.builtin "^(true|false)$"))

; a call target `name(` reads as a function
(function (ident) @function)

; everything else is a plain identifier
(ident) @variable
