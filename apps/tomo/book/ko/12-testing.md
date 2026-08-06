# 테스트

테스트는 검사할 코드 옆, 같은 파일에 두고, 도구가 이름으로 찾아냅니다.

## 테스트 쓰기

이름이 `test_`로 시작하는 함수는 테스트이고, 무엇을 확인하는지는 `assert` /
`assert_eq`로 씁니다.

```maca
Counter = {
    n: int
}

bump(c: Counter) -> Counter =>
    c with { n = c.n + 1 }

test_bump_increments_by_one() {
    c = bump(Counter { n = 1 })
    assert_eq(str(c.n), "2", "원래 값보다 하나 큼")
}

test_bump_is_not_in_place() {
    c = Counter { n = 1 }
    bump(c)
    assert_eq(str(c.n), "1", "원본은 그대로")
}
```

테스트는 반환 타입을 선언하지 않고 아무것도 반환하지 않습니다. 실패한 단언이
하나도 없으면 통과입니다.

테스트 이름은 호출하는 함수가 아니라 **무엇을 보장하는지**로 짓습니다.
`test_bump_is_not_in_place`는 실패했을 때 무엇이 깨졌는지 알려주고,
`test_bump`는 어디서부터 찾아보라고만 알려줍니다.

## 두 개의 단언

| 호출 | 통과 조건 |
|---|---|
| `assert(cond, message)` | `cond`가 참 |
| `assert_eq(got, want, message)` | `got == want` (둘 다 `str`) |

세 번째 인자에는 식을 다시 쓰지 말고 그 식이 **무엇을 보장하려던 것인지**를
적어서, 실패가 문장으로 읽히게 하세요.

```
assertion failed: the two disagree
  got:  got
  want: want
```

`assert_eq`는 문자열을 비교하므로 숫자는 들어갈 때 `str(n)`이 됩니다.

## 실행

```
maca test counter.maca
```

```
running 2 tests
  test_bump_increments_by_one
    ok
  test_bump_is_not_in_place
    ok
2 tests passed
```

드라이버가 파일 안의 `test_` 접두사 함수를 전부 모으고, 파일에 `main`이 있으면
그것은 빼고, 각 테스트를 호출하기 전에 이름을 알리는 러너를 생성합니다.

종료 코드는 실패한 단언의 개수라, `maca test`는 종료 코드를 읽는 무엇과도 그대로
이어 붙습니다.

## 단언이 실패해도 실행은 멈추지 않습니다

모든 단언이 실행되고, 실패는 세어집니다.

```
assertion failed: the two disagree
  got:  got
  want: want
assertion failed: one is not greater than two
running 3 tests
  test_a_failure_shows_both_sides
    FAILED
  test_a_bare_assertion_shows_its_message
    FAILED
  test_a_passing_one_still_runs
    ok
2 assertion(s) failed
```

첫 실패에서 멈추면 스위트를 고치는 데 버그 수만큼 실행이 필요하지만, 세어 두면
한 번의 실행이 전부를 알려줍니다.

`failures()`는 그 누적 개수를 반환합니다. 실패한 전제 조건 때문에 의미가 없어진
작업을 테스트가 스스로 건너뛸 때 쓸 수 있습니다.

## 파일을 넘나드는 테스트

테스트는 지정한 파일에서 찾고, 테스트 파일은 검사할 모듈을 import할 수 있습니다.

```maca
import geometry

test_origin_is_the_zero_point() {
    p = origin()
    assert_eq("{p.x},{p.y}", "0,0", "두 좌표 모두 0에서 시작")
}
```

`std/`가 이렇게 테스트됩니다. `std/tests/path.maca`는 `std/path`만 import하므로,
스위트는 정확히 그 모듈의 공개 표면을 정문으로 통과하며 실행한 것이 됩니다.

## 더 큰 요점: 문서를 실행하세요

`apps/examples/handbook.maca`에는 이 책이 하는 실행 가능한 주장
전부([레코드](05-records.md)의 갱신,
[기본 개념](03-common-concepts.md)의 포맷 스펙,
[합타입](06-sum-types.md)의 리스트 패턴)가 들어 있고, 테스트 스위트가 이를
실행해 출력 각 줄을 검사합니다.

그 파일이 존재하는 이유는 이 핸드북을 쓰다가 뭔가가 깨졌기 때문입니다. 실제로
실행해 보는 것만으로 컴파일러 버그 다섯 개와 존재하지 않는 명령 하나가
나왔습니다.

- 반환 타입을 선언하지 않은 함수가 본문을 버렸습니다
- 선언되지 않은 반환 타입 때문에 호출자가 결과를 변환하지 못했습니다
- 리스트 메서드가 이름 붙은 함수를 거부하고 람다만 받았습니다
- 빈 리스트에 대한 패턴이 없었습니다
- `maca test`가 문서에는 있는데 실제로는 없었습니다
- 문자열 안의 리터럴 `{`가 파일의 나머지를 조용히 삼켰습니다

실행하지 않은 문서는 사실이 아니라 주장입니다. 그것을 사실로 만드는 가장 값싼
방법은 테스트 스위트가 실행하는 파일에 넣는 것입니다.

같은 논리가 테스트 자체에도 적용됩니다. 결과를 출력하고 그 출력을 다른 무언가가
grep해서 검사하는 테스트는 출력을 테스트하고 있는 것입니다. 언어 안에서
단언하고, 종료 코드가 판정이 되게 하세요.
