# 진단 메시지

검사기가 내보내는 모든 진단, 그 의미, 그리고 대처법입니다.

## TypeMismatch

같아야 할 두 타입이 같지 않았습니다.

**인자 타입**

```
TypeMismatch: in call to `d` (argument 2): type mismatch: expected P, found int
```

**인자 개수**: 개수도 타입의 성질이라 여기로 옵니다.

```
TypeMismatch: call to `f` expects 2 argument(s), got 3
```

**어긋나는 갈래**: 타입이 다른 `if`나 삼항입니다.

```
TypeMismatch: ternary branches disagree: type mismatch: expected int, found str
```

`if`와 `match`는 식이므로 갈래들의 타입이 맞아야 합니다. `c ? continue : 0`이
이 이유로 실패합니다.

**레코드 필드**: 리터럴은 레코드가 선언한 모든 필드를 적어야 하고, 선언하지
않은 필드는 적으면 안 됩니다.

```
TypeMismatch: `Config` is missing field(s): port, title
TypeMismatch: `Config` has no field `titel`; did you mean `title`?
```

빠뜨린 필드는 조용한 0이 되고, 오타는 값이 아무 데도 가지 않게 만듭니다.
`base with { f = v }`는 일부만 적는 것이 의도이므로 생성만 검사합니다.

익명 표기도 같은 두 가지를 잡고, 메시지는 타입이 아니라 바인딩의 이름을
말합니다.

```
TypeMismatch: in `p`: record is missing field `y`
TypeMismatch: in `p`: record has unexpected field `z`
```

레코드 리터럴은 그렇지 않으면 열려 있습니다([타입 시스템](a6-types.md)).

**어느 쪽이 어느 쪽인가.** `expected`는 언제나 코드가 *선언한* 타입이고
`found`는 도착한 값입니다.

## NonExhaustive

`match`가 모든 변형을 다루지 않습니다.

```
NonExhaustive: match on `Color` is not exhaustive; missing: Blue
```

갈래를 추가하거나, 정말로 "나머지 전부, 영원히"를 뜻한다면 `_`를 추가하세요.

## Immutable

상수에 대한 대입입니다.

```
Immutable: cannot reassign constant `Limit`; declare it mutable with
`Limit = …` (no `const`)
```

셋이 바인딩을 상수로 만듭니다. `const`, 뒤에 붙는 `as const`, 그리고 **대문자로
시작하는** 이름입니다. `Total = 0`은 대문자 때문에 상수입니다.

## UndefinedName

지역 변수도, 함수도, import도, 빌트인도 아닌 이름입니다.

```
UndefinedName: call to undefined function `helprr`
```

호출 위치의 소문자 이름에 적용됩니다. 대문자 이름은 생성자이고, UFCS 메서드
호출은 점진적으로 남습니다.

Maca에 없는 키워드도 여기서 다룹니다.

```
UndefinedName: `return`: a function's last expression is its value, so drop
the `return`
```

각 힌트는 동작하는 형태를 먼저 말하고 없는 단어를 뒤에 붙입니다. 전체 목록은
[키워드](a1-keywords.md)를 보세요.

**패턴**에 쓰인 대문자 이름도 여기서 다룹니다.

```
UndefinedName: `Busi` is capitalized, so it is a constructor, and nothing
declares one by that name: did you mean `Busy`?
```

패턴에서 `Busy`는 그 변형에 맞고 `busy`는 맞은 것을 이름에 묶습니다. 잘못 적은
변형은 소리 없이 *모든 것*에 맞는 패턴이 되고, 그 아래 팔들은 닿지 않게 되는데도
`match`는 여전히 빠짐없어 보입니다.

## UnknownOption

config 모드에서, 컴파일러가 모르는 옵션 **네임스페이스**에 대한 대입입니다.

```
UnknownOption: unknown NixOS option namespace `servicez`
```

아는 루트는 NixOS의 것들입니다. `networking`, `services`, `system`, `users`,
`environment`, `programs`, `boot`, `hardware`, `security`, `nix`, `fonts`와
그 형제들, 그리고 파일 안의 지역 바인딩입니다.

