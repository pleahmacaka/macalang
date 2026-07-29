# 부록 D: 진단 메시지

검사기가 내보내는 모든 진단, 그 의미, 그리고 대처법입니다.

## TypeMismatch

같아야 할 두 타입이 같지 않았습니다. 여섯 중 가장 넓은데, 서로 다른 여러 실수가
여기로 모이기 때문입니다.

**인자 타입**

```
TypeMismatch: in call to `d` (argument 2): type mismatch: expected P, found int
```

**인자 개수** — 개수도 타입의 성질이라 여기로 옵니다.

```
TypeMismatch: call to `f` expects 2 argument(s), got 3
```

**어긋나는 갈래** — 타입이 다른 `if`나 삼항입니다.

```
TypeMismatch: ternary branches disagree: type mismatch: expected int, found str
```

이건 흔한 실수를 잡습니다. `if`와 `match`는 식이므로, 전체를 문장처럼 다루고
있었더라도 갈래들의 타입이 맞아야 합니다. `c ? continue : 0`이 이 이유로
실패합니다. `continue`에는 값이 없습니다.

**레코드 필드** — 리터럴은 레코드가 선언한 모든 필드를 적어야 하고, 선언하지
않은 필드는 적으면 안 됩니다.

```
TypeMismatch: `Config` is missing field(s): title, port
TypeMismatch: `Config` has no field `titel`; did you mean `title`?
```

빠뜨린 필드는 조용한 0이 됩니다. `str`이면 `""`, `int`면 `0`입니다. 오타는 더
나쁩니다. 값은 아무 데도 가지 않고, 원래 가려던 필드는 빈 채로 남습니다. 이
검사가 생기기 전에는 둘 다 깨끗하게 컴파일됐고, 눈에 보이는 건 제목이 사라진
페이지뿐이었습니다. `base with { f = v }`는 갱신 형태라 일부만 적는 것이
의도이므로, 생성만 검사합니다.

## NonExhaustive

`match`가 모든 변형을 다루지 않습니다.

```
NonExhaustive: match on `Color` is not exhaustive; missing: Blue
```

갈래를 추가하거나, 정말로 "나머지 전부, 영원히"를 뜻한다면 `_`를 추가하세요.
갈래 쪽을 권합니다. 이 진단이 합타입을 쓰는 주된 이유이고, `_`는 그것을
포기하는 것입니다.

## Immutable

상수에 대한 대입입니다.

```
Immutable: cannot reassign constant `Limit` — declare it mutable with
`Limit = …` (no `const`)
```

셋이 바인딩을 상수로 만듭니다. `const`, 뒤에 붙는 `as const`, 그리고 **대문자로
시작하는** 이름입니다. 세 번째가 사람들을 걸려 넘어지게 합니다. `Total = 0`은
대문자 때문에 상수입니다. `maca lint`가 바로 이 이유로 `const`를 명시하라고
권합니다.

## UndefinedName

어디에도 정의되지 않은 이름입니다. 지역 변수도, 함수도, import도, 빌트인도
아닙니다.

```
UndefinedName: call to undefined function `helprr`
```

이 검사가 없으면 오타가 C 컴파일러까지 가서 링크 시점의 undefined reference로
돌아옵니다. 호출 위치의 소문자 이름에 적용됩니다. 대문자 이름은 생성자이고,
UFCS 메서드 호출은 점진적으로 남습니다.

Maca에 없는 키워드도 여기서 다룹니다.

```
UndefinedName: `return`: Maca has no `return` — a function's last expression
is its value
```

전체 목록은 부록 A를 보세요.

## UnknownOption

config 모드에서 옵션 집합이 정의하지 않은 옵션입니다.

```
UnknownOption: unknown option `services.nginx.enabl`
```

config 모드는 이름을 실제 옵션 스키마와 대조하므로, NixOS 옵션의 오타가 배포
시점이 아니라 컴파일 시점에 잡힙니다.

## EffectInConfig

config 모드의 순수하지 않은 연산입니다.

```
EffectInConfig: `async` effect is not allowed in config mode
```

설정은 원하는 상태를 기술합니다. 무언가를 *하면* 안 됩니다. 파일 읽기, 출력,
`spawn`, `await`, `sleep_ms`가 모두 거부됩니다. 하나의 언어가 프로그래밍
언어이면서 설정 언어여도 안전하게 만들어 주는 검사입니다.

## 진단이 아닌 에러들

일부 실패는 파이프라인 아래쪽에서 오고 다르게 읽힙니다.

**파싱/렉싱 에러**는 바이트 범위를 알려줍니다.

```
lex (28, 28): string literal spans a line; write `\n`, or use a raw
"""…""" string. (A literal brace is `\{` or `{{`.)
```

**백엔드 거부** — 특정 타깃이 내보낼 수 없는 올바른 코드입니다.

```
`on:click` needs a live DOM — build this with `--target js`
```

요소가 문자열로 렌더링될 때(15장) 이벤트 핸들러는 붙을 자리가 없습니다. 그래서
네이티브 타깃은 조용히 아무 일도 하지 않는 마크업을 내놓는 대신 그렇다고
말합니다. 각 타깃은 지킬 수 없는 것을 거부하고, 이유도 같습니다. 그러지 않으면
쓰지도 않은 생성 코드에 대한 에러를 보게 되니까요.

| 타깃 | 거부하는 것 |
|---|---|
| native | `on:click=`와 그 형제들 |
| `rust` | 본문 없는(FFI) 함수, `import c`/`import py`, 선언되지 않은 크레이트를 가리키는 `import rust`, 반환하거나 저장하는 빌린 외부 파라미터 |
| `embedded` | `info`와 나머지 콘솔 빌트인, 반환 타입이 있는 `main` |

**C 컴파일러 에러**는 일어나면 안 되고, 일어난다면 보고할 만한 컴파일러 버그
입니다. 오타난 메서드가 예외였습니다. `undefined reference to 'slice'`로 링커까지
살아남았죠. 하지만 `str`나 `T[]`의 메서드 집합은 닫혀 있으므로 거기 없는 이름은
이제 여기서 잡히고, 가까운 것이 있으면 제안도 붙습니다. `any` 수신자에 대한
메서드 호출은 여전히 점진적입니다. 외부 코드에 닿는 방법이 그것이니까요.
