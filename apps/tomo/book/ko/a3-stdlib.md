# 표준 라이브러리

Maca의 "표준 라이브러리"는 대부분 컴파일러와 런타임의 빌트인이라 모든 타깃에서
쓸 수 있습니다.

## 출력

| 함수 | 하는 일 |
|---|---|
| `print(s)` | stdout에 개행 없이 씀 |
| `info(s)` | stdout에 한 줄 씀 |
| `err(s)` | stderr에 한 줄 씀 |

이름은 syslog 레벨에서 왔고, `warn` 이하는 stderr로 갑니다.

## 변환

| 함수 | 하는 일 |
|---|---|
| `str(x)` | 아무 값이나 텍스트로 |
| `int(s)` | 텍스트를 정수로 |
| `float(s)` | 텍스트를 실수로 |
| `len(x)` | 리스트나 문자열의 길이 |

## 문자열

UFCS로 메서드처럼 호출합니다. `s.trim()`은 `trim(s)`입니다.

| 메서드 | 결과 |
|---|---|
| `length()` | 바이트 길이 |
| `split(sep)` | `str[]` |
| `trim()` | 양끝 제거 |
| `upper()` `lower()` | 대소문자 |
| `contains(s)` | `bool` |
| `starts_with(s)` `ends_with(s)` | `bool` |
| `replace(from, to)` | 모든 occurrence |
| `substr(start, len)` | 끝이 아니라 **길이** |
| `slice(from, to)` | `to`는 리스트와 마찬가지로 **제외** |
| `index_of(s)` | 인덱스 또는 `-1` |
| `repeat(n)` | `str` |
| `pad_start(w, p)` `pad_end(w, p)` `pad_center(w, p)` | `p` 기본값은 공백 |
| `chars()` | 한 글자 문자열들의 `str[]` |
| `at(i)` | `i` 위치의 글자 |
| `is_whitespace()` `is_ascii_digit()` `is_alpha()` | 문자 분류 |
| `fixed(n)` | 소수점 `n`자리 텍스트 |

## 리스트

| 메서드 | 결과 |
|---|---|
| `map(f)` `filter(f)` | `T[]` |
| `reduce(init, f)` `fold(init, f)` | 값 하나 |
| `sort()` `reverse()` | `T[]` |
| `sort_by(key)` | `key(x)` 기준으로 정렬한 `T[]`, 안정적 |
| `push(x)` `pop()` | 새 리스트 |
| `set(i, x)` `insert(i, x)` `remove(i)` | `i`를 편집한 새 리스트 |
| `slice(from, to)` | `to`는 **제외** |
| `contains(x)` `index_of(x)` | 검색 |
| `index_of_by(f)` | `f(x)`가 참인 첫 인덱스, 없으면 `-1` |
| `enumerate()` | `{index, value}[]` |
| `sum()` `min()` `max()` | 수치 |
| `first()` `last()` `get(i)` | 원소 |
| `length()` | `int` |
| `join(sep)` | `str[]`를 하나의 `str`로 |
| `parallel(f)` | `map`과 같되 동시 실행 |

## 맵

`Map str V`는 문자열 키 해시 맵이고, 값 타입으로 단형화됩니다.

| 메서드 | 결과 |
|---|---|
| `set(k, v)` | `k`가 `v`에 묶인 맵 |
| `get(k, default)` | 값, 없으면 `default` |
| `has(k)` | `bool` |
| `remove(k)` | `k`가 빠진 맵 |
| `keys()` | 정렬된 `str[]` |
| `length()` | `int` |

```maca
counts: Map str int = map()
counts = counts.set("apple", 3).set("pear", 1)
info("{counts.get("apple", 0)}")     // 3
info("{counts.get("kiwi", 0)}")      // 0, 없으면 기본값
```

키는 `str`뿐이고, 정수 키는 `str(n)` 한 번 거리입니다. `keys()`는 정렬되어
나옵니다.

## 수학

| 함수 | 하는 일 |
|---|---|
| `sqrt(x)` `floor(x)` `ceil(x)` | `float`에 대해 |

## 파일과 디렉터리

| 함수 | 하는 일 |
|---|---|
| `read_file(path)` | 내용을 `str`로 |
| `write_file(path, text)` | 자르고 쓰기 |
| `file_exists(path)` | `bool` |
| `make_dir(path)` | `mkdir -p`처럼 |
| `list_dir(path)` | 정렬된 이름들의 `str[]` |
| `is_dir(path)` | `bool` |
| `file_size(path)` | 바이트 수, 없으면 `-1` |
| `modified_ms(path)` | 수정 시각(ms), 없으면 `-1` |
| `remove_file(path)` | 파일 삭제 |
| `remove_dir(path)` | 디렉터리와 그 내용 삭제 |
| `copy_bytes(src, dst)` | 바이트 단위 복사 |

없는 파일의 크기는 `0`이 아니라 `-1`이라, 빈 파일과 없는 파일을 한 번의
호출로 구분할 수 있습니다.

`copy_bytes`가 따로 있는 이유는 `write_file(dst, read_file(src))`가 첫 NUL에서
멈추기 때문입니다.

## 프로세스

| 함수 | 하는 일 |
|---|---|
| `exec(cmd, args)` | 실행하고 기다린 뒤 종료 코드 반환 |
| `capture(cmd, args)` | 실행하고 표준 출력을 반환 |
| `env(name)` | 환경 변수, 없으면 `""` |
| `cwd()` | 작업 디렉터리 |
| `chdir(path)` | 작업 디렉터리 변경 |

중간에 셸이 없습니다. `args`의 각 원소는 어떻게 쓰든 인자 하나입니다.

