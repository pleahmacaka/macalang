# 합타입과 매칭

레코드가 *이것들 전부 동시에*라면, 합타입은 *정확히 이것들 중 하나*입니다.

## 선언

변형들은 `|`로 구분합니다.

```maca
Color = Red | Green | Blue
```

변형은 그 타입의 값입니다.

```maca
c = Green
```

변형은 데이터를 실을 수 있고, 호출처럼 씁니다.

```maca
Shape = Circle(int) | Rect(int, int)

s = Circle(2)
t = Rect(3, 4)
```

`enum`도 `struct`도 없습니다. 타입 선언은 `Name = …`이고, 중괄호면 레코드,
세로줄이면 합타입입니다.

## 매칭

`match`가 값을 분해합니다.

```maca
area(s: Shape) -> int =>
    match s {
        Circle(r)  => 3 * r * r
        Rect(w, h) => w * h
    }
```

각 갈래는 `패턴 => 식`이고, 변형이 실은 데이터는 패턴에서 이름을 붙여
바인딩합니다. `match`는 **식**입니다.

## 빠짐없음 검사

컴파일러는 `match`가 모든 변형을 다루는지 검사합니다. 하나를 빼면

```maca
name(c: Color) -> str =>
    match c {
        Red   => "red"
        Green => "green"
    }
```

런타임의 놀라움이 아니라 진단 메시지를 받습니다.

```
NonExhaustive: match on `Color` is not exhaustive; missing: Blue
```

1년 뒤에 `Color`에 변형을 추가하면 컴파일러가 구멍이 생긴 모든 `match`로
데려다 줍니다.

포괄 패턴 `_`는 무엇에나 매치되므로, "나머지 전부, 앞으로도 영원히"를 뜻할
때만 쓰세요.

```maca
is_red(c: Color) -> bool =>
    match c {
        Red => true
        _   => false
    }
```

## 리스트 매칭

리스트도 모양으로 분해할 수 있습니다.

```maca
describe(xs: int[]) -> str =>
    match xs {
        []          => "empty"
        [x]         => "one: {x}"
        [x, ..rest] => "head {x}, then {rest.length()} more"
    }
```

`[]`는 빈 리스트, `[x]`는 정확히 하나짜리, `[x, ..rest]`는 머리와 나머지에
매치됩니다. 모호하지 않을 때는 대괄호를 생략해 `x, ..rest`로 쓸 수 있습니다.

## 재귀 합타입

변형은 선언 중인 그 타입을 실을 수 있습니다.

```maca
Tree = Leaf | Node(int, Tree, Tree)

sum(t: Tree) -> int =>
    match t {
        Leaf          => 0
        Node(v, l, r) => v + sum(l) + sum(r)
    }
```

실린 값은 박싱되므로 타입의 크기가 무한해지지 않습니다.
`apps/selfhost/ast.maca`가 이것으로 만들어져 있습니다.

## 레코드와 합타입 중 무엇을 고를까

필드들이 동시적인지 택일적인지를 물으세요. 사용자는 이름 *그리고* 이메일을
가집니다. 레코드입니다. 결제는 카드 *또는* 이체 *또는* 청구서입니다.
합타입입니다.

둘은 자유롭게 중첩됩니다.

```maca
Status = Pending | Done(str) | Failed(str)

Job = {
    id: int
    status: Status
}
```

이제 `Job`은 완료이면서 동시에 실패일 수 없고, 결과 없이 완료일 수도 없습니다.

## 실행해 보기

```
maca run apps/examples/tree.maca
```

이진 트리를 만들고, 더하고, 출력합니다. 그다음 `match`에서 갈래 하나를 지우고
다시 빌드해 보세요.
