// External scanner for Maca's significant newlines.
//
// A newline separates statements and top-level items, except inside `(` / `[`
// groups and string interpolation, where the real lexer suppresses it. Tree-
// sitter drives this by validity: `_newline` is only marked valid where the
// grammar expects a separator (never mid-expression inside a group), so the
// scanner emits it when valid and otherwise declines, letting `extras` swallow
// the whitespace. Consecutive blank lines collapse into one separator.

#include "tree_sitter/parser.h"
#include <wctype.h>

enum TokenType { NEWLINE };

void *tree_sitter_maca_external_scanner_create(void) { return NULL; }
void tree_sitter_maca_external_scanner_destroy(void *p) { (void)p; }
unsigned tree_sitter_maca_external_scanner_serialize(void *p, char *b) {
  (void)p;
  (void)b;
  return 0;
}
void tree_sitter_maca_external_scanner_deserialize(void *p, const char *b, unsigned n) {
  (void)p;
  (void)b;
  (void)n;
}

bool tree_sitter_maca_external_scanner_scan(void *payload, TSLexer *lexer,
                                            const bool *valid_symbols) {
  (void)payload;
  // Consume a run of horizontal whitespace and newlines (blank lines collapse).
  // Emit a single separator only where the grammar expects one (`_newline`
  // valid) and at least one line break was crossed; otherwise the whitespace is
  // simply skipped, which is what suppresses newlines inside `(` / `[` groups.
  bool saw_newline = false;
  while (lexer->lookahead == ' ' || lexer->lookahead == '\t' ||
         lexer->lookahead == '\r' || lexer->lookahead == '\n') {
    if (lexer->lookahead == '\n') saw_newline = true;
    lexer->advance(lexer, true);
  }
  if (!saw_newline || !valid_symbols[NEWLINE]) return false;

  // Line continuation: a line that *starts* with a binary/ternary operator, a
  // method dot, a closing bracket, or a comma is a continuation of the previous
  // one, so no separator is emitted. (`/` is excluded: it starts a comment.)
  switch (lexer->lookahead) {
    case '?': case ':': case '+': case '-': case '*': case '%':
    case '<': case '>': case '=': case '&': case '|': case '.':
    case ')': case ']': case '}': case ',':
      return false;
    default:
      break;
  }

  lexer->result_symbol = NEWLINE;
  return true;
}
