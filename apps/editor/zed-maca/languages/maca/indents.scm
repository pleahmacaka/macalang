; Auto-indent for Maca. Zed indents the body of each matched node by one unit
; and dedents the closing delimiter.

; Brace blocks: function bodies, `if`/`else`, `for`/`while`, `match` arms.
(block) @indent
(match) @indent

; Bracketed constructs that can span lines. (Call arguments are inlined into
; `call` by the grammar, so the call node itself carries the indent.)
(record) @indent
(list) @indent
(params) @indent
(call) @indent
