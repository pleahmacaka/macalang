# 진단 메시지

검사기가 내보내는 모든 진단, 그 의미, 그리고 대처법입니다. 여섯 종류와, 그보다
아래쪽 파이프라인에서 와서 다르게 읽히는 실패들입니다.

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
TypeMismatch: `Config` is missing field(s): port, title
TypeMismatch: `Config` has no field `titel`; did you mean `title`?
```

빠뜨린 필드는 조용한 0이 됩니다. `str`이면 `""`, `int`면 `0`입니다. 오타는 더
나쁩니다. 값은 아무 데도 가지 않고, 원래 가려던 필드는 빈 채로 남습니다. 이
검사가 생기기 전에는 둘 다 깨끗하게 컴파일됐고, 눈에 보이는 건 제목이 사라진
페이지뿐이었습니다. `base with { f = v }`는 갱신 형태라 일부만 적는 것이
의도이므로, 생성만 검사합니다.

익명 표기도 같은 두 가지를 집니다. 그리고 메시지는 타입이 아니라 바인딩의 이름을
말합니다. 그것이 저자가 쓴 것이기 때문입니다.

```
TypeMismatch: in `p`: record is missing field `y`
TypeMismatch: in `p`: record has unexpected field `z`
```

레코드 리터럴은 그렇지 않으면 열려 있습니다. 한 필드를 읽으려고 나머지를 다 알아야
할 필요는 없기 때문입니다. 이름 있는 표기와 익명 표기가 만나는 곳은
[타입 시스템](a6-types.md)입니다.

**어느 쪽이 어느 쪽인가.** `expected`는 언제나 코드가 *선언한* 타입이고
`found`는 도착한 값입니다. 타입 표기, 반환 타입, 파라미터가 expected이고, 식이
found입니다. 반대로 된 짝은 프로그램에 대한 힌트가 아니라 컴파일러 버그입니다.

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
UndefinedName: `return`: a function's last expression is its value, so drop
the `return`
```

각 힌트는 동작하는 형태를 먼저 말하고, 없는 단어를 뒤에 붙입니다. 반대 순서는
거부로 읽혔습니다. "Maca has no `return`"이 먼저 도착하는 바람에, 어떤 독자는
함수가 값을 돌려줄 수 없다고 받아들였습니다. Maca에는 Rust와 똑같은 규칙이 있는데
말이죠. 단어는 곁가지이고, 대신 무엇을 쓰는지가 메시지입니다. 전체 목록은
[키워드](a1-keywords.md)를 보세요.

## UnknownOption

config 모드에서, 컴파일러가 모르는 옵션 **네임스페이스**에 대한 대입입니다.

```
UnknownOption: unknown NixOS option namespace `servicez`
```

아는 루트는 NixOS의 것들입니다. `networking`, `services`, `system`, `users`,
`environment`, `programs`, `boot`, `hardware`, `security`, `nix`, `fonts`와
그 형제들, 그리고 파일 안의 지역 바인딩입니다.

어디까지 미치는지를 분명히 해 두는 편이 좋습니다. 배포가 실패했을 때 어디를
볼지가 여기서 갈리기 때문입니다. 네임스페이스는 검사되고 잎은 검사되지
않습니다. `servicez.nginx.enable`은 여기서 잡히고,
`services.nginx.enabl`은 Nix까지 가서 평가 시점에 Nix 자신의 메시지로
거부됩니다. `maca dev`는 이 진단을 통째로 억제합니다. `dev.*`는 애초에 NixOS
네임스페이스가 아니기 때문입니다.

## EffectInConfig

config 모드의 순수하지 않은 연산입니다.

```
EffectInConfig: config must be pure but this uses effect(s): async
```

메시지는 찾아낸 행을 전부 나열하므로, 출력도 하고 잠도 자는 설정은
`io, async`로 보고됩니다.

설정은 원하는 상태를 기술합니다. 무언가를 *하면* 안 됩니다. 출력, `fail`,
`spawn`, `await`, `sleep_ms`, 그리고 `net`/`http`/`socket`이나 `os`/`process`
수신자를 통한 호출이 거부됩니다. 하나의 언어가 프로그래밍 언어이면서 설정
언어여도 안전하게 만들어 주는 검사입니다.

