# 외부 함수 인터페이스

Maca는 C로 컴파일되므로 C ABI는 이미 딛고 선 땅입니다. FFI는 `import` 하나입니다.

## C

```maca
import c "sqlite3.h"
```

헤더를 선언하고 라이브러리를 링크합니다. 그 안의 함수는 다른 함수와 똑같이
호출합니다.

```maca
import c "sqlite3.h"

main() -> int {
    info("sqlite {sqlite3_libversion()}")
    0
}
```

드라이버는 `nix`가 있으면 그것으로, 없으면 호스트의 `cc`와 시스템
헤더/라이브러리로 라이브러리를 찾습니다.
`apps/examples/ffi_sqlite.maca`가 진짜 데이터베이스를 열고 결과 집합을
순회합니다.

## 타입 매핑

| C | Maca | 비고 |
|---|---|---|
| `char*`, `const char*` | `str` | |
| 그 외 모든 포인터 | `int` | 불투명 핸들 |
| `float`, `double` | `float` | |
| `int`, `long`, `size_t`, `int32_t`, … | `int` | |
| `void` | `int` | |

문자열이 아닌 포인터는 불투명한 정수가 됩니다. Maca 안에서 역참조할 수 없고,
그것을 준 라이브러리에 되돌려 줄 뿐입니다. 핸들 기반 C API가 원하는 것이 보통
정확히 이것이고, FFI가 메모리를 망가뜨릴 통로를 새로 만들지 않게 해 줍니다.

## 선언 생성하기

```
maca bindgen /usr/include/sqlite3.h
maca bindgen /usr/include/sqlite3.h sqlite3.maca
```

인자가 하나면 출력하고, 둘이면 모듈을 씁니다. 완전한 C 파서가 아니라 프로토타입
스캐너입니다. 주석과 전처리기 줄을 걷어내고, `;`로 나누고, 각
`RET NAME(PARAMS)`를 Maca 선언으로 바꿉니다. typedef, struct, union, 함수
포인터는 추측하지 않고 건너뜁니다.

```maca
sqlite3_libversion() -> str
sqlite3_open(filename: str, ppDb: int) -> int
sqlite3_column_double(sqlite3_stmt: int, iCol: int) -> float
sqlite3_close(sqlite3: int) -> int
```

bindgen은 구현이 둘입니다. 컴파일러 안의 원래 Rust 구현과
`apps/bindgen/bindgen.maca`의 Maca 포팅본이고, 테스트가 같은 헤더로 둘을 돌려
출력이 정확히 일치할 것을 요구합니다.

## Python

```maca
import py "json"
```

Python 연동은 `python3-config`를 거칩니다. 인터프리터를 임베드하므로 둘 중
무거운 쪽이고, Python에만 있는 라이브러리에 닿기 위해 존재합니다.

## JavaScript: `maca` 브리지

`import js """…"""`는 블록을 그대로 `app.js`에 심습니다. `.maca` 사용자
인터페이스가 필요한 호스트 글루(WebAssembly 인스턴스, 에디터, 브라우저 API)를
두 번째 파일 없이 들고 다니는 방법입니다.

블록과 프로그램은 `maca`라는 객체 하나에서만 만납니다.

| 호출 | 하는 일 |
|---|---|
| `maca.get(name)` | 프로그램이 선언한 이름을 읽습니다 |
| `maca.set(name, value)` | 쓰고 나서 화면을 갱신합니다 |
| `maca.set({ a, b })` | 여럿을 쓰고 한 번만 갱신합니다 |
| `maca.refresh()` | 다른 것이 바뀐 뒤 바인딩된 노드를 다시 맞춥니다 |
| `maca.provide({ f })` | Maca 쪽이 선언한 함수를 건네줍니다 |

프로그램이 선언한 적 없는 이름은 새 필드가 아니라 오류입니다.

```
maca.set: `form_titel` is not state in this program; declared: form_title, form_url
maca.set: `Limit` is a constant
```

예전에는 블록이 `state.form_titel = …`처럼 직접 대입했고, 그러면 아무것도
바인딩되지 않은 필드가 조용히 생기고 예외도 나지 않아서 채우려던 대화상자가 그냥
빈 채로 남았습니다.

### 호스트가 주는 함수

본문 없이 선언한 함수가 반대 방향의 경계입니다. 시그니처는 Maca의 것이고,
구현은 호스트의 것입니다.

```maca
cfg_sections() -> Section[]
cfg_write(title: str, url: str, icon: str, section: str) -> bool

form_title = ""

import js """
maca.provide({
  cfg_sections: () => JSON.parse(localStorage.getItem("sections") || "[]"),
  cfg_write: (title, url, icon, section) => save(title, url, icon, section),
});

document.addEventListener("app:link", (e) => {
  maca.set({ form_title: e.detail.title });
});
"""
```

Maca에서 `cfg_sections()`는 평범한 호출이고, 백엔드가 그것을 브리지로 보냅니다.
아무도 구현하지 않은 것을 호출하면 그렇다고 말합니다.

```
maca: `cfg_write` is declared in Maca but nothing implements it;
call maca.provide({ cfg_write: … }) from the import js block
```

### 순서

생성된 파일은 브리지, 블록, 앱 순입니다. 그래서 `maca.provide`와 `maca.set`는
블록의 최상위에서 동작하고, 거기서 정한 값은 `mount()`가 화면을 처음 그리기 전에
이미 자리를 잡습니다.

