# 클로저와 제어 흐름

함수는 값입니다. 넘길 수도, 돌려줄 수도, 그 자리에서 쓸 수도 있습니다.

## 함수는 값이다

이름으로 참조한 최상위 함수는 값이고, 함수를 기대하는 자리에 그대로 넘길 수
있습니다.

```maca
is_even(n: int) -> bool => n % 2 == 0
evens = [1, 2, 3, 4].filter(is_even)   // [2, 4]
```

람다는 주변 스코프를 캡처합니다.

```maca
step = 10
bumped = [1, 2, 3].map(n => n + step)  // [11, 12, 13]
```

주석이 없는 파라미터가 본문에서 *호출*되면 함수로 추론되므로, 고차 함수 코드에
특별한 문법이 필요 없습니다.

```maca
run_twice(f, x) => f(f(x))
run_twice(n => n + 1, 40)              // 42
```

람다는 반환 타입을 본문에서 추론하고, 직접 선언할 수도 있습니다.

```maca
inc = (n) -> int => n + 1
```

어노테이션을 쓸 때는 파라미터를 괄호로 묶으세요. 파라미터가 하나면 어느 쪽이든
되지만 둘이면 안 됩니다. 어노테이션이 필요한 곳은 Rust trait 구현 한
군데뿐이고, [타깃](a10-targets.md)에 있습니다.

람다 본문은 `match` 갈래와 마찬가지로 블록이 될 수 있습니다.

```maca
classify = (n) -> str => {
    doubled = n * 2
    doubled > 10 ? "big" : "small"
}
```

그래서 `=>` 뒤의 맨 `{ … }`는 익명 레코드가 아니라 그 블록입니다. 레코드를
뜻했다면 괄호로 감싸세요. `(n) => ({ x = n })`.

## 식으로서의 `if`

`if`/`else`는 식입니다.

```maca
label = if score >= 60 { "합격" } else { "불합격" }
```

두 갈래 선택에는 공백을 띄운 삼항 연산자가 있습니다.

```maca
label = score >= 60 ? "합격" : "불합격"
```

## `match`

`match`는 합타입, 리터럴, 리스트에 모두 동작하며 빠짐없이 다뤄야 합니다.

```maca
describe(xs: int[]) -> str =>
    match xs {
        []          => "비어 있음"
        [x]         => "하나: {x}"
        [x, ..rest] => "머리 {x}, 이어서 {rest.length()}개 더"
    }
```

대괄호는 선택이고 `x, ..rest`도 똑같이 매칭됩니다.

## 반복

`while`과 `for`는 문입니다. `for`는 리스트나 `..`로 쓴 정수 범위를 순회합니다.

```maca
sum_to(n: int) -> int {
    total = 0
    for i in 0..n {          // 0, 1, …, n - 1
        total = total + i
    }
    total
}
```

`while`은 조건을 받고, 카운터는 직접 움직입니다.

```maca
countdown(start: int) -> int {
    n = start
    while n > 0 {
        info("{n}")
        n = n - 1
    }
    0
}
```

`break`와 `continue`는 예상대로 동작합니다. Maca의 관용구는 명시적 반복문보다
재귀와 리스트 메서드(`map`/`filter`/`reduce`)에 기웁니다.

## `return`으로 일찍 떠나기

함수의 마지막 식이 곧 그 함수의 값입니다. 하지만 함수 맨 위의 가드는 *멈추기*를
원하고, `return`이 그렇게 말하는 방법입니다.

```maca
save(title: str) -> str {
    trimmed = title.trim()

    if trimmed == "" {
        return "title is required"
    }

    // 나머지. `else` 밑으로 들여쓰지 않습니다
    store(trimmed)
    "saved"
}
```

`return e`는 결과를 선언한 함수를 떠나고, `e`는 그 결과 타입에 대해 검사됩니다.
인자 없는 `return`은 결과를 선언하지 않은 함수를 떠납니다.

```maca
log_unless(quiet: bool, line: str) {
    if quiet {
        return
    }

    info(line)
}
```

`return`은 **문**입니다. 한 줄에 홀로 서거나, `if`/`match`/`for`/`while` 갈래의
꼬리에 섭니다. 값이 필요한 자리에 쓰면 컴파일러가 이름을 대며 거절합니다.

```maca
label = ok ? return 1 : 2     // 거절: 이 `return`은 식 안에 서 있습니다
```

람다의 본문은 *곧* 그 값이므로 값을 쓰세요. 이름 있는 함수에서는 `return`을,
람다에서는 값을 씁니다.

## 함수 안의 함수

블록은 함수를 정의할 수 있고, 그 함수는 자기를 둘러싼 스코프를 읽고 *쓸* 수
있습니다.

```maca
board() -> str {
    held = 0
    moves = 0

    grab(section: int) {
        if section < 0 {
            return
        }

        held = section
        moves = moves + 1
    }

    release() {
        held = 0
    }

    grab(4)
    release()

    "held={held} after {moves} move(s)"
}
```

중첩 정의가 하나라도 대입하는 지역 변수는 그들 사이에서 *공유*됩니다. 아무도
대입하지 않는 변수는 정의가 만들어질 때 복사됩니다.

중첩 정의는 블록 안의 다른 모든 바인딩과 같은 규칙을 따릅니다. **그 줄부터
스코프에 있고, 그 앞에는 없습니다.** 여기서 두 가지가 따라 나오고, 각각 전용
진단이 있습니다.

```maca
main() -> int {
    go() -> int {
        return go()          // 거절: 중첩 함수는 자기 이름을 부를 수 없습니다
    }

    first() -> int => second()
    second() -> int => 1     // 거절: `second`는 더 아래에 정의되어 있습니다
    first()
}
```

둘 중 하나가 필요하면 함수를 최상위로 올리세요. 최상위에서는 모든 함수가
어디서나 스코프에 있습니다.

값이므로 중첩 정의는 넘길 수 있고, `(T) -> R`로 선언된 레코드 필드에 담을 수
있고, 호출자에게 돌려줄 수 있습니다. 캡처한 것은 닿을 수 있는 동안 힙에
남습니다.

```maca
Knob = { read: (int) -> int, write: (int) -> int }

knob(start: int) -> Knob {
    level = start

    get(ignored: int) -> int => level

    set(to: int) -> int {
        level = to

        return level
    }

    Knob { read = get, write = set }
}
```

네이티브 C와 JS 백엔드가 이것을 낮춥니다. `rust`, `jvm`, `embedded` 타깃은
이름을 대며 거절하고, 그 이유는 [타깃](a10-targets.md)에 있습니다.

## 오류 전파

실패할 수 있는 호출은 호출 지점에 `?`를 붙여 표시합니다.

```maca
config = read_file("app.toml")?      // 실패를 호출자에게 전파
```

## 실행해 보기

```
maca run apps/examples/lambda.maca
```

리스트 메서드에 넘긴 람다, 값으로 쓴 이름 붙은 함수, 지역 변수를 캡처한 클로저.
셋 다 코드 포인터와 힙 환경으로 컴파일되고, 그래서 호출 지점에서 서로 바꿔 쓸
수 있습니다.
