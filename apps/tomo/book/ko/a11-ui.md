# UI 문법

요소 문법이 가진 모든 형태와, 스타일시트 생성기가 따르는 모든 규칙. 둘 다
라이브러리가 아니라 언어의 일부이고, 둘 다 하나의 타깃에만 묶여 있지 않습니다.
같은 마크업이 브라우저용으로 빌드하면 살아 있는 DOM이 되고, 네이티브 바이너리로
빌드하면 HTML 문자열이 됩니다.

입문은 [인터페이스와 문서](15-ui.md)입니다. 지금 읽고 계신 이 페이지가 여기
적힌 것으로 만들어졌습니다.

## 요소는 호출이다

HTML 태그 이름은 그대로 호출할 수 있습니다.

```maca
article(class="prose",
    h1("안녕하세요")
    span("본문"))
```

이름 있는 인자는 속성이 되고, 위치 인자는 자식이 됩니다. 자식은 쉼표로 나눠도
되고 아무것도 쓰지 않아도 됩니다. 뒤쪽이 중첩된 마크업을 읽기 좋게 만들어 주는
쪽이고, 위 예제에서 제목과 문단 사이에 아무 구두점이 없는 이유입니다.

빠뜨릴 닫는 태그도, 템플릿 언어도, 별도 파일도 없습니다. 요소는 평범한
표현식이므로 나머지 모든 것과 어울립니다. 함수 안에서 만들 수도, 리스트에 담을
수도, `match`에서 반환할 수도 있습니다.

```maca
item(name: str) -> str => li(name)

list_of(names: str[]) -> str =>
    ul(names.map(item).join(""))
```

## 타깃은 둘, 문법은 하나

```
maca build app.maca --target js -o out
```

는 반응형 페이지를 만듭니다. 요소는 `createElement` 호출이 되고, `onclick=…`
은 핸들러를 붙이며, 상태 이름에 붙은 `value=…` 는 그 이름을 양방향으로 묶습니다.
이 책과 함께 배포되는 플레이그라운드가 바로 이렇게 컴파일된 `.maca` 파일
하나입니다.

```
maca build gen.maca -o gen
```

는 네이티브 바이너리를 만들고, 거기서 같은 요소는 **텍스트**로 렌더링됩니다.

```maca
main() -> int {
    info(article(class="prose", h1("Hi") span("Body")))
    0
}
```

```
<article class="prose"><h1>Hi</h1><span>Body</span></article>
```

정적 사이트 생성기에 필요한 것이 정확히 이것입니다. 문자열에는 이벤트 핸들러가
붙을 자리가 없으므로, 네이티브 타깃에서 예전 철자인 `on:click=` 지시자는 컴파일
오류가 되어 `js` 로 빌드하라고 알려 줍니다. 조용히 아무 일도 하지 않는 마크업을
내놓는 대신에요. 반면 평범한 `onclick="…"` 은 거기서 보통의 HTML 속성입니다.
HTML 도 그렇게 읽으니까요. 다만 그 타깃에서 `onclick=` 에 *함수* 를 주면 그것도
페이지에 닿지는 못하지만, 불평이 Maca 가 아니라 C 컴파일러에서 나옵니다. 두
철자가 갈리는 곳은 여기뿐입니다.

속성 값은 이스케이프되고, 자식은 **되지 않습니다**. 자식은 이미 마크업인 다른
요소이거나, 프로그램이 일부러 거기에 넣은 텍스트이기 때문입니다. 코드 블록을 직접
이스케이프해 둔 생성기가 렌더러에게 한 번 더 이스케이프당할 수는 없습니다.

## 이벤트, 그리고 양방향인 이름 하나

이름이 `on` 다음에 소문자로 이어지는 속성은 이벤트 핸들러이고, 그 소문자가 곧
이벤트입니다. `onclick`, `oninput`, `onchange`, `ondragstart`, `ondragover`,
`ondragend`, `ondrop`, 그리고 플랫폼이 앞으로 늘릴 무엇이든. 추가해야 할 목록은
없습니다. 규칙이 곧 철자이기 때문입니다.

```maca
li(draggable=true, ondragstart=grab, ondragover=over, ondrop=drop_here, name)
```

값은 함수입니다. 이름으로 부른 최상위 정의이거나, 이벤트를 받는 람다입니다.

```maca
button(onclick=(e => count = count + 1), "+")
```

`value=` 는 양쪽 방향으로 읽히는 하나뿐인 속성입니다. 프로그램이 선언한 이름을
주면 프로퍼티가 상태를 따라가고, 입력한 것이 상태로 되돌아갑니다.

