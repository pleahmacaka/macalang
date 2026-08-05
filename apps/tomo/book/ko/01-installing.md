# 설치하기

Maca는 바이너리 하나와 언어 서버 하나입니다. 설치는 한 줄이면 됩니다.

## macOS와 Linux

```sh
curl -fsSL https://raw.githubusercontent.com/pleahmacaka/macalang/main/install.sh | bash
```

플랫폼에 맞는 `maca`와 `maca-lsp` 빌드본을 받아 `~/.local/bin`에 넣습니다. 그
디렉터리가 `PATH`에 없으면 설치 스크립트가 알려주고, 추가할 줄까지 보여줍니다.

체크아웃한 저장소에서도 같은 스크립트가 동작하고, `PREFIX`로 설치 위치를
정합니다.

```sh
./install.sh                     # ~/.local/bin
PREFIX=/usr/local ./install.sh   # /usr/local/bin, sudo가 필요할 수 있음
```

## Windows

PowerShell에서:

```powershell
irm https://raw.githubusercontent.com/pleahmacaka/macalang/main/install.ps1 | iex
```

바이너리는 `%USERPROFILE%\.local\bin`으로 가고, `$env:PREFIX`로 옮길 수
있습니다.

## 그 밖에 필요한 것

**C 컴파일러.** 진짜로 필요한 건 이것 하나입니다. Maca는 C를 거쳐 컴파일되므로
`maca build`와 `maca run`이 파이프라인 끝에서 `cc`나 `clang`을 부릅니다. macOS는
Xcode command line tools가 제공하고, Debian/Ubuntu는 `build-essential`,
Fedora는 `gcc`입니다. 툴체인의 나머지는 없어도 돌아가지만, 가장 많이 쓸 두
명령은 그렇지 않습니다.

**Rust는 가끔만.** 설치 스크립트는 플랫폼에 맞는 빌드본이 있으면 받아 쓰고,
없으면 소스에서 빌드합니다. `cargo`가 필요한 건 그 후자뿐입니다.

**Nix는 두 가지에만.** `maca dev`와 Nix 타깃이 [Nix](https://nixos.org)를
씁니다. 없으면 설치 스크립트가 Determinate Systems 설치기로 받아줄지 물어보고,
거절해도 그 둘 말고는 전부 동작합니다. 자동 실행이라면 `MACA_INSTALL_NIX=1`로
설치, `MACA_INSTALL_NIX=0`으로 건너뛰기를 미리 정할 수 있습니다.

Nix는 Windows 네이티브 빌드가 없어서, Windows에서는 `maca dev`가 WSL 아래에서
돕니다. Windows 설치 스크립트는 이걸 알고 있어서 묻지 않습니다.

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

언어 서버는 이미 설치돼 있습니다. 에디터 쪽 설정은 별도 단계입니다.

**Zed**라면 저장소의 `apps/editor/zed-maca`에 확장이 들어 있습니다. tree-sitter
문법, 강조, 아웃라인, 언어 서버 연결이 되어 있습니다. 체크아웃한 저장소에서
*Extensions → Install Dev Extension*을 고르고 그 디렉터리를 지정하세요.

LSP를 말하는 다른 에디터라면 `*.maca` 파일에 대해 `maca-lsp` 바이너리를
가리키면 됩니다. 진단, 호버, 정의로 이동, 참조 찾기, 문서 심볼, 시그니처
도움말, 자동완성, 이름 변경, 포매팅을 제공합니다.

## 아무것도 설치하지 않고

[플레이그라운드](../play/)는 브라우저 안에서 컴파일러를 돌립니다. WebAssembly로
빌드한 같은 컴파일러입니다. 그래서 툴체인을 설치할지 정하기 전에 다음 몇 장을
먼저 따라가 볼 수 있습니다.

## 최신으로 유지하기

```sh
maca upgrade
```

가장 최근 릴리스를 받아 바이너리를 제자리에서 교체합니다.