네임스페이스는 검사되고 잎은 검사되지 않습니다. `services.nginx.enabl`은 Nix
까지 가서 평가 시점에 거부됩니다. `maca dev`는 이 진단을 통째로 억제합니다.

## EffectInConfig

config 모드의 순수하지 않은 연산입니다.

```
EffectInConfig: config must be pure but this uses effect(s): async
```

출력, `fail`, `spawn`, `await`, `sleep_ms`, 그리고 `net`/`http`/`socket`이나
`os`/`process` 수신자를 통한 호출이 거부되고, 메시지는 찾아낸 행을 전부
나열합니다.

행은 호출의 *모양*으로 판정합니다. 그래서 `file.read(p)`는 잡히고 자유 함수
`read_file(p)`는 잡히지 않습니다([이펙트와 async](a7-effects.md)).

## import 해석

검사기보다 앞, 컴파일러가 각 `import`가 어느 파일을 가리키는지 알아내어
인라인하는 동안 나옵니다.

**모호한 import**는 두 파일을 가리키며, 하나를 고르는 대신 거부합니다.

```
apps/x/main.maca: ambiguous import `bench/stat`: it names two files:
  apps/x/bench/stat.maca (as written, the one this build would use)
  modules/bench/stat.maca (under a search root)
  A directory sharing a package's name hides the package, and the import line
  cannot say which was meant. Rename the directory, or move the module so that
  one path names it.
```

쓰인 경로를 탐색 루트보다 먼저 시도하므로, 소스 옆에 있는 디렉터리가 패키지와
이름을 공유하면 패키지를 가립니다.

**한 이름을 두 모듈 이상이 정의**하면 인라인할 수 없습니다.

```
`render` is defined by more than one module of this program, and every module
is inlined into one:
  modules/tomo/page.maca
  modules/tomo/feed.maca
  Both are API, so neither can be moved out of the way. Rename one of them, or
  ask for the one you mean with `import { … } from …` and keep the other out of
  the program.
```

두 파일이 각각 같은 이름의 *비공개* 헬퍼를 두는 것은 괜찮습니다.

**아무것도 정해 주지 않는 참조**는 같은 충돌을 제3의 파일에서 본 것입니다.

```
apps/site/home.maca: `render` is defined by more than one module this file
reaches, and every module is inlined into one:
  modules/tomo/page.maca
  modules/tomo/feed.maca
  Nothing here says which one `render` means. Ask for the one you mean with
  `import { render } from …`, or rename the others.
```

어느 쪽이든 선택적 import가 답입니다.

**어느 파일로도 해석되지 않는 import**도 에러입니다. 한 단어짜리 선택적 import도
포함됩니다.

```
apps/x/main.maca: no module `std/str`: `std/str.maca` is not beside this file
or in the working directory
```

## 진단이 아닌 에러들

**파싱/렉싱 에러**는 바이트 범위를 알려줍니다.

```
lex (28, 28): string literal spans a line; write `\n`, or use a raw
"""…""" string. (A literal brace is `\{` or `{{`.)
```

모호한 `=> { … }`가 그중 하나입니다.

```
parse (45, 46): `mk`: this `=> { … }` reads as a record literal and as a
block. Write `Name { … }` for the record, or drop the `=>` for the block
```

전체 규칙은 [문법](a5-syntax.md)에 있습니다.

**백엔드 거부**: 특정 타깃이 내보낼 수 없는 올바른 코드입니다.

```
`on:click` needs a live DOM; build this with `--target js`
```

요소가 문자열로 렌더링될 때([UI 문법](a11-ui.md)) 이벤트 핸들러는 붙을 자리가
없습니다.

| 타깃 | 거부하는 것 |
|---|---|
| native | `on:click=`와 그 형제들 |
| `rust` | 본문 없는(FFI) 함수, C 또는 Python import, 선언되지 않은 크레이트를 가리키는 import, 반환하거나 저장하는 빌린 외부 파라미터 |
| `embedded` | `info`와 나머지 콘솔 빌트인, 반환 타입이 있는 `main` |

**C 컴파일러 에러**는 일어나면 안 되고, 일어난다면 보고할 만한 컴파일러
버그입니다. `str`나 `T[]`에 없는 메서드 이름은 이제 검사기에서 잡힙니다.