```maca
who = "world"

main() -> Element => div(input(value=who) span("Hello, {who}"))
```

그 밖의 것은, 상수를 포함해, 보통의 속성입니다. `input(value="literal")` 은
속성을 설정하고 아무것도 듣지 않습니다. 저장할 값이 입력한 텍스트 그대로가
아니라면 람다가 무엇을 저장할지 말합니다.

```maca
input(value=(v => age = int(v)))
```

예전 지시자 철자인 `on:click=` 과 `bind:value=` 도 그대로 파싱되고 같은 뜻입니다.
`value` 가 아닌 프로퍼티를 양방향으로 묶는 방법은 `bind:` 뿐이기도 합니다.

## 대입이 곧 갱신이다

핸들러는 다시 그려 달라고 부탁하지 않습니다. 선언된 상태 이름에 쓰는 것이 곧
그 부탁입니다.

```maca
count = 0
note = "idle"

go() {
    count = count + 1
    note = "counted"
}

main() -> Element =>
    div(button(onclick=go, "go") span("{count}") span(note))
```

이것을 떠받치는 규칙이 셋 있고, 페이지가 예상과 다르게 굴 때 알아 둘 만합니다.

**그 이름을 읽는 것만 다시 실행됩니다.** 묶인 노드마다 자기 식이 언급하는 상태
이름을 기록해 두므로, `count` 에 대입하면 `count` 를 읽는 노드가 다시 실행되고
`note` 를 읽는 노드는 그대로 있습니다. 값이 *호출* 에서 나오는 노드
(`span(shown(count))`) 는 이름을 하나도 기록하지 못합니다. 함수 본문은 손이 닿지
않는 곳이기 때문입니다. 그래서 그런 노드는 무엇이 바뀌든 다시 실행됩니다.

**핸들러 하나가 한 턴입니다.** 이벤트가 도착하고 핸들러가 끝날 때까지 대입된
것은 모두 모였다가, 마지막에 한 번 다시 그려집니다. 루프도 같은 답입니다. 한
핸들러 안에서 백 번 대입해도 루프가 끝난 뒤 한 번입니다.

**아무것도 바꾸지 않은 쓰기는 갱신이 아닙니다.** 이름이 이미 들고 있던 값을
그대로 대입하면 아무것도 더럽혀지지 않고 아무것도 다시 그려지지 않습니다.

여기까지는 *노드* 단위이고, 뷰는 노드 하나보다 큽니다. 자식이 뷰 호출
(`toolbar()`, `dialog()`)이면 그것은 서브트리 전체이고, 그 모양은 뷰가 무엇을
읽었는지에 달려 있습니다. `toolbar()` 는 잠겨 있는 동안 `[]` 를 주고 풀리면
단추 줄을 줍니다. 그래서 뷰 자식은 쓰인 자리에 닻을 내리고, 그것이 읽는 상태가
바뀌면 **다시 만들어집니다**. 옛 노드와 그 노드들의 바인딩은 함께 버려집니다.
지역값도 함께 새로 만들어지므로, 다시 만드는 계기는 프로그램의 상태이지 뷰
자신의 지역값이 아닙니다. 지역값에 깨어난다면 깨운 그 쓰기를 버리게 됩니다.

`update()` 는 남아 있고 `maca.refresh()` 도 그렇습니다. 규칙이 볼 수 없는 경우,
즉 Maca 바깥에서 무언가 움직였고 그것을 읽는 노드에게 알려 주어야 하는 경우를
위해서입니다. 뷰가 상태에 *대입하는* 것은 다른 이야기입니다. 그것은 영원히
자기를 다시 그리게 되므로, 탭을 멈춰 세우는 대신 멈추고 그 사실을 말합니다.

## 정의가 태그를 이긴다

`label`, `code`, `main`, `section`, `p`, `a`, `form`, `option` 은 HTML 태그
*이면서* 사람들이 자기 함수와 변수에 붙이는 이름이기도 합니다. 그 이름이
정의되어 있으면 정의가 이깁니다.

```maca
label(pos: bool) -> str => pos ? "오른쪽" : "왼쪽"

main() -> int {
    info(label(true))     // "오른쪽", <label>이 아니라 여러분의 함수
    info(span("tag"))     // "<span>tag</span>", `span`을 가리는 것은 없음
    0
}
```

이것은 특정 이름 목록을 위한 예외가 아니라, 태그를 따지기 전에 적용되는 평범한
스코프 규칙입니다. 지역 바인딩도 함수와 똑같이 태그를 가립니다.

## 하이픈, 그리고 그것을 가능하게 하는 규칙

