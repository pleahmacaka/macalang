# 설정을 위한 하나의 언어

프로그램에 쓰는 것과 똑같은 Maca가 인프라도 기술합니다. **설정 모드**에서
프로그램은 바이너리 대신 Nix로 컴파일됩니다 — 머신의 정의가 문자열투성이 YAML
파일이 아니라 타입 검사를 거친 코드가 되는 것입니다.

## 설정은 값이다

```maca
import nixpkgs

networking.hostName = "rigel"
system.stateVersion = "24.11"

system.packages = git, curl, htop, ripgrep

services.openssh = {
    passwordAuthentication = false
}
```

평범한 Maca입니다. 대입, 대괄호 없는 콤마 리스트, 레코드. `--target nix`로
빌드하면 NixOS 모듈이 나옵니다.

```
maca build host.maca --target nix -o host.nix
```

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

Nix가 이것을 평가해 시스템을 구성합니다.

들어간 대로 나오지 않은 줄이 둘 있습니다. `system.packages`는 NixOS의 이름인
`environment.systemPackages`로 바뀌고, 설정을 한 `services.X` 블록에는 쓰지
않은 `enable = true`가 붙습니다. 서비스를 켜는 일과 설정하는 일을 따로 하지는
않으니까요. 두 재작성은 [설정 모드](a12-config.md)에 정리되어 있습니다.

## 설정 모드가 금지하는 것

설정 모드는 *순수*합니다. 설정은 상태를 기술할 뿐 동작을 수행하지 않습니다.
그래서 프로그램에서는 괜찮던 이펙트가 여기서는 에러가 됩니다.

- `await`/`spawn`/`sleep_ms` — async는 비순수 → **컴파일 에러**.
- I/O, 네트워크, 프로세스에 손대는 것 — 비순수 → **컴파일 에러**.

컴파일러는 [앞 장](13-colorblind-async.md)의 이펙트 시스템으로 이를
강제합니다.

```
EffectInConfig: config must be pure but this uses effect(s): async
```

머신 정의에 부수효과를 실수로 끼워 넣을 수 없습니다.

## 옵션 이름은 검사된다

NixOS 모듈이 대입할 수 있는 옵션 네임스페이스를 컴파일러가 알고 있으므로,
존재하지 않는 네임스페이스는 내 책상에서 잡힙니다.

```
UnknownOption: unknown NixOS option namespace `servicez`
```

`UnknownOption`과 `EffectInConfig`은 타입 불일치와 같은 자격의 평범한 컴파일
에러입니다.

## 실행해 보기

저장소에 진짜 설정이 `examples/system.maca`로 들어 있습니다. 빌드해서 나온
Nix를 읽어 보세요.

```
maca build examples/system.maca --target nix -o system.nix
```

그다음 사본에 `delay = sleep_ms(10)`을 넣고 다시 빌드해 보세요. 그때 나오는
에러가 이 장의 요점입니다.

## 왜 언어를 공유하는가

프로그램과 설정이 같은 언어이므로 타입을 공유하고, 도구를 공유하며, 값까지
공유할 수 있습니다. 한 번 정의한 포트 번호가 서버가 바인딩하는 그 상수이자
방화벽이 여는 그 상수입니다 — "앱"과 "앱이 도는 상자" 사이에 어긋남이 없습니다.

## 전체 규칙은 어디에

레퍼런스의 [설정 모드](a12-config.md)에 모드가 어떻게 선택되는지, 이펙트 표
전체, 옵션 검사가 정확히 어디까지 미치는지(한쪽으로는 짐작보다 멀고 다른
쪽으로는 짧습니다), 그리고 같은 기계 장치를 호스트가 아니라 개발 셸에 겨눈
`maca dev`가 있습니다.
