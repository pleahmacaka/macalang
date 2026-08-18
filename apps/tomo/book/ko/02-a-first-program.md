# 첫 프로그램

문장을 받아 각 단어가 몇 번 나왔는지 세는 단어 집계기를 만듭니다. 완성본은
`apps/examples/wordcount.maca`에 있습니다.

## 문장 쪼개기

문장을 단어로 만듭니다.

```maca
words(text: str) -> str[] =>
    text.lower().replace(".", " ").replace(",", " ").split(" ")
```

`str[]`는 문자열의 리스트입니다. 원소 타입을 먼저 쓰고 대괄호를 뒤에 붙입니다.

`str`는 자기 메서드를 가지지 않는 기본 타입입니다. `text.lower()`가 되는 것은
**UFCS** 덕분입니다. `x.f(y)`는 `f(x, y)`를 뜻합니다.

**화살표 본문**은 `=>` 다음에 식 하나가 오고 그것이 결과입니다.

## 세기

```maca
Tally = {
    word: str
    count: int
}
```

레코드, Maca의 구조체입니다. **`:`는 타입을 도입하고, `=`는 값을 도입합니다.**

단어 하나를 목격했다고 기록합니다.

```maca
bump(ts: Tally[], w: str) -> Tally[] {
    at = find(ts, w, 0)
    at < 0
        ? ts ++ [Tally { word = w, count = 1 }]
        : replace_at(ts, at, Tally { word = w, count = ts.get(at).count + 1 })
}
```

지역 바인딩이 먼저 필요해서 **블록 본문**이고, 블록의 마지막 식이 값입니다.
`at = find(...)`이 지역 변수를 바인딩합니다. `let`은 없습니다. `+`는 리스트나
문자열을 이어붙입니다.

도우미 두 개입니다.

```maca
find(ts: Tally[], w: str, i: int) -> int =>
    i >= ts.length() ? -1 : (ts.get(i).word == w ? i : find(ts, w, i + 1))

replace_at(ts: Tally[], at: int, t: Tally) -> Tally[] {
    ts[at] = t
    ts
}
```

`find`는 커서 `i`를 매개변수로 직접 들고 다니는 재귀 함수이고, `ts[at] = t`는
인덱스를 통해 대입합니다.

## 단어들을 접기

```maca
tally(ws: str[], i: int, acc: Tally[]) -> Tally[] =>
    i >= ws.length()
        ? acc
        : tally(ws, i + 1, ws.get(i).length() == 0 ? acc : bump(acc, ws.get(i)))
```

재귀 호출로 누산기를 넘기는 fold입니다. 공백으로 나누면 빈 문자열이 남으므로
빈 단어는 건너뜁니다.

## 출력

```maca
show(ts: Tally[], i: int) -> int {
    if i < ts.length() {
        info("{ts.get(i).word:<8} {ts.get(i).count}")
        show(ts, i + 1)
    }
    0
}
```

문자열은 **보간**됩니다. `{식}`이 평가되어 그 자리에 들어가고, `{…:<8}`은 8칸에
왼쪽 정렬하라는 포맷 스펙입니다([다음 장](03-common-concepts.md)).

## main

```maca
main() -> int {
    text = "the quick brown fox. the lazy dog, the end."
    ts = tally(words(text), 0, [])
    info("{ts.length()} distinct words")
    show(ts, 0)
    0
}
```

`main() -> int`가 진입점이고, 그 결과가 프로세스 종료 상태입니다.

## 실행하기

```
maca run apps/examples/wordcount.maca
```

```
7 distinct words
the      3
quick    1
brown    1
fox      1
lazy     1
dog      1
end      1
```

프로그램은 파싱되고, 타입 검사를 받고, C로 낮춰지고, 진짜 C 컴파일러가
컴파일하고, 결과 바이너리가 캐시됩니다.

## 설명하지 않은 것

왜 `Tally`가 클래스가 아니라 레코드인지, 리스트를 다시 만들 때 메모리에 무슨
일이 일어나는지, `str[]`이 메서드를 어디서 얻는지. 다음 장들의 내용입니다.
