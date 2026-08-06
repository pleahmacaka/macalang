# 프로젝트: 스타일 린터

도구 하나를 통째로 만듭니다. 저장소의 `apps/lint/lint.maca`, Maca 소스가 실제로
지켜야 하는 린터입니다.

## 하는 일

규칙 넷입니다. 80칸을 넘는 줄, 한 줄짜리 `if` 블록, 줄 끝 공백, 하드 탭. 경로를
주면 모든 위반을 보고하고 0이 아닌 코드로 끝납니다.

## 한 줄부터 시작

가장 작은 유용한 조각은 한 줄에 대한 술어입니다.

```maca
has_trailing_space(line: str) -> bool =>
    line.length() > 0 && (line.ends_with(" ") || line.ends_with("\t"))
```

폭 규칙의 첫 버전은 틀렸습니다.

```maca
too_wide(line: str) -> bool =>
    line.length() > 80 && !line.trim().starts_with("//")
```

주석은 면제인데, 실제 코드베이스에 돌리면 한 줄이 긴 *문자열*인 줄들에서
걸립니다. 그래서 규칙은 문자열을 접은 상태로 줄을 잽니다.

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

`collapse`는 `chars()` 위를 재귀로 걸으며 커서, 플래그, 누산기를 넘기는 모양,
Maca에서 몇 번이고 쓰게 될 모양입니다. 이 변경으로 저장소의 지적이 65개에서
13개로 줄었습니다.

## 지적 모으기

각 규칙은 텍스트 한 줄을 내놓거나 아무것도 내놓지 않습니다.

```maca
line_issues(path: str, no: int, line: str) -> str =>
    say(path, no, too_wide(line), "line exceeds 80 columns")
        ++ say(path, no, single_line_if(line),
               "single-line `if` block; break it across lines")
        ++ space_issues(path, no, line)

say(path: str, no: int, hit: bool, what: str) -> str =>
    hit ? at(path, no) ++ what ++ "\n" : ""
```

리포트를 문자열로 쌓으면 모든 함수가 순수하고 테스트 가능한 상태로 남습니다.
IO를 하는 것은 `main`뿐입니다.

## 파일 읽기

```maca
lint_file(path: str) -> str =>
    scan_lines(path, read_file(path).split("\n"), 0, false, "")
```

`read_file`이 내용을 `str`로 돌려주고, `false`는 "raw 문자열 안인가"
플래그입니다. raw `"""…"""` 블록에는 외부 CSS와 JavaScript가 들어 있으므로 Maca
규칙을 적용하면 안 됩니다.

## 디렉터리 순회

```maca
lint_entry(path: str) -> str =>
    ends_with_maca(path)
        ? lint_file(path)
        : (list_dir(path).length() > 0 ? lint_dir(path) : "")
```

`is_dir` 기본 함수가 없어서, 파일에 대한 `list_dir`이 아무것도 찾지 못한다는
사실을 이용합니다.

## 인자와 종료 코드

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
```

`main(args: str[])`이 명령행을 받고, 반환값이 종료 상태이므로 이 도구를
pre-commit 훅에서 쓸 수 있습니다.

## 자기 자신에게 돌리기

```
maca run apps/lint/lint.maca
```

처음 돌렸을 때 자기 소스에서 문제를 보고했습니다. 자기 자신에게 돌리지 않는
린터는 제안일 뿐입니다.

## 만들면서 찾은 것

한 줄짜리 `if` 규칙은 `line.contains("{")`를 검사했는데 아무것에도 매치되지
않았습니다. 문자열 안의 `{`는 보간을 엽니다. 닫는 따옴표가 끝내주지 못하는
보간을 열었고, 뒤따르는 `"`가 *중첩된* 문자열을 열어 다음 따옴표까지의 소스를
삼켰습니다.

수정은 두 군데였습니다. 리터럴 중괄호는 `\{` 또는 `{{`이고, `"…"` 문자열은 더
이상 줄을 넘지 못합니다.

## 여기까지가 Maca 배우기입니다

값, 레코드, 합타입, 컬렉션, 에러, 함수, 모듈, 메모리, 테스트, 그리고 Maca가
다르게 하는 넷까지 손에 넣으셨습니다.

**[레퍼런스](a5-syntax.md)**는 문법에서 시작해 타입 시스템, 이펙트 행, 소유권
규칙, 모듈 해결 순서, 모든 타깃, UI 문법 전체, 툴체인, 표준 라이브러리, 그리고
모든 진단 메시지까지 이어집니다.

- [문법](a5-syntax.md): 모든 형태를 표로. 줄바꿈 규칙도 여기 있습니다.
- [표준 라이브러리](a3-stdlib.md): 모든 빌트인과 모든 메서드.
- [진단 메시지](a4-diagnostics.md): 지금 눈앞에 있는 그 메시지와 대처법.

언어가 아니라 프로젝트에 대한 장이 둘 있습니다.
[셀프호스팅 컴파일러](a15-self-hosting.md), 그리고 이 페이지를 만든
[Tomo](a16-tomo.md)입니다.