문서에는 `data-*`, `aria-*`, `http-equiv`, `accept-charset` 이 가득합니다.
HTML에 있는 그대로, 하이픈을 써서 씁니다.

```maca
nav(data-tomo="toc", aria-label="목차", body)
```

```
<nav data-tomo="toc" aria-label="목차">…</nav>
```

여기에 치환도 우회도 없습니다. **붙여 쓴** `-` 는 식별자의 일부이고, **띄어 쓴**
`-` 는 뺄셈 연산자이기 때문입니다. 두 해석이 같은 인자 목록 안에 모호함 없이
공존합니다.

```maca
div(data-kind="note", span("{a - b}"))
```

이 규칙은 이미 두 번 만났습니다. `x?` 는 실패를 전파하고 `c ? x : y` 는
삼항이며, `{n:>8}` 은 포맷 스펙이고 `{c ? a : b}` 는 문자열 안의 삼항입니다.
붙여 쓰기와 띄어 쓰기는 일부러 다른 뜻이고, 공백이 그 선택입니다.

같은 하이픈이 **커스텀 요소**의 이름이 됩니다. 그것이 커스텀 요소가 무엇인지에
대한 플랫폼 자신의 규칙이기 때문입니다. 하이픈이 든 태그가 커스텀 요소이고,
그래서 내장 태그에는 결코 하이픈이 없습니다. 웹 컴포넌트도 다른 모든 태그와
똑같이 부릅니다.

```maca
iconify-icon(class="text-2xl", icon="lucide:lock")
```

```
<iconify-icon class="text-2xl" icon="lucide:lock"></iconify-icon>
```

JS 백엔드는 노드를 만들고, 컴포넌트의 스크립트가 그것을 정의할 때 브라우저가
승격하도록 둡니다. 네이티브 백엔드는 마크업을 쓰되 자체 닫기가 아니라 닫는
태그로 씁니다. 빈 요소인 커스텀 요소는 없기 때문입니다. `label` 과 `code` 가
그렇듯 정의는 여전히 태그를 이깁니다.

## 식별자만으로는 안 되는 두 가지

**불리언.** HTML은 속성 값이 무엇이든 참으로 읽습니다. `hidden="false"` 도
요소를 숨깁니다. 그래서 bool은 속성의 값이 아니라 속성의 **존재 여부**를
결정합니다.

```maca
details(open=true, summary("더 보기") "내용")   // <details open>…
div(hidden=false, "보임")                       // <div>보임</div>
div(hidden=n > 5, "계산됨")                     // 실행 시점에 결정
```

**실행 시점에 정해지는 태그.** 문서 생성기는 입력을 보고 태그를 고릅니다. 제목의
깊이가 `h1`…`h6` 을 고르고, 표의 행이 `th` 인지 `td` 인지를 고릅니다.
`element` 는 태그를 값으로 받습니다.

```maca
heading(level: int, text: str) -> str =>
    element("h" ++ level, id=slug(text), text)
```

어떤 호출로도 이름 붙일 수 없는 `<main>` 에도 이것으로 닿습니다. 모든 프로그램이
`main` 을 정의하고, 정의가 이기기 때문입니다.

## 목록인 자식

위치 인자는 `Element`가 아니라 `Element[]`여도 되고, 그러면 목록의 각 원소가
그 자리의 자식이 됩니다. `[]`는 아무것도 내지 않습니다. `js`에서는 노드가 없고
네이티브에서는 마크업이 없습니다.

| 형태 | 내는 것 |
|---|---|
| `[a, b]` | 자식 둘, 순서대로 |
| `xs.map(f)` | 원소마다 자식 하나 |
| `a ++ b` | 두 목록의 자식 전부 |
| `[]` | 없음 |
| `-> Element[]`로 선언된 호출 | 그 뷰가 돌려준 것 |
| `-> Element`로 선언된 호출 | 그 요소 하나 |

`Element`는 렌더링된 요소의 타입입니다. 네이티브에서는 `str`, `js`에서는 DOM
노드입니다. 컴파일러가 읽는 것은 선언이므로, 노드를 돌려주는 뷰는 그렇게
적습니다.

```maca
toolbar(locked: bool) -> Element[] {
    if locked {
        return []
    }

    [div(class="toolbar", button("edit"))]
}
```

`str`을 돌려주는 함수는 그대로입니다. 여전히 텍스트로 렌더링되는 자식입니다.
`class="hidden"` 삼항식을 대신하는 것이 이것입니다. 노드를 만들어 놓고 숨기는
것이 아니라 아예 만들지 않습니다.

