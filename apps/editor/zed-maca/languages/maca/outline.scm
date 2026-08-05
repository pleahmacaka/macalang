; Outline / breadcrumbs for Maca: what Zed lists in the symbol picker
; (`cmd-shift-o`) and shows in the breadcrumb bar.
;
; A function definition: its name, plus the parameter list and return type as
; context so overloads and signatures stay distinguishable in the list.
(function
  (ident) @name
  "(" @context
  (params)? @context
  ")" @context
  ("->" @context
   (type) @context)?) @item

; A type declaration: a record (`Point = { … }`) or a sum
; (`Color = Red | Green`).
(type_decl
  (type_ident) @name) @item

; A top-level constant binding (`const Limit = 100`, `Limit = 100`).
(binding
  "const"? @context
  target: (ident) @name) @item
