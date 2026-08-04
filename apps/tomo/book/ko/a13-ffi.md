# 외부 함수 인터페이스

Maca는 C로 컴파일됩니다. 그러니 C ABI는 이 언어가 놓아야 할 다리가 아니라
이미 딛고 선 땅입니다. FFI는 `import` 하나입니다.

이 장이 그 전부입니다. 두 가지 형태, 타입 매핑, 선언 생성기, 그리고 이 중
어느 것도 쓰지 말아야 할 때에 대한 솔직한 지침.

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

드라이버는 `nix`가 있으면 그것으로, 없으면 호스트의 `cc`와 시스템 헤더/라이브러리로
라이브러리를 찾습니다. 그래서 평범한 리눅스 머신에서 `-lsqlite3`가 아무 설정 없이
동작합니다.

`examples/ffi_sqlite.maca`가 진짜 데이터베이스를 열고 진짜 결과 집합을 순회합니다.

## 타입 매핑

C 타입은 작고 고정된 표에 따라 Maca로 넘어옵니다.

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

큰 헤더의 extern 선언을 손으로 쓰는 것은 지겨운 일이라 생성기가 있습니다.

```
maca bindgen /usr/include/sqlite3.h
maca bindgen /usr/include/sqlite3.h sqlite3.maca
```

인자가 하나면 출력하고, 둘이면 모듈을 씁니다. 완전한 C 파서가 아니라 프로토타입
스캐너입니다. 주석과 전처리기 줄을 걷어내고, `;`로 나누고, 각 `RET NAME(PARAMS)`를
Maca 선언으로 바꿉니다. typedef, struct, union, 함수 포인터는 추측하지 않고
건너뜁니다.

```maca
sqlite3_libversion() -> str
sqlite3_open(filename: str, ppDb: int) -> int
sqlite3_column_double(sqlite3_stmt: int, iCol: int) -> float
sqlite3_close(sqlite3: int) -> int
```

bindgen은 구현이 둘입니다. 컴파일러 안의 원래 Rust 구현과, `tools/bindgen.maca`의
Maca 포팅본입니다. 테스트가 같은 헤더로 둘을 돌려 출력이 정확히 일치할 것을
요구합니다. 포팅본이 표류하도록 두지 않습니다.

## Python

```maca
import py "json"
```

Python 연동은 `python3-config`를 거치고, 모듈의 함수를 같은 방식으로 호출할 수
있게 됩니다. 둘 중 무거운 쪽입니다. 인터프리터를 임베드하니까요. Python에만
있는 라이브러리에 닿기 위해 존재합니다.

## JavaScript: `maca` 브리지

JS 백엔드는 다른 종류의 외부 코드를 인라인으로 받습니다. `import js """…"""`는
블록을 그대로 `app.js`에 심습니다. `.maca` 사용자 인터페이스가 필요한 호스트
글루(WebAssembly 인스턴스, 에디터, 브라우저 API)를 두 번째 파일 없이 들고 다니는
방법입니다.

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

이 객체가 존재하는 이유가 바로 그것입니다. 예전에는 블록이 `state.form_titel = …`
처럼 직접 대입했고, 그러면 아무것도 바인딩되지 않은 필드가 조용히 생기고 예외도
나지 않아서, 채우려던 대화상자가 그냥 빈 채로 남았습니다. 상수를 거절하는 이유도
Maca가 상수 재대입을 거절하는 이유와 같습니다.

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
선언되지 않은 이름을 주는 것은 상태 이름 오타와 똑같이 거절되고, 아무도 구현하지
않은 것을 호출하면 그렇다고 말합니다.

```
maca: `cfg_write` is declared in Maca but nothing implements it;
call maca.provide({ cfg_write: … }) from the import js block
```

### 순서

생성된 파일은 브리지, 블록, 앱 순입니다. 그래서 `maca.provide`와 `maca.set`는
블록의 최상위에서 동작하고, 거기서 정한 값은 `mount()`가 화면을 처음 그리기 전에
이미 자리를 잡습니다.

생성된 `state` 객체와 `update()` 함수는 여전히 있고 여전히 동작합니다. 그것들에
기대어 쓰인 프로그램이 있으니까요. 다만 그 둘은 약속이 아니라 이 백엔드의 지역
변수입니다. 문서화된 쪽은 `maca`이고, 새 프로그램이 잡아야 할 것도 그쪽입니다.

### 그 위에 올린 프로그램 하나

`apps/playground/playground.maca`가 그 실제 예입니다. 본문 없는 함수 열다섯 개와
상태 이름 넷을 선언하고, 그것이 `import js` 블록에 닿을 수 있는 표면의 전부입니다.
경계 너머에 있는 것은 Maca로 적을 수 없는 브라우저의 능력뿐입니다. WebAssembly
인스턴스, Monaco 에디터, URL 프래그먼트, 클립보드, 샌드박스 미리보기 iframe.
이쪽에 있는 것은, 페이지가 찍는 모든 문장까지 포함해 전부 Maca입니다.

## 언제 FFI를 쓸까

솔직한 지침은 이렇습니다. 일이 자체 완결적이면 Maca로 구현하고, 라이브러리
자체가 가치인 곳(SQLite, 압축 코덱, 시스템 API)에 FFI를 쓰세요. 모든 FFI
호출은 타입 검사기가 도와줄 수 없는 지점입니다. 반대편의 타입은 검사된 것이
아니라 주장된 것이니까요.

## 반대 방향

Maca는 다른 언어를 호출하는 대신 다른 언어로 컴파일될 수도 있습니다.
`--target rust`는 crates.io를 쓸 수 있는 Rust 소스를, `--target jvm`은 Java를,
`--target js`는 JavaScript를 내보냅니다. 필요한 생태계가 그중 한 플랫폼에 있다면
바인딩하는 것보다 그쪽을 타깃하는 편이 나은 경우가 많습니다.
[타깃](a10-targets.md)이 여섯 개 전부를 다룹니다.
