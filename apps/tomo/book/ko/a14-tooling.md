# 툴체인

바이너리 하나, `maca`가 전부를 합니다. 따로 설치할 빌드 도구, 포매터, 패키지
매니저, 테스트 러너가 없습니다.

## 명령들

| 명령 | 하는 일 |
|---|---|
| `maca build FILE` | 네이티브 바이너리로 컴파일 |
| `maca run FILE` | 컴파일 후 실행 |
| `maca test FILE` | 파일의 모든 `test_…` 함수 실행 |
| `maca fmt FILE` | 소스 포매팅 |
| `maca lint FILE` | 스타일 + 타입/이펙트 진단 |
| `maca watch FILE` | 변경 시 재빌드 |
| `maca dev` | 개발 셸 flake 생성 |
| `maca init` | 프로젝트 시작 |
| `maca profile FILE` | callgrind로 실행, 플레임 그래프 렌더 |
| `maca bindgen HEADER` | C 헤더에서 Maca 선언 생성 |
| `maca add SPEC` | 의존성 추가 (`npm:pkg`, `git+url`, `name@ver`) |
| `maca update` | 의존성 재해결 |
| `maca upgrade` | 툴체인 자체 업데이트 |

`build`는 타깃을 받습니다. `--target nix|js|jvm|rust|embedded|tauri`, 그리고
embedded용 `--mcu`와 JVM 클래스패스용 `--cp`.

## 바이너리가 지니고 다니는 것

`maca`는 자기 완결적입니다. 모든 네이티브 빌드가 링크하는 **C 런타임**, 그리고
**표준 라이브러리** 아홉 패키지 전부(`std`, `cli`, `http`, `bench`, `profile`,
`signal`, `tambo`, `web`)가 그 안에 들어 있습니다. 릴리스는 `maca`와
`maca-lsp` 둘뿐입니다.

import를 해결한다는 것은 소스를 읽는다는 것이므로 그 사본은 디스크에 닿아야
합니다. 프로젝트가 아무것도 답하지 못한 첫 import가 사본을 한 번 캐시
디렉터리(`MACA_CACHE`, 없으면 `XDG_CACHE_HOME`, 없으면 `~/.cache/maca`)에
풉니다. 디렉터리 이름은 컴파일러의 버전과 파일들의 다이제스트로 지어서, 한 기계
위의 두 버전이 서로의 `std`를 읽는 일이 없습니다.

프로젝트가 언제나 이깁니다. 직접 쓴 `modules/std/text.maca`나 `maca add`가
설치한 `maca_modules/std/`는 지닌 사본이 제안되기도 전에 평범한 탐색이 먼저
찾아냅니다([모듈과 배치](a9-modules.md)). `MACA_STDLIB=<dir>`은 지닌 사본을
당신의 디렉터리로 바꿉니다.

## 빌드는 캐시됩니다

네이티브 빌드는 소스, 컴파일러 버전, 타깃의 순수 함수입니다. 완성된 바이너리가
그 셋의 해시 아래 저장되고, 바뀌지 않은 프로그램을 다시 빌드하면 파이프라인
전체를 건너뜁니다.

변하지 않는 C 런타임은 컴파일된 오브젝트로 따로 캐시되므로, *바뀐* 프로그램도
런타임을 다시 컴파일하지 않습니다.

`MACA_NO_CACHE=1`로 전부 끌 수 있습니다.

## 빌드하면서 읽는 파일

`data("config/links.json")`은 그 파일을 빌드 시점에 읽고, 프로그램은 읽은 것을
지니고 다닙니다.

```maca
import { decode } from std/json

Config = { title: str, links: Link[] }

Links = "config/links.json"

config: Config = data(Links)
```

값은 바인딩이 선언한 타입으로 읽히고, 그 일을 하는 것이
[`std/json`의 `decode`](a3-stdlib.md)입니다. 경로는 직접 적거나 상수에
묶으세요. 빌드는 실행 중에 계산하는 경로를 따라갈 수 없습니다.

경로는 작업 디렉터리가 아니라 **명령이 부른 파일**을 기준으로 풀립니다.

### `.local`이 커밋된 사본을 가립니다

확장자 앞에 `.local`이 붙은 파일은 그 옆의 파일을 대신합니다.

```
config/links.json         커밋된 것. 새로 클론하면 이것으로 빌드됩니다
config/links.local.json   당신의 것. gitignore 되고, 당신의 빌드가 읽습니다
```

소스는 한 경로만 부르고, 둘 중 어느 쪽이 답하는지는 프로그램이 아니라 트리의
성질입니다. `.local` 사본을 지우면 커밋된 것이 돌아옵니다.

기본값 대신 오류가 되는 것이 셋 있습니다.