## 스타일은 링크하는 것이 아니라 생성되는 것

클래스는 Tailwind의 유틸리티 이름으로 `class=` 에 쓰고, 컴파일러는 프로그램이
실제로 언급한 유틸리티에 대한 스타일시트를 만들어 냅니다.

```maca
page() -> str =>
    div(class="max-w-2xl mx-auto font-bold", "텍스트")
```

`styles()` 가 그 스타일시트를 문자열로 돌려줍니다.

```maca
head(
    meta(charset="utf-8")
    style(styles()))
```

```css
*,*::before,*::after{box-sizing:border-box}
html,body{margin:0}
.font-bold { font-weight:700; }
.max-w-2xl { max-width:42rem; }
.mx-auto { margin-left:auto;margin-right:auto; }
```

리셋 두 줄, 그다음 쓰인 유틸리티 하나당 규칙 하나입니다. 프로그램이 언급하지
않은 유틸리티는 규칙을 만들지 않습니다. 흔들어 떨어뜨릴 프레임워크가 없는데,
애초에 아무것도 포함하지 않았기 때문입니다. 네트워크 요청도 없고, 그래서 이렇게
만든 책은 디스크에서 바로 열어도 제대로 보입니다.

클래스는 사용하는 자리의 `class=` 만이 아니라 모듈 어디에서든 수집되므로,
함수로 빼내도 됩니다.

```maca
button_class() -> str =>
    "font-bold hover:bg-zinc-100 dark:bg-zinc-800 md:px-4"
```

수집되지 *않는* 곳은 딱 하나, 원시 `"""…"""` 문자열 안입니다. 원시 블록으로 쓴
마크업은 수집기에게 보이지 않고, 그 안의 클래스는 끝내 만들어지지 않는 규칙을
가리키게 됩니다.

## 변형

유틸리티 앞에 붙는 접두사가 적용 조건을 좁힙니다. 상태 변형은 선택자에 접미사를
붙입니다.

| 변형 | 선택자 |
|---|---|
| `hover:` `focus:` `active:` | `:hover` `:focus` `:active` |
| `first:` `last:` | `:first-child` `:last-child` |
| `open:` | `[open]`: 열린 `<details>` |
| `before:` `after:` `marker:` | 대응하는 의사 요소 |
| `placeholder:` | `::placeholder` |
| `details-marker:` | `::-webkit-details-marker` |

조건 변형은 규칙을 미디어 쿼리로 감쌉니다.

| 변형 | 쿼리 |
|---|---|
| `dark:` | `prefers-color-scheme: dark` |
| `sm:` `md:` `lg:` `xl:` | 최소 너비 40 / 48 / 64 / 80rem |
| `max-sm:` `max-md:` `max-lg:` | 최대 너비 40 / 48 / 64rem |

순서와 개수에 상관없이 겹쳐 쓸 수 있습니다.

```maca
a(class="text-zinc-500 hover:text-black dark:hover:text-white max-md:hidden",
  href="x.html", "링크")
```

생성된 규칙은 변형이 그것이 수식하는 맨 유틸리티를 이기도록 정렬됩니다. CSS는
동점을 소스 순서로 가르기 때문에, `max-md:block` 이 `grid` 에 지는 것은 미관의
문제가 아니라 진짜 버그입니다. 그래서 이 정렬은 손으로 맞추는 것이 아니라
생성기의 일부입니다.

## 임의 값

스케일에 필요한 값이 없을 때는 대괄호에 값을 그대로 씁니다.

```maca
div(class="max-w-[42rem] text-[0.88em] mt-[3px]", body)
```

class 속성에는 공백을 넣을 수 없으므로, 대괄호 안의 밑줄은 공백이 됩니다.

```maca
div(class="grid-cols-[1fr_18rem]", body)
```

생성되는 선택자는 이스케이프됩니다. 보기보다 중요한 부분인데, `.max-w-[42rem]`
는 유효한 CSS 선택자가 아니고, 이것을 만난 브라우저는 **아무 말 없이 규칙을
버리기** 때문입니다. 콘솔 경고도 없고, 스타일이 빠진 것 말고는 눈에 띄는 실패도
없습니다.

## 한데 모으면

다른 파일 없이, 페이지 하나를 온전히 만들어 봅시다.

```maca
main() -> int {
    write_file("index.html",
        "<!doctype html>\n"
        ++ html(lang="ko",
            head(
                meta(charset="utf-8")
                meta(name="viewport", content="width=device-width,initial-scale=1")
                title("메모")
                style(styles()))
            body(class="font-serif bg-white dark:bg-zinc-900",
                element("main",
                    h1(class="text-[2rem] font-bold", "메모")
                    span(class="my-4", "Maca로 썼습니다.")))))
    0
}
```

