# 타깃

같은 소스가 아주 다른 것들로 컴파일됩니다. 둘러보기는
[프로그램이 도착하는 곳](15-targets.md)이고, 이 장은 모든 타깃, 모든 플래그,
그리고 각 타깃이 거부하는 것의 정확한 목록입니다.

## 플래그

| 명령 | 만드는 것 |
|---|---|
| `maca build app.maca -o app` | C를 거친 정적 네이티브 바이너리 |
| `maca build app.maca --target js -o out` | 자체 완결적인 페이지 |
| `maca build app.maca --target jvm --cp …` | Java 소스 |
| `maca build app.maca --target rust` | Rust 소스 |
| `maca build app.maca --target embedded --mcu cortex-m0` | freestanding C |
| `maca build app.maca --target nix` | `.nix` 식 |
| `maca build app.maca --target tauri` | 데스크톱 애플리케이션 스캐폴드 |

`--mcu`는 `embedded`의, `--cp`는 `jvm`의 것입니다.

## 네이티브, C를 거쳐

배포할 런타임도, 인터프리터도, 수집기도 없습니다.

```
maca build app.maca -o app
./app
```

수치 연산이 몰린 구간도 같은 C 경로를 탑니다. `f32x8` 같은 벡터 타입은 C 백엔드가
레인 폭 벡터로 낮추고, `-mavx2`가 붙어 256비트 레지스터 하나로 컴파일됩니다.

## JavaScript

```
maca build app.maca --target js -o out
```

자체 완결적인 페이지 하나를 만듭니다. JavaScript 백엔드는 Maca의 반응형 UI
문법을 이해하고, 찾아낸 Tailwind 유틸리티 클래스에 대한 CSS를 생성합니다.
번들러도, `package.json`도 필요 없습니다.

같은 문법이 네이티브 타깃에서는 HTML 문자열로 렌더링됩니다
([UI 문법](a11-ui.md)).

### 페이지가 자기를 무엇이라 말하는가

프로젝트가 달리 말하지 않으면 페이지는 소스 파일 이름을 따릅니다. 달리 말하는
곳은 `maca.toml`입니다.

```toml
[page]
title = "tabpane"
lang = "ko"
description = "a browser start page"
```

`title`은 `<title>`을 채우고 `--target tauri`에서는 창의 제목도 됩니다. `lang`은
`<html lang>` 속성이 되고 `description`은 메타 태그가 됩니다. 셋 중 어느 것도
아닌 키는 기본값이 아니라 오류입니다.

### 페이지가 무엇을 지고 가는가

페이지에 필요한 Maca가 아닌 스타일시트나 스크립트는 둘 다 import입니다.

```maca
import "vendor/daisyui.css"       // 파일. 빌드 시점에 읽습니다
import "vendor/iconify-icon.js"    // 파일. 빌드 시점에 읽습니다

import css """
.card { border-radius: 8px }
"""                                   // 인라인으로 쓴 소스 자체
```

따옴표 문자열은 파일을 가리키고, 확장자가 이미 그것이 무엇인지 말하므로 언어
이름을 붙이지 않습니다. 파일은 그것을 import한 소스를 기준으로 찾아 빌드 시점에
읽어 **인라인**합니다. 스타일시트는 생성된 `<style>` 앞으로, 스크립트는 앱이
마운트하는 요소 뒤로 들어갑니다. 링크는 하지 않습니다. `index.html` 하나가
배포물 전체입니다. `import "x.wasm"`은 바이너리를 base64로 심습니다.

가리키는 파일이 없는 경로는 빌드를 실패시키고 그 파일을 이름으로 말합니다.

## JVM과 Rust

이 둘은 배포보다 *도달 범위*에 관한 것입니다.

`--target jvm`은 Java 소스를 내보내고, 저장소의 예제는 Fabric을 통한 Minecraft
모드입니다. `--target rust`는 Rust 소스를 내보내고 `maca.toml`의
`[rust-dependencies]`에서 의존성을 가져옵니다.

대가는 그 타깃의 시작 비용과 런타임을 함께 물려받는다는 것입니다.

### 외부 trait 구현하기

`Type : Trait = { … }`는 Rust trait 구현을 선언합니다. 메서드 하나가 필드
하나입니다. 람다의 반환 타입을 반드시 적어야 하는 곳은 여기 하나뿐입니다.
trait이 컴파일러가 읽지 않는 크레이트 안에 있기 때문입니다.

