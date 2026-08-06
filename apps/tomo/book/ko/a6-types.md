# 타입 시스템

Maca는 정적 타입이고 대부분 추론됩니다. 함수 경계에만 표기하면 안쪽은
따라옵니다.

## 경계에서의 추론

함수 시그니처가 계약입니다.

```maca
double(n: int) -> int =>
    n * 2
```

지역 변수에는 절대 표기하지 않습니다.

```maca
half(n: int) -> int {
    m = n / 2      // m은 int, 선언할 것 없음
    m
}
```

반환 타입은 생략할 수 있고, 그러면 본문에서 추론됩니다.

```maca
inc(x) => x + 1
```

그래도 쓰는 편이 낫습니다. 반대할 대상이 있을 때 에러 메시지가 훨씬 좋아집니다.

추론이 붙잡을 것이 없는 자리에는 지역 변수도 표기를 답니다.

```maca
counts: Map str int = map()
```

## 꺾쇠 없는 제네릭

타입 자리의 **소문자** 이름은 타입 변수입니다.

```maca
identity(x: a) -> a =>
    x

pair_first(xs: a[], fallback: a) -> a =>
    xs.length() > 0 ? xs.first() : fallback
```

`a`는 "아무 타입, 세 자리 모두 같은 것"입니다. 선언할 `<T>`가 없습니다.

함수 시그니처는 스킴으로 일반화되고 호출마다 새로 인스턴스화되므로,
`identity(1)`과 `identity("x")`가 둘 다 됩니다. 표준적인 Hindley-Milner
추론입니다.

C 백엔드는 **단형화**합니다. 런타임 박싱도 디스패치도 없어서, 제네릭 함수는
손으로 쓴 특수화 버전과 정확히 같은 비용이 듭니다.

## 점진적 타이핑과 `any`

외부 함수, 표준 라이브러리의 일부 구석, 백엔드 경계를 넘는 값에는 줄 수 있는
Maca 타입이 없습니다. 그런 것들을 위해 `any`가 있고, 모든 것과 통일됩니다.

`any`는 가운데 뚫린 구멍이 아니라 가장자리의 탈출구입니다. 미지의 영역으로
들어가는 호출이 가짜 에러의 폭포를 만들지 않는다는 뜻이자, 그 영역의 실수는
잡히지 않는다는 뜻입니다.

`str`와 `T[]`의 메서드 집합은 **닫혀 있으므로** 거기에 없는 이름은 오타이고
제안과 함께 보고됩니다. `any` 수신자는 점진적으로 남습니다
([표준 라이브러리](a3-stdlib.md)).

## 레코드 타입과 레코드 리터럴

`Point = { x: int, y: int }`는 타입에 이름을 붙이고, `{ x = 5, y = 6 }`은 이름
없이 하나를 씁니다. 문맥이 이름을 말하는 곳에서는 리터럴이 그 이름 있는 레코드가
*됩니다*.

```maca
Point = { x: int, y: int }

origin: Point = { x = 0, y = 0 }
mk() -> Point => { x = 1, y = 2 }
far(p: Point) -> int => p.x + p.y
```

타입 표기, 반환 타입, 파라미터, 다른 레코드의 필드, 그리고 `Point[]`의 원소가
모두 이름을 말하는 곳입니다. 이름 있는 타입이 되면 레코드 자신의 구조체가
만들어지고, `with`와 선언된 필드의 타입과 오버로드된 연산자가 그 값에서
동작합니다.

레코드 리터럴은 그렇지 않으면 **열려** 있습니다. 여기서 열어 두면 아무도 쓰지
않은 필드가 조용히 0이 되므로, 이름 있는 레코드에 쓰인 리터럴은 선언한 모든
필드를 적어야 하고 선언하지 않은 필드는 하나도 없어야 합니다.

```
TypeMismatch: in `p`: record is missing field `y`
TypeMismatch: in `p`: record has unexpected field `z`
```

그런 문맥이 없으면 리터럴은 구조적으로 남고, 같은 모양의 리터럴 둘은 한
타입입니다.

## 이펙트

검사기는 함수가 무엇을 *하는지*도 추적하며, 이펙트는 추론될 뿐 선언되지
않습니다([이펙트와 async](a7-effects.md)).

## 진단 메시지

| 진단 | 의미 |
|---|---|
| `TypeMismatch` | 같아야 할 두 타입이 같지 않음 |
| `NonExhaustive` | `match`가 모든 변형을 다루지 않음 |
| `Immutable` | 상수에 대한 대입 |
| `UndefinedName` | 어디에도 정의되지 않은 이름 호출 |
| `UnknownOption` | 존재하지 않는 설정 옵션 |
| `EffectInConfig` | config 모드의 순수하지 않은 연산 |

`TypeMismatch`는 이름보다 넓습니다. 호출 인자 개수도 불일치입니다.

```
TypeMismatch: call to `f` expects 2 argument(s), got 3
```

`if`나 삼항의 갈래가 서로 다른 것도요.

```maca
x = c ? 1 : "two"
// TypeMismatch: ternary branches disagree: expected int, found str
```

모든 메시지와 대처법은 [진단 메시지](a4-diagnostics.md)에 있습니다.

## 상수

바인딩은 기본적으로 가변입니다.

```maca
count = 0
count = count + 1
```

셋 중 하나가 상수로 만듭니다. `const`, 뒤에 붙는 `as const`, 그리고 대문자로
시작하는 이름입니다.

```maca
const Limit = 100
step = 5 as const
Origin = 0
```

어느 것이든 재대입하면 컴파일 에러입니다.

```
Immutable: cannot reassign constant `Limit`; declare it mutable with
`Limit = …` (no `const`)
```

대문자 규칙은 암묵적이라서 `maca lint`가 `const`를 명시하도록 권합니다.

## 에러 읽기

타입 에러는 함수와 위치를 알려줍니다.

```
TypeMismatch: in call to `d` (argument 2): type mismatch: expected P, found int
```

메시지가 구체 타입이 아니라 타입 변수에 관한 것이라면, 보통 위쪽 어딘가에
표기가 빠진 것이 원인입니다.