| 무엇 | 빌드가 하는 말 |
|---|---|
| 그 경로에 파일이 없음 | `` data("config/links.json"): …/config/links.json: No such file or directory `` |
| 텍스트에 타입을 주는 것이 없음 | `` data(…) reads the file into the type the binding declares; add `import { decode } from std/json` `` |
| 경로가 계산된 것 | `` data(…): the path is read while building, so write it out or bind it to a constant `` |

파일의 바이트는 [빌드 캐시](#빌드는-캐시됩니다)의 키에 들어갑니다.

## 린터

`maca lint`가 의미 검사를 담당합니다. 그 옆의 `apps/lint/lint.maca`는 Maca
자체로 쓰인 스타일 린터입니다.

```
maca run apps/lint/lint.maca            # 저장소 자신의 소스
maca run apps/lint/lint.maca src        # 디렉터리
maca run apps/lint/lint.maca a.maca     # 파일 하나
```

네 가지를 봅니다. 80칸을 넘는 줄, 한 줄짜리 `if` 블록, 줄 끝 공백, 하드 탭.
뭔가 찾으면 0이 아닌 코드로 끝나므로 pre-commit 훅이나 CI에 그대로 넣을 수
있습니다.

폭은 문자열 리터럴을 접은 상태로 재므로, 문자열 안의 긴 C 템플릿은 긴 주석과
똑같이 면제됩니다. 같은 면제가 raw `"""…"""` 블록 안에도 적용됩니다.

## API 문서

`apps/macadoc/macadoc.maca`는 모듈의 선언을 읽고 각각을 그 위의 주석과 짝지어
HTML 레퍼런스를 씁니다.

```
maca run apps/macadoc/macadoc.maca site/api std/text.maca std/list.maca
```

Maca에는 `export` 키워드가 없습니다. 어떤 항목을 API로 만드는 것은 별표
블록으로 쓰는 **문서 주석**입니다.

```maca
/** *첫* 번째 등장에서만 자릅니다: `split_once("a=b=c", "=")`는
 * `["a", "b=c"]`입니다. 구분자가 없으면 문자열 전체와 ""를 줍니다.
 */
split_once(s: str, sep: str) -> str[] {
    …
}

// 인덱스를 `0..len` 안으로 넣습니다. 이쪽은 평범한 주석이라 소스를 읽는 다음
// 사람에게 헬퍼를 설명할 뿐, 레퍼런스에는 들어가지 않습니다.
clamp(n: int, len: int) -> int {
```

컴파일러에게 문서 블록은 다른 주석과 똑같습니다. 세 번째 슬래시는 MacaDoc이 읽는
관례입니다.

문서 주석 안에서 백틱은 코드가 되고 `*별표*`는 강조가 됩니다. 빈 줄은 문단을
시작하고, 들여쓴 줄은 코드 블록입니다. 파일 맨 위의 평범한 `//` 블록은 모듈
자체의 설명입니다.

## 에디터 지원

언어 서버 `maca-lsp`는 진단, 호버, 정의로 이동, 참조 찾기, 문서 심볼, 시그니처
도움말, 자동완성, 이름 변경, 포매팅을 제공합니다.

저장소에는 `apps/editor/zed-maca`에 Zed 확장이 들어 있습니다. Zed에서
*Extensions → Install Dev Extension*을 고르고 그 디렉터리를 가리키면 됩니다.

Monaco와 TextMate용 구문 정의는 렉서의 실제 키워드 목록과 테스트로
동기화됩니다. 언어에 키워드를 추가하고 문법에 넣지 않으면 빌드가 실패합니다.

## 플레이그라운드

`apps/playground/playground.maca`는 에디터, 실시간 진단, 그리고 프런트엔드가 그
프로그램에서 만들어 낸 산출물 전부를 탭 하나씩으로 놓습니다. Console은
인터프리터의 출력과 종료 상태, Preview는 샌드박스 iframe에서 도는 JavaScript,
Definitions는 문서 아웃라인, C와 JS와 CSS는 백엔드가 쓴 것, Nix는 설정 모드가
내보내는 것입니다.

JavaScript 백엔드로 컴파일되는 Maca 파일 하나이고,
[`maca` 브리지](a13-ffi.md)와 [대입이 곧 갱신](a11-ui.md)의 실제 예입니다.
남은 `maca.refresh()` 하나는 호스트에 있고, Maca가 볼 수 없는 단 하나의 사건,
곧 컴파일 결과가 도착한 순간을 위한 것입니다.

## 프로파일링

```
maca profile FILE
maca profile FILE -o flame.svg
```

프로그램을 callgrind 아래에서 실행하고 플레임 그래프를 렌더합니다.

## 프로젝트 구성

`maca init`은 파일 두 개만 씁니다. 이름과 빌드할 `[[bin]]`을 적은 `maca.toml`,
그리고 그 `main.maca`입니다.

```toml
[package]
name = "hello"

[[bin]]
path = "main.maca"
```

Rust 타깃의 의존성은 `[rust-dependencies]` 테이블에 넣으면 Cargo로 전달되고,
`[page]` 테이블은 JS나 Tauri 빌드가 만드는 페이지의 이름을
정합니다([타깃](a10-targets.md)).

당신 코드에 대해서는 모듈 시스템이 매니페스트를 전혀 필요로 하지
않습니다([모듈과 레이아웃](a9-modules.md)).

## 저장소 하나, 패키지 여럿

하나 이상을 담는 저장소는 각각에 `maca.toml`을 하나씩 쓰고, 그것들을 모으는
루트를 하나 더 씁니다.

```toml
# 루트의 maca.toml
[package]
name = "maca"
version = "0.3.2"

[workspace]
members = [
    "modules/std",
    "apps/tomo",
]

[format]
indent_size = 4
```

```toml
# modules/std/maca.toml
[package]
name = "std"
description = "The layer above the prelude builtins."
```

### 어느 매니페스트가 답하는가

**그 키를 말하는 가장 가까운 매니페스트가 답합니다.** 한 파일을 덮는 사슬은 그
파일이 있는 디렉터리의 매니페스트에서 시작해 워크스페이스 루트에서 끝나고, 어떤
키에 대해 아무 말도 하지 않는 매니페스트는 그 위의 답을 물려받습니다.

위 패키지가 이름은 적고 버전은 적지 않은 이유가 그것입니다. 릴리스하는 버전은
워크스페이스의 것입니다.

테이블 셋은 설정이 아니라서 사슬에 오르지 않습니다.

| 테이블 | 읽는 곳 | 이유 |
|---|---|---|
| `[workspace]` | 루트, 그리고 루트에서만 | 그 디렉터리를 루트로 만드는 것이 이것이라서 |
| `[package]` | 그 패키지 자신의 매니페스트 | 멤버는 자기 `name`을 직접 적어야 해서 |
| `[[bin]]` | 그 패키지 자신의 매니페스트 | *이* 패키지가 무엇을 빌드하는지 말하는 것이라서 |

매니페스트가 적는 모든 경로는 그 매니페스트가 놓인 디렉터리 기준입니다.

### 멤버는 나열하고, 그 목록은 대조한다

멤버는 직접 적습니다. 관례로 찾지 않고, 목록만 믿지도 않습니다.

- 나열되었는데 `maca.toml`이 없는 멤버는 그 이름을 밝히는 에러입니다.
- 멤버 옆에 있으면서 `maca.toml`을 들고 있는데 나열되지 않은 디렉터리 역시 그
  이름을 밝히는 에러입니다.

디렉터리는 `maca.toml`을 씀으로써만 패키지가 되므로, 패키지들 옆의 작업용
디렉터리는 결코 패키지가 아닙니다.

### 멤버 매니페스트가 바꾸지 않는 것

어떤 디렉터리가 import 검색 루트인지도, 그 순서도 바꾸지 않습니다. 바뀌는 것은
검색이 어디서 멈추는가 하나뿐입니다. 위로 올라가는 걸음은 이제 처음 만난
`maca.toml`이 아니라 워크스페이스 루트에서 끝납니다
([모듈과 레이아웃](a9-modules.md)).

### 패키지 안에서 일하기

파일을 대지 않으면 세 명령은 작업 디렉터리가 담고 있는 패키지에 대한 것입니다.

```
cd apps/hello
maca build              # 그 패키지의 [[bin]]
maca run                # 같은 것을 빌드해서 실행
maca test               # tests/ 아래의 모든 .maca 스위트
```

`[[bin]]`이 여럿인 패키지에서는 `--bin <name>`으로 고르고, `[package] tests`는
테스트 디렉터리 이름을 바꿉니다. `[[bin]]`이 없는 라이브러리는 이름과 함께
그렇다고 말합니다.

```
$ cd modules/std && maca build
maca: build: package `std` declares no [[bin]] in .../modules/std/maca.toml; name a .maca file
```

### 빌드는 플래그가 아니라 선언

`[build]`는 이 프로젝트를 빌드한다는 것이 무엇인지를 프로젝트가 직접 적는
곳입니다.

```toml
[build]
target = "js"
out = "build"
```

키는 다섯. `target`(`--target`), `out`(`-o`), `mcu`(`--mcu`),
`classpath`(`--cp`), 그리고 `bin`(`--bin`). 모르는 키는 조용히 무시되는 대신 그
이름을 대는 오류입니다.

명령줄의 플래그는 여전히 매니페스트를 이깁니다. 적어둔 타깃은 컴파일러가 소스를
보고 추측했을 타깃도 이깁니다. `out`은 그것을 적은 매니페스트의 디렉터리를
기준으로 답합니다.