```maca
Counter : Render = {
    render = (self, window, cx) -> AnyElement =>
        div().child("Count: {self.count}").into_any_element()
}
```

이 형태는 Rust 타깃 전용입니다.

## 임베디드

```
maca build blink.maca --target embedded --mcu cortex-m0
```

베어메탈 마이크로컨트롤러용 freestanding C를 내보냅니다. libc도, 할당기도,
운영체제도 없습니다. Cortex-M과 RISC-V를 지원합니다. 메모리 맵 레지스터는
평범한 Maca 값이고, 필드 쓰기는 알맞은 폭의 read-modify-write로 낮춰집니다.

콘솔이 없으므로 `info`와 그 형제들은 쓸 수 없습니다. UART는 `mmio_write`로 직접
다루세요. 그리고 `main`은 아무것도 반환하지 않습니다. 리셋 핸들러가 호출하고,
반환하면 멈춥니다.

여기서 `int`는 64비트가 아니라 32비트 워드입니다. MMIO 어휘는
`mmio_write`/`mmio_read`, `set_bits`/`clear_bits`/`toggle_bits`, `bit`,
`shl`/`shr`/`bit_or`/`bit_and`, `delay`, `nop`이고, `for _ in forever()`가
슈퍼 루프입니다.

## Nix

`--target nix`는 [설정 모드](a12-config.md)의 출력입니다. 바이너리를 만들어낸 그
언어가 그 바이너리가 돌아갈 머신도 기술합니다.

## Tauri

`--target tauri`는 데스크톱 애플리케이션을 스캐폴딩합니다. 인터페이스는
JavaScript 백엔드로, 그 아래는 네이티브 바이너리로요.

## 각 타깃이 거부하는 것

타깃은 지킬 수 없는 것을 컴파일 시점에, 이름을 대며 거부합니다.

| 타깃 | 거부하는 것 |
|---|---|
| native | `on:click=`와 그 형제들: 문자열에는 이벤트 핸들러가 붙을 자리가 없음 |
| `rust` | 본문 없는(FFI) 함수, `import c`/`import py`, 선언되지 않은 크레이트를 가리키는 `import rust`, 반환하거나 저장하는 빌린 외부 파라미터, 다른 함수 안에 정의된 함수 |
| `jvm` | 다른 함수 안에 정의된 함수 |
| `embedded` | `info`와 나머지 콘솔 빌트인, 반환 타입이 있는 `main`, 다른 함수 안에 정의된 함수 |
| `nix` | 비어 있지 않은 이펙트 행 전부([이펙트와 async](a7-effects.md) 참조), 떠날 함수가 없는 설정 모드의 `return` |

다른 함수 안에 정의된 함수를 세 타깃이 거부하는 이유는 셋 다 *쓰기*에
있습니다. Rust는 두 클로저가 한 지역 변수를 동시에 가변으로 빌리도록 두지
않고, Java 람다는 사실상 final인 변수를 캡처하며, 프리스탠딩 C에는 공유되는
지역 변수에 필요한 힙 셀을 줄 할당기가 없습니다. 네이티브 C와 JS 백엔드는 둘 다
이것을 낮춥니다.

## ABI와 링크

네이티브 쪽은 전부 **C ABI**로 모입니다. SIMD를 포함해 오브젝트는 모두 C 백엔드가
내보내고, FFI 선언은 평범한 extern이며, async 함수는 평범한
함수입니다.

## 무엇을 고를까

대부분의 프로그램은 네이티브를 원하고, 질문은 프로그램 바깥의 무언가가 답을
정할 때만 생깁니다. 쓸모 있는 성질은 백엔드가 많다는 게 아니라, 그 사이를 옮겨
다니는 비용이 다시 쓰기가 아니라 플래그 하나라는 점입니다.

## 목록이 멈추는 지점

여기 있는 타깃은 전부 무언가에 닿는 것으로 제 자리를 법니다. 브라우저,
마이크로컨트롤러, JVM의 라이브러리, crates.io.

사람들이 묻는 BEAM 타깃이 목록에 없는 이유는, 그것이 도달 범위가 아니라 우아함
때문에 추가되는 첫 백엔드가 될 것이기 때문입니다.
