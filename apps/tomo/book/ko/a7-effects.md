# 이펙트와 async

함수의 타입은 인자와 결과, 그리고 **그 함수가 하는 일**입니다. 세 번째가 이펙트
행입니다. 검사기가 본문에서 추론해 모든 호출자에게 퍼뜨리고, 소스에는 아무것도
적히지 않습니다. 가르치는 쪽은
[컬러블라인드 Async](13-colorblind-async.md)입니다.

## 다섯 개의 행

| 이펙트 | 도입하는 것 |
|---|---|
| `io` | `print`, `input`, 그리고 콘솔 가족(`info`, `warn`, `err`, `debug`, `notice`, `crit`, `alert`, `emerg`, `panic`)과 파일/스트림 메서드 `read`, `write`, `exists`, `remove`, `append`, `create` |
| `net` | `net`, `http`, `socket` 수신자를 통한 호출 |
| `os` | `os`, `process` 수신자를 통한 호출 |
| `async` | `await`, `spawn`, `sleep_ms` |
| `exn` | `fail`, 그리고 발생시킬 수 있는 모든 호출 |

식의 이펙트는 그 부분들의 합집합이므로, 함수의 행은 본문이 닿는 모든 것의
합집합입니다. 말단 함수에 `sleep_ms`를 넣으면 모든 호출자가 시그니처를 바꾸지
않은 채 `async`가 됩니다.

**`try`는 `exn`을 덜어냅니다.** 이펙트를 더하는 대신 *제거하는* 유일한
연산입니다.

## async는 추론되는 이펙트

- `spawn f(x)`: `f(x)`를 동시에 실행하고 `Future a`를 돌려줍니다;
- `await fut`: future가 완료될 때까지 중단하고 그 값이 됩니다;
- `sleep_ms(ms)`: 중단 지점입니다.

이 중 무엇이든 쓰는 함수는 그 자체로 async입니다.

```maca
fetch_both(a: str, b: str) -> str {
    fa = spawn get(a)          // 두 요청이 동시에 실행됨
    fb = spawn get(b)
    await fa ++ await fb        // 합류
}
```

`await`와 `spawn`은 **단항 우선순위의 전위 연산자**입니다. 모든 이항 연산자보다
강하게, 호출보다는 약하게 묶입니다.

## 왜 중요한가

전파될 색이 없으므로 말단 함수를 동시성으로 바꾼다고 해서 모든 호출자를 다시 쓸
필요가 없습니다. 대가는 시그니처를 읽고 그 호출이 중단될 수 있는지 알 수 없다는
것입니다.

## 진짜 동시성으로 컴파일된다

async 함수는 **ABI 변화가 없는** 평범한 함수입니다. 동시성 함수와 동기 함수는
호출 지점에서도 FFI 경계에서도 서로 바꿔 쓸 수 있습니다.

| 타깃 | 중단 지점이 되는 것 |
|---|---|
| 네이티브 (C) | `maca_spawn` / `maca_await` / `maca_sleep_ms`: 런타임의 pthread 기반 future. 중단 지점은 진짜 스레드 경계입니다 |
| JS | `await`, 그리고 그것에 닿는 모든 함수에 붙는 `async function`. 어느 함수가 그런지는 컴파일러가 알아내므로, 독자를 기다리는 핸들러도 기다리지 않는 핸들러와 똑같이 씁니다 |
| 플레이그라운드 | 인터프리터가 즉시 평가합니다. 출력은 같고 타이밍은 다릅니다 |
| Nix (설정) | 거부됩니다. 아래 참조 |

## 이펙트는 검사된다

[설정 모드](a12-config.md)에서 비어 있지 **않은** 이펙트 행은 무엇이든 컴파일
에러입니다.

```
EffectInConfig: config must be pure but this uses effect(s): async
```

메시지는 찾아낸 행을 전부 나열합니다. 특별 취급은 없어서, 설정은 파일을 읽어
자기가 선언할 내용을 정할 수 없습니다.

## freestanding 타깃이 거부하는 것

임베디드 타깃은 `io` 빌트인을 행이 아니라 이름으로 거부합니다. 베어메탈
이미지에는 콘솔이 없습니다. UART는 `mmio_write`로 직접 다루세요.
[타깃](a10-targets.md)을 보세요.
