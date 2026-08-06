# A Project: A Style Linter

`apps/lint/lint.maca` is the linter the Maca sources are held to. It reads
files, walks directories, takes command line arguments and sets an exit code.

## What it does

Four rules: a line over 80 columns, a single-line `if` block, trailing
whitespace, a hard tab. Given a path it reports every violation and exits
non-zero; given nothing it lints the repository's own sources.

## Starting with one line

```maca
has_trailing_space(line: str) -> bool =>
    line.length() > 0 && (line.ends_with(" ") || line.ends_with("\t"))
```

The width rule is more interesting than it looks, and the first version was
wrong:

```maca
too_wide(line: str) -> bool =>
    line.length() > 80 && !line.trim().starts_with("//")
```

Comments are exempt, because prose wraps differently from code. But run that
over a real codebase and it fires on lines that are one long *string*, which
cannot be rewrapped without changing what they mean. So the rule measures the
line with its strings collapsed:

```maca
too_wide(line: str) -> bool =>
    !line.trim().starts_with("//") && collapse_strings(line).length() > 80

collapse_strings(line: str) -> str =>
    collapse(line.chars(), 0, false, "")

collapse(cs: str[], i: int, quoted: bool, acc: str) -> str =>
    i >= cs.length()
        ? acc
        : (cs.get(i) == "\\" && quoted
            ? collapse(cs, i + 2, quoted, acc)
            : cs.get(i) == "\""
                ? collapse(cs, i + 1, !quoted, acc ++ "\"")
                : collapse(cs, i + 1, quoted, quoted ? acc : acc ++ cs.get(i)))
```

`collapse` is the shape you will write over and over in Maca: a recursive walk
over `chars()` threading a cursor, a flag, and an accumulator. That change took
the repository from 65 findings to 13, and all 13 were real.

## Collecting the complaints

Each rule contributes a line of text, or nothing:

```maca
line_issues(path: str, no: int, line: str) -> str =>
    say(path, no, too_wide(line), "line exceeds 80 columns")
        ++ say(path, no, single_line_if(line),
               "single-line `if` block; break it across lines")
        ++ space_issues(path, no, line)

say(path: str, no: int, hit: bool, what: str) -> str =>
    hit ? at(path, no) ++ what ++ "\n" : ""

at(path: str, no: int) -> str =>
    path ++ ":" ++ str(no) ++ ": "
```

Building a report as a string, rather than printing as you go, keeps every
function pure and testable. Only `main` does IO.

## Reading a file

```maca
lint_file(path: str) -> str =>
    scan_lines(path, read_file(path).split("\n"), 0, false, "")
```

`read_file` returns the contents as a `str`. The `false` is the "inside a raw
string" flag: raw `"""…"""` blocks hold foreign CSS and JavaScript, and the
Maca-shape rules should not apply to them.

## Walking a directory

```maca
lint_dir(dir: str) -> str =>
    lint_entries(dir, list_dir(dir), 0, "")

lint_entries(dir: str, names: str[], i: int, acc: str) -> str =>
    i >= names.length()
        ? acc
        : lint_entries(dir, names, i + 1,
                       acc ++ lint_entry(dir ++ "/" ++ names.get(i)))

lint_entry(path: str) -> str =>
    ends_with_maca(path)
        ? lint_file(path)
        : (list_dir(path).length() > 0 ? lint_dir(path) : "")
```

There is no `is_dir` primitive, so `lint_entry` uses the fact that `list_dir` of
a file finds nothing.

## Arguments and exit codes

```maca
main(args: str[]) -> int {
    report = args.length() > 0
        ? (file_exists(args.get(0)) ? pick(args.get(0)) : missing(args.get(0)))
        : lint_all(default_dirs(), 0, "")
    n = count_issues(report)
    n > 0 ? report_issues(report, n) : clean()
}

report_issues(report: str, n: int) -> int {
    print(report)
    info("{n} issue" ++ (n == 1 ? "" : "s"))
    1
}

clean() -> int {
    info("clean")
    0
}
```

`main(args: str[])` receives the command line. The return value is the exit
status, so `report_issues` returning `1` is what makes the tool usable in a
pre-commit hook.

## Running it on itself

```
maca run apps/lint/lint.maca
```

The first time this ran it reported issues in its own source.

## What building it found

The single-line-`if` rule tested `line.contains("{")`, and matched nothing,
ever. In Maca a `{` inside a string opens an interpolation, so `"{"` was not a
literal brace; it opened an interpolation that the closing quote never ended,
and the following `"` opened a *nested* string that swallowed source up to the
next quote. The program compiled. A binding several lines below vanished.

The fix was in two places. A literal brace is `\{` or `{{`, which the rule now
uses. And a `"…"` string may no longer span a line, so the mistake is a
diagnostic instead of a silent miscompile.

## That is Learning Maca

You have the language: values, records, sum types, collections, errors,
functions, modules, memory, tests, and the four things Maca does differently.

**[The Reference](a5-syntax.md)** starts at the grammar and goes through the
type system, the effect rows, the ownership rules, the module resolution order,
every target and what each one refuses, the UI syntax in full, the toolchain,
the standard library and every diagnostic.

Three entrances worth knowing by name:

- [Syntax](a5-syntax.md): every form, in tables, including the line-break rule
  that has one silent trap in it.
- [The Standard Library](a3-stdlib.md): every builtin and every method.
- [Diagnostics](a4-diagnostics.md): the message you are looking at right now,
  and what to do about it.

And two chapters about the project rather than the language: the
[self-hosted compiler](a15-self-hosting.md), which is where new compiler work
goes, and [Tomo](a16-tomo.md), which built the page you are reading.
