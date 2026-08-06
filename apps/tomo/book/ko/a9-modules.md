# 모듈과 레이아웃

`import`가 어떤 파일을 가리키는지, 컴파일러가 어떤 순서로 찾는지, 프로젝트의
디렉터리가 무엇을 뜻하는지. 가르치는 쪽은 [모듈](07-modules.md)입니다.

## 형태들

| 쓰는 법 | 가리키는 것 |
|---|---|
| `import a/b` | 모듈 `a/b` |
| `import a` | 모듈 `a`. 그런 파일이 없으면 빌트인 |
| `import { f, g } from a/b` | 그 모듈에서 `f`와 `g`만 |
| `import { f } from a` | 같음. 여기서는 한 단어도 파일 |
| `import c "hdr.h"` | C 헤더와 그 라이브러리 |
| `import py "mod"` | Python 모듈 |
| `import js """…"""` | JS 백엔드용 원시 JavaScript |
| `import css """…"""` | JS 백엔드용 원시 CSS |

## 아무것도 가리키지 못하는 import는 에러입니다

슬래시 경로는 모듈만을 가리키므로, 찾지 못한 것은 오타입니다.

```
maca: app.maca: no module `std/str`: `std/str.maca` is not beside this file
or in the working directory
```

**선택적** import는 한 단어여도 같은 약속입니다. 정당하게 파일을 못 찾을 수 있는
유일한 형태는 맨 `import a`입니다.

모듈이 정의하지 않은 이름을 대는 것도 깔끔한 에러입니다.

```
maca: import { centroid } from geometry: 'centroid' is not defined in that module
```

## 해결 순서

`import a/b`는 경로 `a/b`가 되고, 컴파일러는 `a/b.maca`를 찾습니다.

1. **import하는 파일 자신의 디렉터리**에서, 그다음 그 위의 각 디렉터리에서.
   각 디렉터리마다:
   1. `<dir>/a/b.maca`: 쓴 경로를 그대로;
   2. `<dir>/modules/a/b.maca`, `<dir>/src/a/b.maca`,
      `<dir>/maca_modules/a/b.maca`: 탐색 루트들을 이 순서로.
2. 이 순회는 **프로젝트 루트에서 멈춥니다**. 트리에 워크스페이스가 있으면
   워크스페이스 루트이고, 없으면 가장 가까운, `maca.toml`을 가진 디렉터리입니다.
   멤버 자신의 매니페스트는 순회를 멈추지 않습니다([도구](a14-tooling.md)).
3. import하는 파일이 어떤 프로젝트에도 속하지 않으면, 같은 두 단계를
   **작업 디렉터리**에서 한 번 더 시도합니다.
4. 그다음 **형제 파일**: `<import하는 파일의 디렉터리>/b.maca`.
5. 마지막으로, 그리고 마지막에만, **컴파일러가 지니고 다니는 표준
   라이브러리**: `maca` 바이너리 안에 든 `modules/`의 사본입니다.

*위로* 올라가는 것은 트리 깊숙한 프로그램이 `import std/text`라고 쓰고 그
프로젝트의 것을 뜻할 수 있게 해 줍니다. 프로젝트 루트에서 멈추는 것은 탐색이
`$HOME`과 `/`로 새는 것을 막습니다. 형제 파일 규칙이 **마지막**인 것은
의도한 것으로, 먼저 시도했더니 프로그램 옆의 아무 `list.maca`가 진단 하나 없이
`std/list`를 가려 버렸습니다.

### 디렉터리에 패키지 이름을 주지 마세요

1.i이 1.ii보다 먼저이므로, 패키지와 같은 이름의 디렉터리는 그 패키지 대신
답합니다. `modules/bench/` 옆의 `bench/`는 `import bench/stat`을 둘 중 존재하는
쪽으로, 둘 다 있으면 패키지가 아닌 쪽으로 만듭니다. 알려 주는 진단은 없습니다.

탐색 루트를 먼저 두는 것으로는 고쳐지지 않습니다. 순회는 디렉터리를 하나씩
방문하므로 `apps/bench/`는 여전히 `apps/` 아래 어디서든 `bench/…`에 답하고,
`maca_modules`가 탐색 루트이므로 루트 우선은 설치한 의존성이 직접 쓴 파일을
이기게 만듭니다.

## 탐색 루트

