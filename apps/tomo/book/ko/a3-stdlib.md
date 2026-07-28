# 부록 C: 표준 라이브러리

Maca의 "표준 라이브러리"는 대부분 Maca 소스가 아니라 컴파일러와 런타임의
빌트인입니다. 덕분에 모든 타깃에서 쓸 수 있습니다. 같은 `xs.map(f)`가 네이티브
바이너리, 브라우저 플레이그라운드, JVM 출력에서 똑같이 동작합니다.

## 출력

| 함수 | 하는 일 |
|---|---|
| `print(s)` | stdout에 개행 없이 씀 |
| `info(s)` | stdout에 한 줄 씀 |
| `err(s)` | stderr에 한 줄 씀 |

이름은 syslog 레벨에서 왔습니다. `warn` 이하는 stderr로 갑니다.

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
| `push(x)` `pop()` | 새 리스트 |
| `slice(from, to)` | `to`는 **제외** |
| `contains(x)` `index_of(x)` | 검색 |
| `sum()` `min()` `max()` | 수치 |
| `first()` `last()` `get(i)` | 원소 |
| `length()` | `int` |
| `join(sep)` | `str[]`를 하나의 `str`로 |
| `parallel(f)` | `map`과 같되 동시 실행 |

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

`is_dir`은 없습니다. 평범한 파일에 대한 `list_dir`이 아무것도 돌려주지 않는
것이 둘을 구분하는 통상적인 방법입니다.

## 동시성

| 형태 | 하는 일 |
|---|---|
| `spawn f(x)` | 동시 실행, `Future a`를 줌 |
| `await fut` | 기다림, `a`를 줌 |
| `sleep_ms(n)` | 중단 |

`async` 키워드는 없습니다. 13장을 보세요.

## 마크업

| 형태 | 하는 일 |
|---|---|
| `div(class="x", child)` | 요소 — 이름 있는 인자는 속성, 위치 인자는 자식 |
| `data-tomo="x"` | 붙여 쓴 `-` 는 이름의 일부, 띄어 쓴 것은 뺄셈 |
| `open=true` | bool은 속성의 존재 여부를 결정 |
| `element(tag, …)` | 같은 것, 태그를 값으로 |
| `styles()` | 이 모듈이 쓴 유틸리티 클래스의 CSS |

15장을 보세요.

## 에러

| 형태 | 하는 일 |
|---|---|
| `fail "message"` | 발생 |
| `x?` | 호출자에게 실패 전파 |
| `try e` | 잡기 |

9장을 보세요.

## 없는 것

계획을 세울 수 있도록 솔직히 적습니다.

- **해시 맵이 없습니다.** 레코드의 리스트와 선형 탐색이 현재의 답이고,
  `examples/wordcount.maca`가 그렇게 합니다.
- **`is_dir`도, 파일 메타데이터도, 삭제도 없습니다.**
- **stdin이 없습니다.** 프로그램은 인자를 받고 파일을 읽습니다.
- **날짜/시간이 없습니다.**
- **정규식이 없습니다.**
- **단언 라이브러리가 없습니다.** 테스트는 `0` 또는 0이 아닌 값을 반환합니다(12장).
- **문자열 `slice`가 없습니다.** 대신 `substr`가 길이를 받습니다(호출하면 링커
  에러가 아니라 깔끔한 진단이 납니다).

이것들이 언제 들어올지는 대체로 다음에 무엇이 Maca로 만들어지느냐의 문제입니다.
특히 해시 맵은 컴파일러에서 필요한 것이 없으므로 첫 기여로 좋습니다.
