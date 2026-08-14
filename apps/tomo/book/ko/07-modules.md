# 모듈

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

import는 이름을 평평하게 들여옵니다. `geometry.origin()` 형태는 없습니다.

중첩 경로는 `/`를 쓰고, 디렉터리 구조와 일치합니다.

```maca
import std/str
import app/models/user
```

`import a/b`는 import하는 파일 옆에서 `a/b.maca`를 찾습니다. import는
전이적입니다.

## import가 실제로 하는 일

**인라인**합니다. import된 모듈의 정의들이 타입 검사 전에 프로그램 안으로
끼워 넣어집니다. 별도의 컴파일 단위도 링크 단계도 없습니다.

이름은 전역이라 `parse`를 둘 다 정의하는 두 모듈은 충돌합니다. 접두사를
붙이거나(`json_parse`, `toml_parse`) 더 쪼개세요.

## 필요한 것만 가져오기

```maca
import { origin, dist2 } from geometry
```

그 둘과, 전이적으로 *그것들이* 참조하는 것들까지 넘어옵니다. `origin`은
`Point`를 반환하므로 `Point`가 자동으로 따라옵니다.

모듈이 정의하지 않은 이름을 대면 깔끔한 에러가 납니다.

```
maca: import { centroid } from geometry: 'centroid' is not defined in that module
```

## 외부 언어 import

앞에 언어 이름을 붙인 `import`는 Maca 바깥으로 나갑니다.

```maca
import "sqlite3.h"
import "json.py"
import "app.js" """…"""
import "app.css" """…"""
```

앞의 둘은 진짜 라이브러리를 링크하고([FFI](a13-ffi.md)), 뒤의 둘은 JavaScript
백엔드를 위해 원문 텍스트를 심습니다.

## 여러 파일짜리 프로그램 빌드

`main`이 있는 파일을 드라이버에 넘기세요.

```
maca build app/main.maca
```

import로 닿는 모든 것이 함께 컴파일됩니다. 매니페스트도 빌드 그래프도
없습니다.

## 모듈 시스템이 멈추는 지점

가시성 수식자가 없습니다. 모듈이 정의한 모든 것은 import 가능합니다. 파일을
넘어서는 네임스페이스도, 버전이 붙은 모듈 레지스트리도 없습니다.

## 실행해 보기

위의 `geometry.maca`와 `import geometry` 버전의 `main.maca`를 한 디렉터리에
놓으세요.

```
maca run main.maca
```

이제 `geometry.maca`를 `modules/` 하위 디렉터리로 옮기고 다시 실행해 보세요.
`modules/`가 탐색 루트이므로 그래도 해결됩니다.

## 전체 규칙은 어디에

레퍼런스의 [모듈과 레이아웃](a9-modules.md)에 어떤 디렉터리를 어떤 순서로
뒤지는지가 전부 있습니다.
