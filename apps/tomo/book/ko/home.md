# 프로그램과 그것이 돌아갈 기계를, 하나의 타입 있는 언어로

Maca는 **정적 타입**이고, **네이티브 바이너리로 컴파일**되며, **가비지
컬렉터가 없습니다**. 서비스를 쓰는 그 언어가 그 서비스가 배포될 기계도
기술합니다.

```maca
Shape = Circle(float) | Rect(float, float)

area(s: Shape) -> float =>
    match s {
        Circle(r)  => 3.14159 * r * r
        Rect(w, h) => w * h
    }

main() -> int {
    floor = [Circle(1.0), Rect(3.0, 4.0)]
    info("floor plan: {floor.map(area).sum():.2} m²")
    0
}
```

플레이그라운드에서 바로 돌려보세요. 컴파일러 자체가 WebAssembly로 컴파일되어
있어서, 서버도 설치도 없이 페이지 안에서 돕니다.

## 당신에게 맞는 도구인가요?

언어를 어느 칸에 넣을지 정하는 결정들을, 돌려 말하지 않고 적습니다.

| 질문 | 답 |
|---|---|
| 타입? | **정적**이고 추론합니다. 시그니처가 아닌 자리에 타입을 쓸 일은 거의 없습니다. `any`가 있지만 기본값이 아니라 외부 코드를 위한 명시적 탈출구입니다. |
| GC? | **추적 수집기 없음.** 참조 카운트를 컴파일러가 넣습니다(Perceus). 런타임도, 멈춤도 없습니다. |
| 컴파일? 인터프리터? | **컴파일**입니다. C를 거치고 최적화는 `cc`가 합니다. `maca run`이 컴파일과 실행을 한 번에 하므로 스크립트처럼 느껴집니다. |
| 얼마나 빠른가? | 재귀와 실수 루프에서 C와 오차 범위 안입니다. [벤치마크](https://github.com/pleahmacaka/macalang/blob/main/bench/results.md)가 저장소에 있고, 지는 경우 하나와 그 이유도 함께 있습니다. |
| 메모리 안전? | 수동 `free` 없음, 안전한 코드에서 use-after-free 없음, 만족시켜야 할 빌림 검사기도 없음. 값은 값이고, 언제 놓아줄지는 컴파일러가 알아냅니다. |
| 동시성? | 스레드와 `spawn`/`await`, 그리고 **함수 색칠이 없습니다.** 호출 그래프 전체로 번져 나갈 `async` 키워드가 없습니다. |
| 런타임? | 정적 링크되는 작은 C 런타임. hello-world는 아무것도 의존하지 않는 바이너리 하나입니다. |
| 성숙도? | 어립니다. 컴파일러는 완성되어 테스트로 잠겨 있고, 표준 라이브러리는 의도적으로 작으며, 스스로를 부트스트랩하는 중입니다. |

## 무엇이 다른가

### 같은 언어가 기계를 설정합니다

컴파일러를 설정 파일로 향하면 바이너리 대신 Nix를 내보냅니다. 같은 문법, 같은
타입 검사기, 같은 에디터 도구 — 그리고 설정 모드에서는 검사기가 I/O를
거부하므로, 기계 기술서가 몰래 무언가를 할 수 없습니다.

```maca
import nixpkgs

networking.hostName = "rigel"
system.packages     = git, curl, htop, ripgrep

services.openssh = {
    passwordAuthentication = false
}
```

`maca build --target nix system.maca`가 저것을 NixOS 모듈로 바꿉니다. 서비스는
이 언어로, 그 서비스가 도는 기계는 저 언어로 관리해 본 적이 있다면, 여기부터
보세요.

### 비동기에 색이 없습니다

`spawn f(x)`가 `f`를 동시에 돌리고 `await`이 기다립니다. 표면은 그게 전부입니다.
비동기성은 함수 타입의 성질이 아니라 추론되는 이펙트라서, 어떤 함수도 자기를
부르는 모든 것에 번지는 색으로 물들지 않습니다.

### 하나의 소스, 여섯 개의 타깃

네이티브 C가 기본입니다. 같은 소스가 반응형 DOM을 만드는 JavaScript로도, JVM
연동을 위한 Java 소스로도, crates.io 라이브러리를 설정 한 줄로 쓰게 해 주는
Rust 소스로도, 설정을 위한 Nix로도, 그리고 libc도 할당기도 없는 Cortex-M /
RISC-V 마이크로컨트롤러용 freestanding C로도 컴파일됩니다.

### 마크업이 문자열이 아니라 문법입니다

태그 이름을 함수처럼 부르면 그게 요소입니다. JS 타깃에서는 반응형 DOM을 만들고,
네이티브에서는 HTML 문자열로 렌더링하며, 컴파일러가 실제로 쓴 유틸리티 클래스
만큼의 스타일시트를 생성합니다.

```maca
page(title: str) -> str =>
    article(class="max-w-2xl mx-auto",
        h1(class="font-bold", title)
        p("서버에서든 브라우저에서든, 이 한 줄에서 렌더링됩니다.")
    )
```

**이 웹사이트가 그 기능입니다.** Markdown을 읽어 지금 보고 있는 페이지들을 쓰는
Maca 프로그램이고, 손으로 쓴 마크업은 없으며 손으로 쓴 CSS는 한 줄입니다.

## 이럴 땐 맞지 않습니다

- 오늘 당장 큰 생태계가 필요할 때. Maca는 자기 생태계를 키우는 대신 다른
  생태계 — C, Python, crates.io, Maven — 에 닿습니다.
- 추적 GC와 그것이 주는 "순환을 신경 쓰지 않아도 되는 자유"가 필요할 때. 참조
  카운팅은 순환을 수거하지 않습니다.
- 검증된 언어가 필요할 때. 이 언어는 어리고, 그렇다고 말합니다.

## 시작하기

```sh
curl -fsSL https://raw.githubusercontent.com/pleahmacaka/macalang/main/install.sh | bash
maca init hello && cd hello
maca run main.maca
```

그다음 스물일곱 개 장과 부록으로 된 핸드북을 읽거나, 플레이그라운드를 열고
뭔가 바꿔 보세요.

소스와 이슈 트래커와 벤치마크는
[GitHub](https://github.com/pleahmacaka/macalang)에 있습니다. 비판을
환영합니다 — 특히 위 표의 어느 칸이 잘못된 선택인지 짚어 주는 쪽으로요.
