# 컬렉션

Maca에는 기본 컬렉션이 둘 있습니다. 리스트와 문자열입니다. 둘 다 메서드
라이브러리를 가지고 있고, 둘 다 UFCS로 쓰이므로 `xs.map(f)`는 왼쪽에서
오른쪽으로 읽히게 차려입은 평범한 함수 적용입니다.

## 리스트

`T`의 리스트는 `T[]`로 씁니다.

```maca
xs = [5, 3, 8, 1]
names = ["ada", "grace"]
empty = []
```

인덱싱과 인덱스를 통한 대입입니다.

```maca
first = xs[0]
xs[2] = 99
```

크기는 `len(xs)`와 `xs.length()` 둘 다 줍니다.

### 리스트 메서드

| 메서드 | 결과 | 비고 |
|---|---|---|
| `map(f)` | `U[]` | `f`가 각 원소를 받음 |
| `filter(f)` | `T[]` | `f`가 참인 원소만 남김 |
| `reduce(init, f)` | `U` | `f(acc, x)`, 왼쪽부터 |
| `fold(init, f)` | `U` | 같음 |
| `sort()` | `T[]` | 오름차순 |
| `sort_by(key)` | `T[]` | `key(x)`(`int`/`float`/`str`) 기준 오름차순 |
| `reverse()` | `T[]` | |
| `push(x)` | `T[]` | 더 긴 리스트 |
| `pop()` | `T[]` | 더 짧은 리스트 |
| `set(i, x)` | `T[]` | `i`번째를 바꾼 리스트 |
| `insert(i, x)` | `T[]` | `i`에 `x`를 넣고 나머지를 밀어냄 |
| `remove(i)` | `T[]` | `i`번째를 빼고 빈자리를 메움 |
| `slice(from, to)` | `T[]` | `to`는 **제외** |
| `contains(x)` | `bool` | |
| `index_of(x)` | `int` | 없으면 `-1` |
| `index_of_by(f)` | `int` | `f(x)`가 참인 첫 `x`, 없으면 `-1` |
| `enumerate()` | `{index, value}[]` | 각 원소와 그 위치 |
| `sum()` `min()` `max()` | `T` | 수치 리스트 |
| `first()` `last()` | `T` | |
| `get(i)` | `T` | `xs[i]`와 같음 |
| `length()` | `int` | |
| `join(sep)` | `str` | `str[]`에만 |
| `parallel(f)` | `U[]` | `map`과 같되 동시 실행 |

```maca
xs = [5, 3, 8, 1]
info("{xs.map(v => v * 2).sum()}")            // 34
info("{xs.filter(v => v > 3).length()}")      // 2
info("{xs.reduce(0, (a, b) => a + b)}")       // 17
info("{xs.sort().first()}")                   // 1
```

리스트를 "바꾸는" 메서드들은 새 리스트를 돌려주고 수신자는 건드리지 않습니다.

```maca
ys = [3, 1, 2]
sorted = ys.sort()
// ys.first()는 여전히 3, sorted.first()는 1
```

이건 몸에 익혀둘 가치가 있습니다. `xs.push(9)`는 `xs`를 늘리는 문장이 아니라,
더 긴 리스트를 값으로 갖는 식입니다. 편집한 리스트를 돌려주는 나머지 셋,
`set`, `insert`, `remove`도 마찬가지입니다. 왜 이렇게 쓰는 것이 보기와 달리
성능 실수가 아닌지는 [메모리](04-memory.md)가 설명합니다.

리스트에 없는 인덱스는 아무 일도 일으키지 않습니다. `xs.set(9, 0)`과
`xs.remove(-1)`은 그대로인 리스트이고, `xs.insert(99, 0)`은 값을 맨 뒤에
놓습니다. `get`과 `slice`가 이미 지키는 그 자르기 규칙 그대로입니다.

`sort_by`는 원소가 아니라 키로 정렬하며, 안정적입니다. 키가 같은 원소들은
원래 순서를 지킵니다.

```maca
ws = ["bb", "a", "cc", "d"]
info(ws.sort_by(w => w.length()).join(","))   // a,d,bb,cc
```

`index_of_by`는 값 대신 질문을 받는 `index_of`이고, `enumerate()`는 각 원소를
그 위치와 짝지어 줍니다. 둘 다 필요한 반복문이 보통 찾던 것이 이것입니다.

```maca
for e in ["a", "b"].enumerate() {
    info("{e.index}: {e.value}")              // 0: a, 그다음 1: b
}
```

`slice`는 시작과 **제외되는** 끝을 받으므로 `xs.slice(1, 3)`은 원소 두 개입니다.