`maca run` 한 번이면 스타일이 입혀지고, 자체 완결적이며, 다크 모드를 아는
페이지가 나옵니다. 이 책을 만든 생성기 [Tomo](a16-tomo.md)는 여기에 Markdown
파서를 하나 얹은 것입니다.

## 타깃별로 요소가 무엇이 되는가

| 타깃 | 요소가 되는 것 |
|---|---|
| 네이티브 (C) | HTML 문자열을 만드는 `maca_concat` 체인. `maca_attr`가 속성 값을 이스케이프하고, 자식은 다시 이스케이프하지 않으며, void 요소는 스스로 닫힙니다 |
| `js` | `createElement` 호출과 반응형 DOM. `onclick=`은 핸들러를 붙이고, 상태 이름에 붙은 `value=`는 양방향으로 묶으며, 그 이름에 대입하면 그것을 읽는 노드가 다시 그려집니다 |
| `element(tag, …)` | 양쪽에서 같고, void 여부는 실행 시점에 `maca_element`가 정합니다 |
| `open=true` | `maca_flag`. 속성이 있거나 없거나이지 `="false"`는 절대 아닙니다 |

네이티브 타깃의 `on:click=` 지시자는 `--target js`를 가리키는 컴파일 오류이고,
두 타깃이 받아들이는 것이 갈리는 곳은 여기뿐입니다. [타깃](a10-targets.md)을
보세요.

## 페이지의 애셋, 그리고 경로 대신 패키지를 부르기

raw `"""…"""` 블록은 소스 그 자체이고 자기가 어느 언어인지 말합니다. 따옴표로
감싼 `"…"`는 파일을 가리키며, 확장자가 이미 종류를 말하므로 언어 이름을 붙이지
않습니다. 그 파일은 페이지를 빌드할 때 읽혀서 페이지 안에 인라인됩니다. 그래서 페이지는 아무것도 링크하지 않고 벤더 스타일시트와 벤더
스크립트를 스스로 지니고 다닙니다.

```maca
import "vendor/reset.css"
import "vendor/iconify-icon.js"
```

여기서 경로는 그것을 쓴 파일을 기준으로 풀리고, 아무 파일도 가리키지 못하는
경로는 그 경로를 이름으로 부르는 빌드 오류입니다.

`maca add`를 실행한 프로젝트가 설치 도구가 고른 디렉터리에 손을 뻗을 일은
없어야 합니다. `npm:`은 `maca.toml`이 의존성에 이미 쓰고 있는 접두사이고,
애셋 임포트에서도 같은 뜻입니다.

```maca
import "npm:daisyui"
import "npm:iconify-icon"
```

**진입점은 패키지가 스스로 정합니다.** 패키지의 `package.json`을 읽어서
`style`, `browser`, `module`, `main` 중 알맞은 종류의 파일을 가리키는 첫 번째
것이 실립니다. 스타일시트는 `.css`, 스크립트는 `.js`/`.mjs`/`.cjs`,
WebAssembly는 `.wasm`입니다. 여러 개를 적어 둔 패키지도 모호하지 않습니다.
그 목록에는 순서가 있고 페이지는 곧 브라우저이므로 `browser`가 `module`을,
`module`이 `main`을 이깁니다.

조용히 아무 일도 일어나지 않는 대신 오류가 되는 것이 셋 있습니다.

| 무엇 | 빌드가 하는 말 |
|---|---|
| 패키지가 설치되어 있지 않음 | `` `daisyui` is not installed; run `maca add npm:daisyui` `` |
| 그 종류의 진입점이 없음 | `` `iconify-icon` states no stylesheet entry point `` |
| 진입점이 없는 파일을 가리킴 | `` `daisyui` states style = "dist/full.css", which is not there `` |

원하는 파일이 진입점이 아니라면 파일을 직접 부르면 됩니다.

```maca
import "npm:daisyui/dist/themes.css"
```

스코프가 붙은 패키지는 `maca add`가 설치할 때 쓴 맨 이름으로 닿습니다. 그래서
`npm:@wooorm/starry-night`와 `npm:starry-night`는 같은 디렉터리입니다.
`maca_modules`를 찾아 올라가는 걸음은 평범한 `import`가 걷는 그 걸음, 즉
임포트한 파일에서 워크스페이스 루트까지이므로, 하위 디렉터리에 있는 페이지도
프로젝트 루트에 설치된 패키지를 찾아냅니다. [모듈과 배치](a9-modules.md)를
보세요.
