# 설치하기

Maca는 바이너리 하나와 언어 서버 하나입니다.

## 설치기를 받아서 실행하기

[최신 릴리스](https://github.com/pleahmacaka/macalang/releases/latest)에서
플랫폼에 맞는 설치기를 고르세요.

```sh
curl -fsSL -O https://github.com/pleahmacaka/macalang/releases/latest/download/maca-install-linux-x86_64
chmod +x maca-install-linux-x86_64
./maca-install-linux-x86_64
```

Apple 실리콘 맥이면 `macos-aarch64`, ARM 서버면 `linux-aarch64`, Windows에서는
`maca-install-windows-x86_64.exe`입니다.

`maca`와 `maca-lsp`를 `~/.local/bin`에 넣고, 그 디렉터리가 `PATH`에 없으면
추가할 줄을 알려줍니다. 플래그는 둘입니다.

```sh
./maca-install-linux-x86_64 --prefix /usr/local   # 다른 위치로
./maca-install-linux-x86_64 --version 0.3.0       # 예전 릴리스로
```

환경 변수 `PREFIX`와 `MACA_VERSION`도 같은 둘을 정합니다.

마지막으로 표준 라이브러리를 import하는 작은 프로그램을 컴파일하고 실행합니다.
설치한 컴파일러가 실제로 무언가를 컴파일하기 전까지는 성공했다고 말하지
않습니다.

## GitHub Actions에서

```yaml
- uses: pleahmacaka/macalang@main
- run: maca build
```

러너에 맞는 설치기를 받아 실행한 뒤 `maca install`을 돌립니다. `maca.toml`이
이름을 댄 것들을 `maca.lock`이 고정한 버전으로 가져옵니다.

## 그 밖에 필요한 것

**C 컴파일러.** Maca는 C를 거쳐 컴파일되므로 `maca build`와 `maca run`이
파이프라인 끝에서 `cc`나 `clang`을 부릅니다. macOS는 Xcode command line tools,
Debian/Ubuntu는 `build-essential`, Fedora는 `gcc`입니다.

**Nix는 두 가지에만.** `maca dev`와 Nix 타깃이 [Nix](https://nixos.org)를
씁니다. Nix는 Windows 네이티브 빌드가 없어서, Windows에서는 `maca dev`가 WSL
아래에서 돕니다.

**Rust는 컴파일러를 직접 빌드할 때만.** 체크아웃에서 `cargo build`로 빌드하는
경로만 Rust 툴체인이 필요합니다.

## 잘 됐는지 확인하기

```sh
maca --version
```

그다음 존재하는 가장 짧은 진짜 프로그램입니다.

```sh
echo 'main() -> int {
    info("Hello, World")
    0
}' > hello.maca
maca run hello.maca
```

`Hello, World`가 찍히면 컴파일러, C 툴체인, 런타임이 모두 제자리에 있는
것입니다. `maca run`이 파이프라인 전체를 거치므로 `--version`보다 나은 확인
방법입니다.

## 에디터 지원

언어 서버는 이미 설치돼 있고, 에디터 쪽 설정만 남았습니다.

**Zed**라면 저장소의 `apps/editor/zed-maca`에 확장이 있습니다. tree-sitter
문법, 강조, 아웃라인, 언어 서버 연결이 되어 있습니다. *Extensions → Install Dev
Extension*을 고르고 그 디렉터리를 지정하세요.

LSP를 말하는 다른 에디터라면 `*.maca` 파일에 대해 `maca-lsp` 바이너리를
가리키면 됩니다. 진단, 호버, 정의로 이동, 참조 찾기, 문서 심볼, 시그니처
도움말, 자동완성, 이름 변경, 포매팅을 제공합니다.

## 아무것도 설치하지 않고

[플레이그라운드](../play/)는 같은 컴파일러를 WebAssembly로 빌드해 브라우저
안에서 돌립니다.

## 최신으로 유지하기

```sh
maca upgrade
```

가장 최근 릴리스를 받아 바이너리를 제자리에서 교체합니다.