```maca
xs = [10, 20, 30, 40, 50]
xs.slice(1, 3)      // [20, 30]
```

### 범위

`lo..hi`는 반열린 정수 범위이고, `int[]`입니다. `lo`부터 `hi` 직전까지이므로
`0..xs.length()`가 곧 `xs`의 인덱스 전부이고, `- 1`을 기억할 필요가 없습니다.

```maca
for i in 0..5 {
    info("{i}")     // 0 1 2 3 4
}

xs = "a", "b", "c"
for i in 0..xs.length() {
    info("{i}: {xs[i]}")
}

info("{(1..101).sum()}")    // 5050. 101은 포함되지 않습니다
```

`for` 헤더 안에서는 세는 루프로 낮춰집니다. 리스트가 만들어지지 않습니다.

### 이름 붙은 함수 넘기기

함수를 받는 메서드는 람다도, 최상위 함수의 이름도 받습니다.

```maca
is_even(n: int) -> bool => n % 2 == 0

evens = [1, 2, 3, 4].filter(is_even)
```

함수 값이 실제로 무엇인지는 [클로저와 제어 흐름](11-closures.md)에서 다룹니다.

## 문자열

`str`는 바이트 문자열입니다. 메서드는 이렇습니다.

| 메서드 | 결과 | 비고 |
|---|---|---|
| `length()` | `int` | 문자가 아니라 바이트 |
| `split(sep)` | `str[]` | |
| `trim()` | `str` | 양끝 |
| `upper()` `lower()` | `str` | |
| `contains(s)` | `bool` | |
| `starts_with(s)` `ends_with(s)` | `bool` | |
| `replace(from, to)` | `str` | 일치하는 곳 전부 |
| `substr(start, len)` | `str` | 끝이 아니라 **길이** |
| `slice(from, to)` | `str` | `to`는 **제외**, 리스트와 동일 |
| `index_of(s)` | `int` | 없으면 `-1` |
| `repeat(n)` | `str` | |
| `pad_start(w, p)` `pad_end(w, p)` | `str` | `p`의 기본값은 공백 |
| `pad_center(w, p)` | `str` | |
| `chars()` | `str[]` | 한 글자짜리 문자열들 |
| `at(i)` | `str` | `i` 위치의 글자 |
| `is_whitespace()` `is_ascii_digit()` `is_alpha()` | `bool` | 문자 분류 |

문자열에는 둘 다 있고, 서로 다른 뜻입니다. **`slice`는 제외되는 끝을 받고,
`substr`는 길이를 받습니다.** 이름이 리스트와 같듯 규약도 같습니다.

```maca
"abcdef".slice(1, 3)      // "bc"  (인덱스 3 앞까지)
"abcdef".substr(1, 3)     // "bcd" (인덱스 1부터 세 글자)
```

`chars`, `at`, 그리고 세 가지 문자 분류가 스캐너를 만드는 재료입니다.
`apps/selfhost/lexer.maca`는 이것 말고는 아무것도 쓰지 않습니다.

```maca
run_digits(cs: str[], i: int) -> int =>
    i >= cs.length() || !cs.get(i).is_ascii_digit()
        ? i
        : run_digits(cs, i + 1)
```

### 오타 난 메서드는 링커가 아니라 검사기가 잡습니다

메서드 호출은 그 외에는 점진적입니다. `any` 수신자는 검사기가 볼 수 없는 외부
코드에 닿으니까요. 하지만 `str`나 `T[]`의 메서드 집합은 닫혀 있으므로, 거기에
없는 이름은 오타입니다. 가까운 것이 있으면 제안도 해 줍니다.

```
UndefinedName: `str` has no method `lenght`; did you mean `length`?
```

## 문자열과 문자

Maca에는 문자 타입이 없습니다. `at(i)`가 한 글자짜리 `str`를 주고, 비교는
기대하는 대로 동작합니다.

```maca
c = "hello".at(1)
info("{c == "e"}")      // true
```

`length`가 바이트를 세므로, 비ASCII 텍스트가 든 문자열의 length는 글자 수보다
큽니다. 보간, 이어붙이기, 비교는 모두 바이트 단위로 정확하고 안전합니다. 다중
바이트 텍스트에서 주의가 필요한 것은 인덱싱뿐입니다.

## 실행해 보기

```
maca run apps/examples/collections.maca
```

위에 나온 리스트 메서드를 하나씩 적용하고 출력합니다. 이 장의 표 두 개는
일상적으로 쓰는 부분집합입니다. *닫혀 있고* 검사되는 전체 메서드 집합은
[표준 라이브러리](a3-stdlib.md)에 있습니다.
