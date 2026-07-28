# Tomo: 지금 읽고 있는 이 책

이 핸드북은 마크다운 파일들이 든 디렉터리입니다. 지금 읽고 있는 HTML은
`apps/tomo/tomo.maca` — Maca로 쓴 정적 사이트 생성기 — 가 만들어냈습니다.
마지막 장인 이유는 이 책의 거의 모든 것을 쓰기 때문이고, 자기 문서를 만드는
도구가 좋은 마무리 연습이기 때문입니다.

*Tomo*는 스페인어로 책의 한 권을 뜻합니다. *maca*와 같은 안데스의 결입니다.

## 무엇인가

대략 mdBook입니다. 마크다운이 들어가고 탐색 가능한 HTML 책이 나옵니다. 다만
의도된 차이가 하나 있습니다. **i18n이 플러그인이 아닙니다.** 데이터 모델
자체입니다.

## 설정

```toml
[book]
title = "The Maca Handbook"
languages = ["en", "ko"]
chapters = [
    "00-introduction",
    "01-getting-started",
]
```

`languages`는 리스트이고 첫 항목이 기본값입니다. 챕터는 한 번만 이름 붙이고
`book/<lang>/` 아래에서 언어별로 해결됩니다.

## 폴백, 이것이 핵심

mdBook에서 책을 번역한다는 것은 평행한 책 하나를 더 유지한다는 뜻입니다. 아직
번역되지 않은 챕터는 없는 페이지입니다.

Tomo는 각 챕터를 언어별로 해결하고, 없으면 폴백합니다.

```maca
build_chapter(root: str, out: str, title: str, langs: str, lang: str,
              fallback: str, chs: str[], titles: str[], ci: int) -> int {
    ch = chs.get(ci)
    own = root ++ "/book/" ++ lang ++ "/" ++ ch ++ ".md"
    src = file_exists(own)
        ? own
        : root ++ "/book/" ++ fallback ++ "/" ++ ch ++ ".md"
    file_exists(src)
        ? write_chapter(out, title, langs, lang, chs, titles, ci,
                        read_file(src))
        : 0
}
```

번역되지 않은 챕터에 닿은 한국어 독자는 한국어 페이지 위에서 사이드바와 내비게이션이
그대로인 채로 영어 본문을 봅니다. 책이 깨지는 일은 없습니다. 부분적으로 번역된
것일 뿐입니다. 덕분에 번역을 한 챕터부터 시작해서 즉시 쓸모 있게 만들 수
있습니다. 배포하려면 먼저 완성되어야 하는 대신에요. 사실 대부분의 번역이 끝내
일어나지 않는 진짜 이유가 그것입니다.

목차도 섞습니다. 챕터의 제목은 자기 `# 헤딩`에서 오고, 어느 언어로 해결되었든
그것을 씁니다.

## 렌더러

핵심은 순수 함수입니다.

```maca
render(md: str) -> str
```

마크다운이 들어가고 HTML이 나옵니다. IO는 없습니다. 나머지 — 파일 읽기, 챕터
목록 순회, 사이트 쓰기 — 가 이것을 감쌉니다. 이 분리가 테스트를 가능하게 합니다.
게이트는 샘플에 `render`를 호출하고 HTML을 검사합니다.

줄들 위의 fold이고, 상태 둘을 넘깁니다.

```maca
render_lines(lines: str[], i: int, in_code: bool, acc: str) -> str =>
    i >= lines.length()
        ? (in_code ? acc ++ "</code></pre>\n" : acc)
        : render_line(lines, i, in_code, acc)
```

여러 줄에 걸치는 블록 요소 — 문단, 인용, 리스트, 표 — 는 자기 끝을 찾아 그
구간을 통째로 소비합니다.

```maca
render_para(lines: str[], i: int, acc: str) -> str {
    stop = para_end(lines, i)
    text = join_range(lines, i, stop)
    render_lines(lines, stop, false, acc ++ "<p>" ++ inline(esc(text)) ++ "</p>\n")
}
```

이건 들리는 것보다 중요합니다. 첫 버전은 소스 줄마다 `<p>` 하나를 냈고, 그래서
줄바꿈된 `**config mode**`가 `<strong>config</p>`가 되었습니다. 마크업이 문단을
가로질러 쪼개진 것이죠. 인용도 같은 버그가 있어서 세 줄짜리 인용이 세 개의
blockquote가 되었습니다. 둘 다 지금은 게이트의 테스트입니다.

## 서버 없는 검색

모든 페이지가 검색 상자를 가집니다. 인덱스는 언어별로, 헤딩당 하나씩 생성되고
그 절의 텍스트를 소문자로 담습니다.

```javascript
window.TOMO_INDEX=[{"u":"08-collections.html#lists","c":"Collections",
                    "s":"Lists","x":"a list of t is written…"},…]
```

페이지가 fetch하는 JSON이 아니라 페이지가 로드하는 `<script>`로 나갑니다. 게으름이
아닙니다. `file://`로 디스크에서 바로 연 책은 `fetch`를 할 수 없습니다. mdBook의
검색은 웹 서버가 필요하고, 이것은 USB 스틱의 폴더에서도 동작합니다.

## 빌드하기

```
maca run apps/tomo/tomo.maca
```

모든 언어의 모든 챕터를 렌더하고, 언어별 목차와 검색 인덱스를 쓰고, 페이지를
몇 개 썼는지 보고합니다. 테스트 스위트가 실제 핸드북을 빌드하고 결과를
검사합니다. 번역되지 않은 챕터가 폴백하면서도 한국어 페이지로 나오는지까지요.

## 이 책에서 무엇을 쓰는가

설정에 레코드. 줄 위를 걷는 모든 순회에 누산기를 든 재귀. 모든 파싱에 `str`
메서드. 챕터와 언어 집합에 리스트. 읽고 쓰는 데 파일 IO. 스타일시트와 검색
JavaScript에 raw `"""…"""` 문자열 — CSS는 중괄호 투성이라 그러지 않으면 보간으로
읽히니까요.

공교롭게도 합타입은 안 씁니다. 렌더러가 토큰 타입이 아니라 줄 접두사로
분기하기 때문입니다. 더 큰 마크다운 구현이라면 합타입을 원할 것입니다.

500줄쯤 됩니다. 그것이 정적 사이트 생성기 전부입니다. 자기가 문서화하는 언어로
쓰인 채로요.