```maca
exec("cp", ["내 메모.txt", dest])    // 파일 하나, 둘이 아님
exec("echo", ["$HOME"])              // $HOME을 그대로 출력, 확장하지 않음
```

`exec`는 셸처럼 `PATH`를 찾고, 없는 프로그램은 `127`로 끝납니다.

`std/proc`이 그 위에 `run`, `try_run`, `run_in`, `output`, `which`/`have`,
`env_or`를 얹습니다.

## 표준 입력

| 함수 | 하는 일 |
|---|---|
| `read_line()` | 한 줄, 개행 제거 |
| `at_eof()` | 입력이 끝났는가? |
| `read_stdin()` | 전부 |

빈 줄과 입력의 끝이 둘 다 빈 문자열로 읽히기 때문에 `at_eof`가 있습니다.

```maca
while !at_eof() {
    line = read_line()
    info(line.upper())
}
```

## 시간

| 함수 | 하는 일 |
|---|---|
| `now_ms()` | Unix 에포크 이후 밀리초 |
| `now_iso()` | `"YYYY-MM-DDTHH:MM:SSZ"` |
| `format_time(ms, fmt)` | 그 시각에 대한 `strftime` |

전부 UTC입니다.

## 동시성

| 형태 | 하는 일 |
|---|---|
| `spawn f(x)` | 동시 실행, `Future a`를 줌 |
| `await fut` | 기다림, `a`를 줌 |
| `sleep_ms(n)` | 중단 |

[이펙트와 async](a7-effects.md)를 보세요.

## 마크업

| 형태 | 하는 일 |
|---|---|
| `div(class="x", child)` | 요소: 이름 있는 인자는 속성, 위치 인자는 자식 |
| `data-tomo="x"` | 붙여 쓴 `-` 는 이름의 일부, 띄어 쓴 것은 뺄셈 |
| `open=true` | bool은 속성의 존재 여부를 결정 |
| `element(tag, …)` | 같은 것, 태그를 값으로 |
| `styles()` | 이 모듈이 쓴 유틸리티 클래스의 CSS |

[UI 문법](a11-ui.md)을 보세요.

## JSON

`import std/json`은 두 부분을 가져옵니다. `encode`와 `decode`는 타입이 붙은
쌍이고, 나머지는 어떤 타입도 설명하지 않는 모양을 위해 JSON을 텍스트로 읽고
씁니다.

```maca
import std/json

Layout = List | Grid
Link   = { title: str, url: str }
Config = { columns: int, layout: Layout, links: Link[] }

save(c: Config) -> unit => write_file("conf.json", encode(c))

load(text: str) -> Config {
    c: Config = decode(text)
    c
}
```

`encode(value)`는 값의 정적 타입을 보고 그에 맞는 JSON을 씁니다. 레코드는 필드
하나에 멤버 하나인 객체가 되고, 순서는 **레코드가 선언한 순서** 그대로입니다.

`decode(text)`는 바인딩이 말하는 타입으로 읽으므로 타입을 반드시 적어야 합니다.
맨 `decode(text)`는 그렇게 말하는 빌드 오류입니다.

### 합 타입이 매핑되는 방식

**변형은 자기 이름의 소문자입니다.** `Layout = List | Grid`는 `"list"`와
`"grid"`로 저장되고 같은 방식으로 다시 읽힙니다.

페이로드를 가진 변형은 이름 말고는 JSON 형태가 없습니다. JSON을 왕복하는
타입은 열거형으로 두고 데이터는 레코드 필드에 담으십시오.

### 텍스트가 타입과 맞지 않을 때 decode가 하는 말

실패하고, 메시지가 필드 이름을 말합니다. `try`가 잡습니다([오류](09-errors.md)).

```maca
why = try load(text)
if why != "" {
    warn("bad config: {why}")
}
```

| 텍스트 | 메시지 |
|---|---|
| `{"columns": "three", …}` | ``field `columns`: expected a number, got a string`` |
| `columns`가 없는 `{"layout": "grid", …}` | ``field `columns`: expected a number, and the object has no such field`` |
| `{"layout": "table", …}` | ``field `layout`: "table" is not one of list, grid`` |
| `[1, 2, 3]` | ``` `Config`: expected an object, got a list ``` |

중첩된 레코드나 리스트 원소 안의 필드는 자기 이름으로 보고합니다.

### 텍스트 쪽

| 함수 | 하는 일 |
|---|---|
| `quote(s)` | `s`를 JSON 문자열 리터럴로 |
| `array_of_str(xs)` `array_of_int(xs)` | 리스트로 배열 만들기 |
| `object_of(keys, values)` | 병렬 리스트로 객체 만들기 |
| `get(src, key)` | 멤버의 원본 텍스트, 없으면 `""` |
| `get_int(src, key, dflt)` `get_bool(src, key)` | 멤버 하나 읽기 |
| `items(src)` | 배열의 원소들, 각각 원본 텍스트로 |

## 에러

| 형태 | 하는 일 |
|---|---|
| `fail "message"` | 발생 |
| `x?` | 호출자에게 실패 전파 |
| `try e` | 잡기 |

[오류](09-errors.md)를 보세요.

## 단언

| 함수 | 하는 일 |
|---|---|
| `assert(cond, msg)` | `cond`가 거짓이면 `msg`를 보고 |
| `assert_eq(got, want, msg)` | 다르면 양쪽을 보고 |
| `failures()` | 실패한 단언의 개수 |

실패한 단언은 보고하고 계속 갑니다. `failures()`가 테스트 함수가 반환하는
수입니다([테스트](12-testing.md)).

## 정규식

없습니다. `contains`, `starts_with`, `ends_with`, `index_of`, `split`과 문자
분류가 실제로 손을 뻗는 범위를 덮습니다.
