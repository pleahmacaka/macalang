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
| `reverse()` | `T[]` | |
| `push(x)` | `T[]` | 더 긴 리스트 |
| `pop()` | `T[]` | 더 짧은 리스트 |
| `slice(from, to)` | `T[]` | `to`는 **제외** |
| `contains(x)` | `bool` | |
| `index_of(x)` | `int` | 없으면 `-1` |
| `sum()` `min()` `max()` | `T` | 수치 리스트 |
| `first()` `last()` | `T` | |
| `get(i)` | `T` | `xs[i]`와 같음 |
| `length()` | `int` | |

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
더 긴 리스트를 값으로 갖는 식입니다. 왜 이렇게 쓰는 것이 보기와 달리 성능
실수가 아닌지는 4장이 설명합니다.

`slice`는 시작과 **제외되는** 끝을 받으므로 `xs.slice(1, 3)`은 원소 두 개입니다.

```maca
xs = [10, 20, 30, 40, 50]
xs.slice(1, 3)      // [20, 30]
```

### 범위

`lo..hi`는 양끝을 포함하는 정수 범위이고, `int[]`입니다.

```maca
for i in 1..5 {
    info("{i}")     // 1 2 3 4 5
}
info("{(1..100).sum()}")    // 5050
```

`for` 헤더 안에서는 세는 루프로 낮춰집니다. 리스트가 만들어지지 않습니다.

### 이름 붙은 함수 넘기기

함수를 받는 메서드는 람다도, 최상위 함수의 이름도 받습니다.

```maca
is_even(n: int) -> bool => n % 2 == 0

evens = [1, 2, 3, 4].filter(is_even)
```

함수 값이 실제로 무엇인지는 11장에서 다룹니다.

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
| `replace(from, to)` | `str` | 모든 occurrence |
| `substr(start, len)` | `str` | 끝이 아니라 **길이** |
| `index_of(s)` | `int` | 없으면 `-1` |
| `repeat(n)` | `str` | |
| `pad_start(w, p)` `pad_end(w, p)` | `str` | `p`의 기본값은 공백 |
| `pad_center(w, p)` | `str` | |
| `chars()` | `str[]` | 한 글자짜리 문자열들 |
| `at(i)` | `str` | `i` 위치의 글자 |
| `is_whitespace()` `is_ascii_digit()` `is_alpha()` | `bool` | 문자 분류 |

한 번은 걸려 넘어질 비대칭을 눈여겨보세요. **리스트의 `slice`는 끝을 받고,
문자열의 `substr`는 길이를 받습니다.**

```maca
"abcdef".substr(1, 3)     // "bcd"
```

`chars`, `at`, 그리고 세 가지 문자 분류가 스캐너를 만드는 재료입니다.
`selfhost/lexer.maca`는 이것 말고는 아무것도 쓰지 않습니다.

```maca
run_digits(cs: str[], i: int) -> int =>
    i >= cs.length() || !cs.get(i).is_ascii_digit()
        ? i
        : run_digits(cs, i + 1)
```

### 문자열에는 slice가 없습니다

`str`에는 `slice`가 아니라 `substr`가 있습니다. 문자열에 `slice`를 호출해도 타입
에러가 나지 않습니다. 메서드 호출은 점진적으로 남아 있어서 검사기가 통과시키고,
C 컴파일러가 `slice`에 대한 undefined reference를 보고합니다. 알려진 거친
모서리입니다. 기본 타입에 대한 오타난 메서드는 진단 메시지여야 하는데, 지금은
링커 메시지입니다.

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
