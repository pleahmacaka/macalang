#!/bin/sh
# A MACA_EVAL_MODEL adapter for the `claude` CLI.
#
# The harness hands one argument, a prompt file, and reads Maca from stdout.
# `-p` is the headless mode; the model is overridable so a run can be repeated
# against a different one and the baseline says which it was.
#
# Fences are stripped here rather than in the harness: a model wrapping its
# answer in ```maca is answering the question, and the grader should be
# measuring the code rather than the packaging.
set -e

MODEL="${MACA_EVAL_CLAUDE_MODEL:-claude-haiku-4-5-20251001}"

claude -p --model "$MODEL" < "$1" 2>/dev/null |
  awk '
    /^[[:space:]]*```/ { infence = !infence; next }
    { print }
  '
