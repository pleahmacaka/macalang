# 설정 모드

Nix 타깃의 규칙입니다. 입문은
[설정을 위한 하나의 언어](14-config-mode.md)입니다.

## 모드 선택

모드는 파일 안의 키워드가 아니라 타깃을 따라갑니다.

| 방법 | 모드 |
|---|---|
| `maca build host.maca --target nix` | 설정 |
| `maca.toml`의 `[hosts.X]` | 설정 |
| `maca.toml`의 `[[bin]]`, 또는 타깃 없음 | 프로그램 |
| `dev.maca`에 대한 `maca dev` | 설정. flake 방출기와 함께 |

## 설정은 값이다

설정 모듈은 레코드가 아니라 **옵션 경로에 대한 최상위 대입**입니다.

```maca
import nixpkgs

networking.hostName = "rigel"
system.stateVersion = "24.11"

system.packages = git, curl, htop, ripgrep
```

`import nixpkgs`가 패키지 집합을 들여옵니다. 중첩된 옵션 집합은 `=` 오른쪽에
레코드로 씁니다.

```maca
services.openssh = {
    passwordAuthentication = false
}
```

출력은 평범한 NixOS 모듈입니다.

```nix
{ config, pkgs, lib, ... }:
{
  networking.hostName = "rigel";
  system.stateVersion = "24.11";
  environment.systemPackages = [ pkgs.git pkgs.curl pkgs.htop pkgs.ripgrep ];
  services.openssh = {
    enable = true;
    passwordAuthentication = false;
  };
}
```

차이는 그 파일이 먼저 타입 검사기와 이펙트 검사기를 통과했다는 점뿐입니다.

### 두 가지 재작성

| 쓰는 것 | 나오는 것 |
|---|---|
| `system.packages = a, b` | `environment.systemPackages = [ pkgs.a pkgs.b ]` |
| `services.X = { … }` | 같은 블록에 `enable = true;`가 붙은 것 |

서비스를 설정하는 것이 곧 그 서비스를 요청하는 것이므로 enable은 조건 없이
주입됩니다. 블록 안에 `enable`을 직접 쓰면 두 번 나오고, 같은 속성이 반복된
것을 Nix는 받아들이지 않습니다.

## 설정 모드가 금지하는 것

설정 모드는 *순수*합니다. 프로그램에서는 괜찮던 이펙트가 여기서는 골라낸 목록이
아니라 **전부** 에러입니다.

```
EffectInConfig: config must be pure but this uses effect(s): async
```

| 손을 뻗은 것 | 행 | 결과 |
|---|---|---|
| `await`, `spawn`, `sleep_ms` | `async` | 컴파일 에러 |
| `info`, `print`, 그리고 콘솔 계열 | `io` | 컴파일 에러 |
| `x.read(…)`, `x.write(…)` 등 파일 메서드 | `io` | 컴파일 에러 |
| `net`/`http`/`socket` 호출 | `net` | 컴파일 에러 |
| `os`/`process` 호출 | `os` | 컴파일 에러 |
| `fail` | `exn` | 컴파일 에러 |

메시지는 찾아낸 행을 전부 나열합니다. 자기가 선언할 내용을 정하려고 파일을 읽는
머신 정의는, 언제 실행했느냐에 따라 의미가 달라지는 머신 정의입니다.

왼쪽 열은 모양의 목록입니다. 자유 빌트인 `read_file`, `capture`, `exec`는 어느
행에도 없으므로 그것을 호출하는 설정은 컴파일됩니다. 허가가 아니라 검사가 닿는
범위로 읽으십시오. 이 검사는 [이펙트와 async](a7-effects.md)의 이펙트 시스템을
함수 대신 모듈 전체에 겨눈 것입니다.

## 옵션은 네임스페이스 단위로 검사됩니다

컴파일러는 NixOS 옵션 루트를 알고 있습니다. `networking`, `services`,
`system`, `users`, `environment`, `programs`, `boot`, `hardware`, `security`,
`nix`, `fonts`와 그 형제들입니다.

```
UnknownOption: unknown NixOS option namespace `servicez`
```

검증되는 것은 **네임스페이스**이지 잎이 아닙니다. `services.nginx.enabl`의
오타는 Nix까지 가서 평가 시점에 거부됩니다.

`maca dev`는 이 진단을 억제하는 유일한 호출자입니다. `dev.*`는 NixOS
네임스페이스가 아니기 때문입니다.

## 개발 셸

`maca dev`는 `dev.maca`를 설정 모드로 읽고 자체 완결적인 `flake.nix` devShell을
내보냅니다.

```maca
dev.name = "myproject"
dev.packages = zig, nix, ripgrep
dev.env = { RUST_LOG = "debug" }
dev.shellHook = "echo ready"
```

Windows에는 Nix가 없으므로, `scoop.*`/`choco.*`/`winget.*` 패키지를 선언한
설정은 `.maca/dev/{setup,activate}.ps1`도 함께 받습니다. flake는 그
네임스페이스들을 무시합니다.

## 왜 언어를 공유하는가

한 번 정의한 포트 번호가 서버가 바인딩하는 그 상수이자 방화벽이 여는 그
상수입니다. 이것을 편리한 정도가 아니라 안전하게 만들어 주는 검사가 이펙트
행입니다.
