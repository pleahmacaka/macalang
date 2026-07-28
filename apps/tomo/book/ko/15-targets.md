# 타깃

하나의 언어, 여러 백엔드. 타깃은 여러분이 고르고, 방법은 컴파일러가 고릅니다.

## 타깃들

- **네이티브** (기본) — C를 거쳐 정적 바이너리로. 빠르고 범용적인 경로입니다.
- **SIMD** — 뜨거운 수치 구간은 벡터화를 위해 LLVM으로 내려갈 수 있습니다.
  나머지와는 C ABI로 링크됩니다.
- **JavaScript** — 같은 소스에서 Tailwind 스타일의 반응형 UI로.
- **BEAM** — Erlang/Elixir 생태계의 동시성과 내결함성을 위해.
- **JVM** — JVM 상호운용을 위한 Java 소스 (일례로 Minecraft/Fabric 모드).
- **Rust** — crates.io 라이브러리 위에 쌓기 위한 Rust 소스.
- **Nix** — 앞 장에서 본 설정 모드.
- **임베디드** — 베어메탈 MCU(Cortex-M / RISC-V)용 freestanding C.

## `maca` CLI

```
maca run app.maca                 # 컴파일 후 실행
maca build app.maca -o app        # 네이티브 바이너리
maca build app.maca --target js   # 웹 번들
maca build cfg.maca --target nix  # Nix 식
maca dev                          # 개발 셸 (Nix 플레이크, 또는 Windows 툴체인)
maca watch                        # 변경 시 재빌드
maca fmt / maca lint              # 포맷과 린트
maca test                         # 테스트 실행
```

## 모듈

프로그램은 `import`로 여러 파일에 걸칠 수 있습니다:

```
import util/math          // 형제 파일 util/math.maca를 인라인
import { parse } from lexer   // 선택적 임포트: `parse`와 그것이 필요로 하는 것만
```

선택적 임포트(`import { … } from …`)는 지명된 정의와 그 의존 클로저만 끌어옵니다
— 죽은 코드는 빌드에 들어오지 않습니다.

## 외부 라이브러리

`import c "sqlite3.h"`는 시스템 툴체인을 통해 실제 C 라이브러리를 링크하고,
`import rust "gpui::div"`는 (`maca.toml`의 `[rust-dependencies]`와 함께) Rust
타깃으로 crates.io 크레이트 위에 쌓습니다. Maca는 자신이 겨냥하는 생태계의
섬이 아니라 좋은 시민이 되기를 지향합니다.

## 앞으로의 길

Maca는 스스로를 부트스트랩하는 중입니다: 컴파일러의 프런트엔드는 `selfhost/`
아래에서 Maca로 다시 쓰이고 있고, 바로 이 책도 Maca 프로그램인 **Tomo**가
빌드합니다. 목표는 스스로를 컴파일하고 스스로를 문서화하는 언어입니다 — Maca로.

읽어 주셔서 고맙습니다. 무언가를 만들어 보세요 — 프로그램이든, 머신이든,
아니면 둘 다든.
