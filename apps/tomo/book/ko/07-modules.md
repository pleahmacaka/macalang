# 모듈

프로그램은 금방 파일 하나에 담기지 않게 됩니다. Maca의 답은 일부러 작습니다.
파일 하나가 모듈 하나이고, `import`가 그것을 끌어옵니다.

## 파일이 곧 모듈

모듈 선언도, `package` 줄도, `mod` 블록도 없습니다. 함수들을 `geometry.maca`에
넣으면 그것이 `geometry` 모듈입니다.

```maca
// geometry.maca
Point = {
    x: int
    y: int
}

origin() -> Point =>
    Point { x = 0, y = 0 }

dist2(a: Point, b: Point) -> int =>
    (a.x - b.x) * (a.x - b.x) + (a.y - b.y) * (a.y - b.y)
```

```maca
// main.maca
import geometry

main() -> int {
    p = origin()
    info("{dist2(p, Point { x = 3, y = 4 })}")
    0
}
```

`origin`을 수식 없이 호출한 점을 보세요. import는 이름을 평평하게 들여옵니다.
`geometry.origin()` 형태는 없습니다. 수식할 네임스페이스가 없기 때문입니다.

중첩 경로는 `/`를 쓰고, 디렉터리 구조와 일치합니다.

```maca
import std/str
import app/models/user
```

`import a/b`는 import하는 파일 옆에서 `a/b.maca`를 찾습니다. import는
전이적입니다. `a/b`가 `a/c`를 import하면 둘 다, 의존 순서대로 끌려옵니다.

## import가 실제로 하는 일

**인라인**합니다. import된 모듈의 정의들이 타입 검사 전에 프로그램 안으로
끼워 넣어집니다. 별도의 컴파일 단위도, 모듈별 오브젝트 파일도, 당신의 모듈들
사이의 링크 단계도 없습니다.

알아둘 결과가 하나 있습니다. import되고 나면 이름은 전역입니다. `parse`를 둘 다
정의하는 두 모듈은 충돌합니다. 큰 프로그램에서는 관례적으로 접두사를 붙이거나
(`json_parse`, `toml_parse`) 더 쪼개세요.

## 필요한 것만 가져오기

함수 하나 쓰자고 모듈 전체를 import하면 그 모듈이 정의한 모든 것이 딸려옵니다.
선택적 import가 이를 해결합니다.

```maca
import { origin, dist2 } from geometry
```

`origin`과 `dist2`만 넘어옵니다. 더불어 전이적으로, 같은 모듈에서 *그것들이*
참조하는 것들도요. `origin`은 `Point`를 반환하므로 `Point`가 자동으로 따라옵니다.
함수가 언급하는 타입을 일일이 나열할 필요가 없습니다.

모듈 경계에서의 죽은 코드 제거입니다. 큰 모듈이라면 전부 컴파일하는 것과 쓰는
것만 컴파일하는 것의 차이입니다.

모듈이 정의하지 않은 이름을 대면 나중에 C 컴파일러가 발견하는 매달린 참조가
아니라 깔끔한 에러가 납니다.

```
maca: import { centroid } from geometry: 'centroid' is not defined in that module
```

## 외부 언어 import

앞에 언어 이름을 붙인 `import`는 Maca 바깥으로 나갑니다.

```maca
import c "sqlite3.h"
import py "json"
import js """…"""
import css """…"""
```

앞의 둘은 진짜 라이브러리를 링크합니다. [FFI 레퍼런스](a13-ffi.md)에서
다룹니다. 뒤의 둘은 JavaScript
백엔드를 위해 원문 텍스트를 심습니다. `.maca` 사용자 인터페이스가 자기 호스트
글루와 스타일시트를 인라인으로 들고 다닐 수 있게요. 삼중 따옴표 문자열을
받는데, 여러 줄에 걸치고 보간을 하지 않으므로 CSS의 중괄호를 이스케이프할 필요가
없습니다.

## 여러 파일짜리 프로그램 빌드

`main`이 있는 파일을 드라이버에 넘기세요.

```
maca build app/main.maca
```

import로 닿는 모든 것이 함께 컴파일됩니다. 소스를 나열하는 매니페스트도,
관리할 빌드 그래프도 없습니다. 셀프호스팅 컴파일러가 정확히 이렇게 빌드됩니다.
`maca build selfhost/main.maca`가 import 목록으로부터 프론트엔드 전체를
컴파일합니다.

## 모듈 시스템이 멈추는 지점

가시성 수식자가 없습니다. 모듈이 정의한 모든 것은 import 가능합니다. 파일을
넘어서는 네임스페이스도 없습니다. 버전이 붙은 모듈 레지스트리도 없습니다.

저장소 하나에 들어가는 프로그램을 위한 작은 언어이고, 모듈 시스템도 거기에 맞춰
크기를 잡았습니다. 이것이 바뀐다면 선택적 import 방향으로 바뀔 것입니다. 이미
키워드 없이 `pub`의 "명시적 표면적" 이점을 주고 있으니까요.

## 실행해 보기

디렉터리 하나에 위의 `geometry.maca`를 놓고, 그 옆에 이 장 앞부분의
`import geometry` 버전을 `main.maca`로 놓으세요. 그다음:

```
maca run main.maca
```

명령 하나, 파일 둘, 매니페스트 없음. 이제 `geometry.maca`를 `modules/`
하위 디렉터리로 옮기고 같은 명령을 다시 실행해 보세요. 그래도 해결됩니다.
`modules/`가 탐색 루트이기 때문입니다.

## 전체 규칙은 어디에

레퍼런스의 [모듈과 레이아웃](a9-modules.md)에 해결 순서가 전부 있습니다. 어떤
디렉터리를 어떤 순서로 뒤지는지, 탐색이 어디서 멈추는지, 그리고 "프로그램 옆의
파일" 규칙이 왜 처음이 아니라 마지막인지까지요.

다음: 이 값들이 사는 메모리에 무슨 일이 일어나는가.
