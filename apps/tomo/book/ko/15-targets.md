# 프로그램이 도착하는 곳

같은 소스가 네이티브 바이너리로도, 브라우저 페이지로도, JVM으로도, Rust로도,
마이크로컨트롤러로도, 머신의 설정으로도 컴파일됩니다. 어느 쪽이 나오는지는
플래그가 정합니다.

## 플래그 여섯

```
maca build app.maca -o app                 # 네이티브 바이너리 (기본값)
maca build app.maca --target js -o out     # 페이지
maca build app.maca --target jvm           # Java 소스
maca build app.maca --target rust          # Rust 소스
maca build app.maca --target embedded --mcu cortex-m0
maca build app.maca --target nix           # 머신의 설정
```

## 손이 먼저 가야 할 것은 네이티브

플래그가 없으면 Maca는 C를 거쳐 컴파일하고 정적 바이너리를 링크합니다. 배포할
런타임도, 인터프리터도, 수집기도 없습니다. 생성된 C는 시스템에 있는 컴파일러로
넘어갑니다.

질문은 프로그램 바깥의 무언가가 답을 정할 때만 생깁니다. 브라우저, 칩, Java
API, 포팅에 한 달 걸릴 라이브러리 같은 것들이요.

## 나머지는 무언가에 닿기 위해 있습니다

| 타깃 | 닿는 곳 |
|---|---|
| `js` | 브라우저. UI 문법이 살아 있는 DOM이 됩니다 |
| `jvm` | Java의 라이브러리. 예제는 Minecraft 모드입니다 |
| `rust` | 내보낸 Rust 소스를 통해 crates.io |
| `embedded` | libc도 없는 베어메탈 Cortex-M / RISC-V |
| `nix` | 그 바이너리가 돌아갈 머신 |

각각은 Maca가 그러지 않으면 다시 구현해야 할 생태계에 닿기 때문에 목록에
있습니다. 사람들이 요청하는 BEAM이 없는 이유는 그것이 도달 범위가 아니라
우아함 때문에 추가되는 첫 백엔드가 될 것이기 때문입니다.
[컬러블라인드 async](13-colorblind-async.md)는 이미 C 런타임의 진짜 스레드
위에서 돌아갑니다.

## 실행해 보기

지금까지 쓴 프로그램 아무거나 두 번 빌드해 보세요.

```
maca build hello.maca -o hello
maca build hello.maca --target rust -o hello.rs
```

두 번째는 읽을 수 있는 Rust 소스입니다. `hello.maca`는 바뀌지 않았습니다.

## 전체 규칙은 어디에

레퍼런스의 [타깃](a10-targets.md)에 플래그 전부, 임베디드 타깃의 MMIO 어휘,
네이티브 양쪽이 합의하는 C ABI, 그리고 각 타깃이 **거부하는** 것의 정확한
목록이 있습니다.

`js` 타깃이 살려 내는 UI 문법은 [앞 장](15-ui.md)이고, Nix 출력은
[설정 모드](14-config-mode.md)입니다.
