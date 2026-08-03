# 컬러블라인드 Async

대부분의 언어는 함수를 두 가지 색, 동기와 `async`로 나누고, 그 색은
전염됩니다: 무언가를 `await`하려면 자신이 `async`여야 하고, 호출자도, 그
위로도 계속 그래야 합니다. Maca에는 이 분리가 없습니다. **`async` 키워드가
아예 없습니다.**

## async는 추론되는 이펙트

동시성은 여러분이 적어 넣는 키워드가 아니라 컴파일러가 추론하는 *이펙트*입니다.
이를 도입하는 연산은 셋입니다:

- `spawn f(x)`: `f(x)`를 동시에 실행하고 `Future a`를 돌려줍니다;
- `await fut`: future가 완료될 때까지 중단하고 그 값이 됩니다;
- `sleep_ms(ms)`: 중단 지점입니다.

이 중 무엇이든 쓰는 함수는 그 자체로 async입니다. 주석도, 색도 없습니다:

```maca
fetch_both(a: str, b: str) -> str {
    fa = spawn get(a)          // 두 요청이 동시에 실행됨
    fb = spawn get(b)
    await fa ++ await fb        // 합류
}
```

`fetch_both`는 자신을 async라 선언하지 않습니다. `spawn`/`await`를 쓰는 것으로
충분합니다. 그리고 `fetch_both`를 호출하는 평범한 함수도 색을 바꿀 필요가
없습니다. 그냥 호출하면 됩니다.

## 왜 중요한가

전파될 색이 없으므로 리팩터링이 편안합니다: 말단 함수를 동시성으로 바꾼다고
해서 모든 호출자를 다시 쓸 필요가 없습니다. 중단이 일어나든 아니든 같은 코드가
돌고, `await a + await b`는 그저 `(await a) + (await b)`입니다. `await`는
평범한 전위 연산자입니다.

## 진짜 동시성으로 컴파일된다

네이티브 경로에서 `spawn`/`await`/`sleep_ms`는 런타임의 pthread 기반 future로
내려갑니다. async 함수도 ABI 변화가 없는 평범한 함수입니다. 브라우저(JS 백엔드)
에서는 같은 연산이 이벤트 루프에 대응됩니다. 이펙트는 한 번만 쓰고, 각 백엔드가
그것을 실현합니다.

## 이펙트는 검사된다

이펙트는 추론될 뿐 아니라, 중요한 곳에서는 강제됩니다. *설정 모드*(Nix 타깃)
에서 async는 순수하지 않으므로 `await`/`spawn`/`sleep_ms`는 컴파일 에러입니다:
인프라 기술은 순수해야 합니다. 프로그램에서 편의였던 이펙트가 설정에서는
가드레일이 됩니다.

## 실행해 보기

```
maca run examples/async.maca
```

각각 50ms를 자는 작업 둘을 함께 spawn하고 await합니다.

```maca
slow_double(n: int) -> int {
    sleep_ms(50)
    n * 2
}

main() -> int {
    a = spawn slow_double(10)
    b = spawn slow_double(20)
    info("{await a + await b}")
    0
}
```

벽시계 시간은 100ms가 아니라 50ms쯤입니다. `spawn` 둘을 빼고 `slow_double`을
직접 호출해 보세요. 출력은 같고, 시간은 두 배이며, 시그니처는 하나도 바뀌지
않습니다.

## 전체 규칙은 어디에

async는 다섯 줄짜리 이펙트 시스템의 한 행이고, 나머지 행들이 config 모드를
안전하게 만드는 이유입니다. 레퍼런스의 [이펙트와 async](a7-effects.md)에 다섯
행 전부, 각각을 도입하는 것, `try`가 *덜어내는* 것, `await`와 `spawn`의
우선순위, 그리고 각 타깃에서 중단 지점이 무엇이 되는지가 있습니다.

다음: 설정을 위한 하나의 언어.