| 디렉터리 | 탐색 루트인가 | 쓰는 법 |
|---|---|---|
| `modules/` | 예 | `modules/std/text.maca`는 `std/text` |
| `src/` | 예 | `src/parser.maca`는 `parser` |
| `maca_modules/` | 예 | `maca_modules/toml/parse.maca`는 `toml/parse` |
| `apps/` | **아니오** | `apps/tomo/conf.maca`는 `apps/tomo/conf` |

`modules/*`는 패키지, 곧 import되라고 있는 코드입니다. `src/*`는 패키지가 하나뿐인
저장소를 위한 같은 개념이고, `maca_modules/`는 `maca add`가 의존성을 설치하는
곳입니다.

`apps/`는 일부러 루트가 아닙니다. 두 애플리케이션이 각각 `conf`를 가질 수 있고
어느 쪽도 다른 쪽을 대신 답해서는 안 되기 때문입니다.

`maca.toml`이 어느 것이든 이름을 바꿉니다.

```toml
[layout]
modules = "packages"
src     = "lib"
apps    = "services"
```

키는 줄 단위로 읽으므로 주석 처리된 키는 주석입니다.

## 컴파일러가 지니고 다니는 표준 라이브러리

`maca`는 아홉 패키지를 모두 안에 지니고 다닙니다. `std`, `cli`, `http`,
`bench`, `profile`, `signal`, `tambo`, `web`입니다.

컴파일러는 *소스*를 해결하고 인라인하므로 그 사본은 디스크에 닿아야 합니다.
5단계까지 내려온 첫 import가 사본을 한 번 캐시 디렉터리(`MACA_CACHE`, 없으면
`XDG_CACHE_HOME`, 없으면 `~/.cache/maca`)에 풉니다. 디렉터리 이름은 컴파일러의
버전과 파일들의 다이제스트로 짓습니다.

5단계가 마지막이라는 것이 우선순위 규칙의 전부입니다.

```
modules/std/text.maca        프로젝트 안에         이깁니다
maca_modules/std/text.maca   maca add가 설치한 것   지닌 사본을 이깁니다
                             지닌 사본             둘 다 없을 때만
```

직접 쓴 파일은 컴파일러가 지닌 패키지들에 대해서도 사본을 대신합니다.
`modules/std/text.maca`를 쓰면 지닌 `std/json`도 *당신의* `std/text`를
읽습니다.

체크아웃 전체를 지닌 사본 앞에 두려면 이름을 대세요.

```
MACA_STDLIB=~/src/macalang/modules maca build main.maca
```

그러면 아무것도 풀지 않고, 모든 `std/…`가 그 디렉터리에서 옵니다.

## 엔트리 파일도 인덱스도 없습니다

디렉터리는 모듈이 아닙니다. `modules/http/server.maca`는 `http/server`이고,
이웃들을 다시 내보내는 `modules/http.maca` 같은 것을 대신 import하는 방법은
없습니다.

다른 쪽도 해 봤습니다. 파일마다 이름이 둘이 되고, 파일 하나를 옮길 때 고칠 곳이
하나 더 생겼습니다.

## import가 프로그램에 하는 일

**인라인**합니다. import된 모듈의 정의들이 타입 검사 전에 프로그램 안으로 끼워
넣어집니다. 별도의 컴파일 단위도 링크 단계도 없습니다.

**import되고 나면 이름은 전역입니다.** `parse`를 둘 다 정의하는 두 모듈은
충돌합니다.

**선택적 import는 모듈 경계에서 죽은 코드를 제거합니다.**
`import { origin, dist2 } from geometry`는 그 정의 둘과, 같은 모듈에서
*그것들이* 참조하는 것들의 전이적 폐포를 가져옵니다.

import는 전이적입니다.

## 빌드

`main`이 있는 파일을 드라이버에 넘기세요.

```
maca build app/main.maca
```

import로 닿는 모든 것이 함께 컴파일됩니다.

`maca -m module.function`은 `main` 없이 모듈의 함수를 실행합니다. 종료 상태는
진입점의 선언된 반환 타입에서 오고, `str[]` 매개변수는 남은 명령행을 받습니다.

## 모듈 시스템에 없는 것

가시성 수식자가 없습니다. 모듈이 정의한 모든 것은 import 가능합니다. 파일을
넘어서는 네임스페이스도, `maca add`가 `maca_modules/`에 설치하는 것 말고 버전이
붙은 레지스트리도 없습니다.
