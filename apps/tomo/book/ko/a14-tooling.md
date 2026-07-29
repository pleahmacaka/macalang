# 툴체인

바이너리 하나, `maca`가 전부를 합니다. 따로 설치할 빌드 도구, 포매터 바이너리,
패키지 매니저, 테스트 러너가 없습니다.

모든 명령, 무엇이 캐시되는지, 그리고 다른 언어라면 서브커맨드였을 일을 하는
저장소 안의 Maca 프로그램 둘.

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
embedded용 `--mcu`와 JVM 클래스패스용 `--cp`. `--target`이 없으면 네이티브
바이너리가 나옵니다.

## 빌드는 캐시됩니다

네이티브 빌드는 소스, 컴파일러 버전, 타깃의 순수 함수입니다. 그래서 완성된
바이너리가 정확히 그 셋의 해시 아래 저장됩니다. 바뀌지 않은 프로그램을 다시
빌드하면 파이프라인 전체 — 파싱, 검사, 코드 생성, C 컴파일러 호출 — 를 건너뛰고
캐시된 산출물을 제자리에 복사합니다.

변하지 않는 C 런타임은 컴파일된 오브젝트로 따로 캐시되므로, *바뀐* 프로그램도
런타임을 다시 컴파일하지 않습니다. C 컴파일러를 거치는 것은 당신의 생성된
`main.c`뿐입니다.

`MACA_NO_CACHE=1`로 전부 끌 수 있습니다. 컴파일 시간을 재고 싶을 때 필요한
것입니다.

## 린터

`maca lint`가 의미 검사를 담당합니다. 그 옆에 `tools/lint.maca`가 있는데, Maca
자체로 쓰인 스타일 린터이고 디렉터리 트리를 순회합니다.

```
maca run tools/lint.maca            # 저장소 자신의 소스
maca run tools/lint.maca src        # 디렉터리
maca run tools/lint.maca a.maca     # 파일 하나
```

네 가지를 봅니다. 80칸을 넘는 줄, 한 줄짜리 `if` 블록, 줄 끝 공백, 하드 탭.
뭔가 찾으면 0이 아닌 코드로 끝나므로 pre-commit 훅이나 CI에 그대로 넣을 수
있습니다.

규칙 둘은 보기보다 조심스럽습니다. 폭은 문자열 리터럴을 접은 상태로 재므로,
문자열 안의 200자짜리 C 템플릿은 긴 주석과 똑같이 면제됩니다. 규칙의 대상은
텍스트가 아니라 코드니까요. 같은 면제가 raw `"""…"""` 블록 안에도 적용됩니다.
그 안에 든 것은 Maca가 아니라 외부 CSS나 JavaScript이기 때문입니다.

## API 문서

`tools/macadoc.maca`는 모듈의 선언을 읽고 각각을 그 위의 주석과 짝지어 HTML
레퍼런스를 씁니다. Rust의 rustdoc, TypeScript의 TSDoc에 해당하는 것입니다.

```
maca run tools/macadoc.maca site/api std/text.maca std/list.maca
```

Maca에는 `export` 키워드가 없고, 모듈의 대부분은 헬퍼입니다. 어떤 항목을 API로
만드는 것은 **문서 주석**이고, 슬래시 세 개로 씁니다:

```maca
/// *첫* 번째 등장에서만 자릅니다: `split_once("a=b=c", "=")`는
/// `["a", "b=c"]`입니다. 구분자가 없으면 문자열 전체와 ""를 줍니다.
split_once(s: str, sep: str) -> str[] {
    …
}

// 인덱스를 `0..len` 안으로 넣습니다. 이쪽은 평범한 주석이라 소스를 읽는 다음
// 사람에게 헬퍼를 설명할 뿐, 레퍼런스에는 들어가지 않습니다.
clamp(n: int, len: int) -> int {
```

컴파일러에게 `///` 줄은 다른 주석과 똑같은 주석입니다. 세 번째 슬래시는
MacaDoc이 읽는 관례이지 토큰이 아닙니다. "항목 위의 주석은 모두 API"라는 다른
규칙을 먼저 써 보고 `std/README.md`가 광고하는 것과 대조해 재 봤더니, 60개 중
18개가 틀렸습니다. 설명이 필요했던 헬퍼를 싣고, 설명이 필요 없던 공개 함수를
빠뜨렸습니다.

문서 주석 안에서 백틱은 코드가 되고 `*별표*`는 강조가 됩니다. 빈 줄은 문단을
시작하고, 들여쓴 줄은 코드 블록입니다. 모듈의 머리 주석이 필요한 `import`
줄로 끝날 수 있는 이유가 이것입니다. 파일 맨 위의 평범한 `//` 블록은 모듈 자체의
설명입니다.

## 에디터 지원

언어 서버 `maca-lsp`가 있습니다. 진단, 호버, 정의로 이동, 참조 찾기, 문서 심볼,
시그니처 도움말, 자동완성, 이름 변경, 포매팅을 제공합니다. LSP를 말하는 에디터면
무엇이든 쓸 수 있습니다.

저장소에는 `editor/zed-maca`에 Zed 확장이 들어 있습니다. tree-sitter 문법, 구문
강조, 아웃라인, 언어 서버 연결이 되어 있습니다. 개발 확장으로 설치하세요. Zed에서
*Extensions → Install Dev Extension*을 고르고 그 디렉터리를 가리키면 됩니다.

Monaco(플레이그라운드)용과 TextMate용 구문 정의는 렉서의 실제 키워드 목록과
테스트로 동기화됩니다. 언어에 키워드를 추가하고 문법에 넣지 않으면 빌드가
실패합니다.

## 플레이그라운드

`playground/playground.maca`는 브라우저 플레이그라운드입니다. 에디터, 실시간
진단, 프로그램을 실행해 C와 JavaScript 출력을 보는 기능이 있습니다. JavaScript
백엔드로 컴파일되는 Maca 파일 하나이고, 자기 호스트 글루와 스타일시트를 raw
문자열로 인라인에 들고 다닙니다. 이 언어로 쓴 실제 프로그램의 예로 읽어볼 만합니다.

## 프로파일링

```
maca profile FILE
maca profile FILE -o flame.svg
```

프로그램을 callgrind 아래에서 실행하고 플레임 그래프를 렌더합니다. 주로
컴파일러 자신에 유용합니다. 존재하는 가장 큰 Maca 프로그램이니까요.

## 프로젝트 구성

`maca init`이 `maca.toml`과 함께 프로젝트를 시작합니다. Rust 타깃의 의존성은
`[rust-dependencies]` 테이블에 넣으면 Cargo로 전달됩니다.

당신 코드에 대해서는 모듈 시스템이 매니페스트를 전혀 필요로 하지 않습니다.
`maca build app/main.maca`가 import를 따라갑니다.
[모듈과 레이아웃](a9-modules.md)을 보세요.