생성된 `state` 객체와 `update()` 함수는 여전히 동작하지만, 약속이 아니라 이
백엔드의 지역 변수입니다. 문서화된 쪽은 `maca`입니다.

### 그 위에 올린 프로그램 하나

`apps/playground/playground.maca`가 본문 없는 함수 열다섯 개와 상태 이름 넷을
선언하고, 그것이 `import js` 블록에 닿을 수 있는 표면의 전부입니다. 경계 너머에
있는 것은 Maca로 적을 수 없는 브라우저의 능력뿐입니다. WebAssembly 인스턴스,
Monaco 에디터, URL 프래그먼트, 클립보드, 샌드박스 미리보기 iframe.

## 모듈로서의 브라우저

`modules/web`은 브라우저를 평범한 Maca로 내놓은 것입니다. 그중 셋은 필요한
함수를 몸통 없이 선언하고 자기 `import js` 블록에서 구현하는 다리입니다.

| 모듈 | 무엇에 닿는가 |
|---|---|
| `web/storage` | 방문과 방문 사이에 브라우저가 기억하는 것 |
| `web/time` | 지역 시계, 그리고 타이머에 맞춘 다시 그리기 |
| `web/file` | 독자에게 내려받기를 건네고, 파일을 되받기 |
| `web/format` | 시계가 어떻게 읽히고 내려받기가 어떤 이름을 갖는지. 호스트는 전혀 없습니다 |

### 남는 상태

```maca
import web/storage

config: Config = stored("homepage.config", data(Links))
locked = stored("homepage.locked", true)
```

`stored(key, default)`가 전부입니다. 그 이름은 브라우저가 `key` 아래 저장해 둔
것으로 **시작**하고, 저장된 것이 없으면 옆에 적힌 값으로 시작합니다. 그리고
**그 이름에 대입하면 다시 저장됩니다**.

```maca
lock() -> int {
    locked = !locked
    0
}
```

읽기 호출도, 쓰기 호출도, 키 상수도 없습니다. 대입은 이미 갱신이고, 이제
저장이기도 합니다. [UI 문법](a11-ui.md)을 보세요.

키는 직접 적거나 상수에 묶습니다. `const` 이름은 저장할 수 없습니다. 저장되는
이름은 대입될 때 바로 되쓰이는 이름이기 때문입니다.

생성된 `app.js`에 그대로 보입니다. 바인딩은 선언된 값을 그대로 지니고, 그 이름에
대한 모든 쓰기는 `local_store(key, …)`가 되며, 페이지가 처음 만들어지기 전에 그
이름을 되읽는 한 줄이 다리에 들어갑니다.

```js
maca.set("locked", local_start("homepage.locked", maca.get("locked")));
```

`local_start`, `local_store`, `local_forget`이 `web/storage`가 선언한 세
함수입니다. `import web/storage` 없이 쓴 `stored(…)`는 무슨 임포트를 더해야
하는지 이름으로 말해 주는 빌드 오류입니다.

### 독자가 고른 파일

`web/file`에는 호출이 둘 있습니다. `download(name, text)`는 파일을 건네고,
`pick_text(accept)`는 파일을 청해서 그 글을 답으로 줍니다.

```maca
import { pick_text } from web/file

import_config() {
    text = await pick_text("application/json")

    if text == "" {
        return
    }

    next: Config = decode(text)
    commit(next, "imported")
}
```

고르개는 곧바로 답할 수 없으므로 그 호출은 중단점이고, 그것을 읽는 것이
`await`입니다. 독자가 아무것도 고르지 않으면 `""`입니다. `import_config`는
스스로를 async라 선언하지 않으며, 그것을 부르는 단추도 그렇습니다.
[async는 색이 아니라 효과입니다](a7-effects.md). 기다리기 전에 쓴 것은 기다리는
동안 화면에 있고, 그 뒤에 쓴 것은 답이 닿을 때 다시 그려집니다.

### 브라우저 밖에서

구현이 `import js` 블록인 모듈은 다른 어디에서도 돌릴 것이 없습니다. 다른
타깃으로 빌드하면 아무것도 하지 않는 프로그램으로 컴파일되는 대신 **이름을 대며**
거절합니다.

```
`web/storage` runs in a browser: what implements it is an `import js` block,
and the native target has no JavaScript to run it in; build the page with
`maca build --target js`
```

`web/format`이 따로 한 파일인 이유가 이것입니다. 시계의 자릿수 채움과 내려받기
파일 이름은 거기서 평범한 Maca로 정해지므로, `modules/web/tests/`는 `maca test`가
다른 것과 똑같이 돌리는 스위트입니다.

## 언제 FFI를 쓸까

일이 자체 완결적이면 Maca로 구현하고, 라이브러리 자체가 가치인 곳(SQLite, 압축
코덱, 시스템 API)에 FFI를 쓰세요. 모든 FFI 호출은 타입 검사기가 도와줄 수 없는
지점입니다. 반대편의 타입은 검사된 것이 아니라 주장된 것이니까요.

## 반대 방향

Maca는 다른 언어를 호출하는 대신 다른 언어로 컴파일될 수도 있습니다.
`--target rust`는 crates.io를 쓸 수 있는 Rust 소스를, `--target jvm`은 Java를,
`--target js`는 JavaScript를 내보냅니다. 필요한 생태계가 그중 한 플랫폼에 있다면
바인딩하는 것보다 그쪽을 타깃하는 편이 나은 경우가 많습니다.
[타깃](a10-targets.md)이 여섯 개 전부를 다룹니다.