여기서도 검사가 닿는 범위를 분명히 해 두는 편이 낫습니다. 행은 호출의
*모양*으로 판정합니다. 알려진 빌트인 이름이거나, 위 수신자들 중 하나에 대한
메서드거나. 그래서 `file.read(p)`는 잡히고 자유 함수 `read_file(p)`는 잡히지
않습니다. 자유 빌트인으로 파일을 읽는 설정은 오늘 컴파일됩니다. 행과 각 행을
만드는 것은 [이펙트와 async](a7-effects.md)에 있습니다.

## import 해석

이것들은 검사기보다 앞, 컴파일러가 각 `import`가 어느 파일을 가리키는지 알아내어
인라인하는 동안 나옵니다. `DiagKind`는 아니지만 사람이 실제로 만나는 컴파일
에러이고, 하나하나가 조용히 틀린 답을 대신한 것입니다.

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
이름을 공유하면 패키지를 가립니다. 두 후보는 구조상 같은 이름을 가진 서로 다른
실제 파일이라서 하나는 컴파일되고 다른 하나는 조용히 되지 않으며, 어느 쪽인지는
import 줄이 말하지 않는 디렉터리 배치가 결정합니다. 그것은 누가 표현한 선호가
아니므로 존중할 것이 없습니다. 디렉터리 이름을 바꾸거나, 한 파일만 가리키는 경로를
쓰세요.

**한 이름을 두 모듈 이상이 정의**하면 인라인할 수 없습니다. 모든 것이 하나의
번역 단위가 되기 때문입니다.

```
`render` is defined by more than one module of this program, and every module
is inlined into one:
  modules/tomo/page.maca
  modules/tomo/feed.maca
  Both are API, so neither can be moved out of the way. Rename one of them, or
  ask for the one you mean with `import { … } from …` and keep the other out of
  the program.
```

두 파일이 각각 같은 이름의 *비공개* 헬퍼를 두는 것은 괜찮습니다. 컴파일러가
모듈 자신의 이름으로 한정해 줍니다. 이 에러는 둘 다 API일 때 나오고, 그때
이름을 바꾸는 결정은 저자만이 할 수 있습니다.

**아무것도 정해 주지 않는 참조**는 같은 충돌을 제3의 파일에서 본 것입니다.

```
apps/site/home.maca: `render` is defined by more than one module this file
reaches, and every module is inlined into one:
  modules/tomo/page.maca
  modules/tomo/feed.maca
  Nothing here says which one `render` means. Ask for the one you mean with
  `import { render } from …`, or rename the others.
```

위의 것과 다른 점은 모호함이 어디에 있는지입니다. 위에서는 두 모듈이 그 이름을
답하고, 여기서는 *제3의* 모듈이 그 이름을 쓰는데 그 파일의 무엇도 어느 쪽을
뜻했는지 말하지 않습니다. 어느 쪽이든 선택적 import가 답입니다. 원하는 것을
이름으로 말하는 것이 이것을 정해 주는 유일한 방법이기 때문입니다.

**어느 파일로도 해석되지 않는 import**도 에러입니다. 한 단어짜리 선택적 import도
포함됩니다. 빌트인에서는 골라낼 것이 없기 때문입니다.

```
apps/x/main.maca: no module `std/str`: `std/str.maca` is not beside this file
or in the working directory
```

한때 네 파일이 존재한 적 없는 `std/str`을 조용히 import했고, 각자 import하고
있다고 믿은 헬퍼를 직접 손으로 썼습니다.

## 진단이 아닌 에러들

일부 실패는 파이프라인 아래쪽에서 오고 다르게 읽힙니다.

**파싱/렉싱 에러**는 바이트 범위를 알려줍니다.

```
lex (28, 28): string literal spans a line; write `\n`, or use a raw
"""…""" string. (A literal brace is `\{` or `{{`.)
```

모호한 `=> { … }`가 그중 하나입니다. 모든 엔트리가 서로 다른 `name = value`이고
줄바꿈만이 그것들을 구분하면 레코드 리터럴과 블록이 똑같이 읽히므로, 어느 해석도
택하지 않습니다.

```
parse (45, 46): `mk`: this `=> { … }` reads as a record literal and as a
block. Write `Name { … }` for the record, or drop the `=>` for the block
```

전체 규칙과, 모호하지 않은 경우에 무엇이 그것을 결정하는지는
[문법](a5-syntax.md)에 있습니다.

**백엔드 거부** — 특정 타깃이 내보낼 수 없는 올바른 코드입니다.

```
`on:click` needs a live DOM — build this with `--target js`
```

요소가 문자열로 렌더링될 때([UI 문법](a11-ui.md)) 이벤트 핸들러는 붙을 자리가
없습니다. 그래서
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
