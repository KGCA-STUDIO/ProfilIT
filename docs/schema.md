# `profile.toml` 스키마

현재 버전: **1** (`schema_version = 1`)

정식 정의는 [`src/config.rs`](../src/config.rs) 이고, 이 문서는 그 타입들을 사람이 읽을 수
있게 옮긴 것입니다. 검증 규칙은 [`src/validate.rs`](../src/validate.rs) 에 있습니다.

## 번역 가능한 값

표에서 **Text** 로 적힌 값은 문자열이거나 언어별 표입니다.

```toml
title = "학력"                             # 모든 언어 공통
title = { ko = "학력", en = "Education" }   # 언어별
```

한 언어만 쓰면 평범한 문자열 그대로라, 기존 설정이 그대로 읽힙니다. 번역이
필요한 항목만 골라서 표로 바꾸면 됩니다.

해당 언어의 번역이 없으면 기본 언어로, 그것도 없으면 있는 것 중 하나로
대체합니다. 빈 화면을 보여주는 것보다 원문이라도 보이는 편이 낫기 때문입니다.
빠진 번역은 검증이 언어당 한 줄로 모아 알려줍니다.

화면 문구("연락처 저장" 등)는 설정이 아니라 `locales/<코드>.json` 에 있습니다.
내장 언어는 `ko` `en` `ja` 이고, 같은 이름의 파일을 프로젝트에 두면 문구를
덮어쓸 수 있습니다.

## 설계 원칙

**편집 방식에 의존하지 않습니다.** 이 스키마는 로컬 편집 UI, 정적 렌더러,
나중에 붙일 브라우저 관리자 모드가 모두 공유합니다. 편집 UI 에만 필요한
상태(예: "마지막으로 열어둔 섹션")는 여기 넣지 않습니다.

**테마 필드는 CSS 커스텀 프로퍼티와 1:1 로 대응합니다.** 렌더러는 `[theme]` 을
읽어 `<head>` 의 인라인 `<style>` 에 `--pf-*` 토큰만 주입하고, `styles.css` 는
그 토큰을 소비할 뿐입니다. 덕분에 테마를 바꿔도 스타일시트는 캐시에 남고,
편집 UI 는 CSS 변수만 갱신해서 실시간 프리뷰를 만들 수 있습니다.

**내용은 지우지 않고 숨깁니다.** 섹션과 대부분의 항목에 `enabled` 가 있습니다.
편집 UI 에서 토글로 껐다 켜는 것이 지웠다 다시 쓰는 것보다 안전합니다.

**앞으로의 변경.** 필드를 추가할 때는 `#[serde(default)]` 를 붙여 기존 파일이
계속 읽히게 합니다. 호환되지 않는 변경에만 `schema_version` 을 올리고
마이그레이션을 추가합니다.

> 대부분의 테이블은 `deny_unknown_fields` 가 켜져 있어 오타 난 키가 조용히
> 무시되지 않고 파싱 오류로 잡힙니다. `[[sections]]` 는 예외입니다 — 공통
> 필드와 타입별 본문을 `#[serde(flatten)]` 으로 합치는데 serde 가 flatten 과
> `deny_unknown_fields` 를 함께 지원하지 않기 때문입니다. 그 구멍은
> `validate::lint_section_keys` 가 원본 TOML 을 한 번 더 훑어 **경고**로 메웁니다.

---

## `[site]` — 메타데이터

| 키 | 타입 | 필수 | 설명 |
|---|---|---|---|
| `title` | string | ✅ | `<title>`, `og:title` |
| `description` | string | | `<meta name="description">`, `og:description` |
| `base_url` | string | | 배포 주소. 없으면 `og:url`·`canonical` 을 절대 경로로 만들 수 없어 **경고**가 납니다 |
| `lang` | string | | 기본 언어. 사이트 최상단(`/`)에 놓입니다. 기본값 `"ko"` |
| `languages` | string 배열 | | 생성할 언어들. 나머지는 `/en/` 처럼 하위 경로에 놓입니다 |
| `og_image` | 경로 | | 공유 카드 이미지 |
| `favicon` | 경로 | | 파비콘 |

## `[profile]` — 명함 상단

| 키 | 타입 | 필수 | 설명 |
|---|---|---|---|
| `name` | string | ✅ | 이름 |
| `tagline` | string | | 이름 아래 한 줄. 직함이나 짧은 소개 |
| `bio` | string | | 여러 줄 가능. 줄바꿈이 `<br>` 로 변환됩니다 |
| `avatar` | 경로 | | 원형 프로필 사진. 없으면 이름 첫 글자로 자리를 채웁니다 |
| `location` | string | | `"서울"` 처럼 짧은 지역 표기 |

## `[[socials]]` — 소셜 아이콘 행

배열 순서가 화면 순서입니다.

| 키 | 타입 | 필수 | 설명 |
|---|---|---|---|
| `platform` | enum | ✅ | 아래 목록 참고 |
| `url` | string | ✅ | 허용 스킴만 |
| `label` | string | | 스크린리더용. 생략하면 플랫폼 이름 |
| `icon` | 경로 | | 직접 넣을 SVG. `platform = "custom"` 이면 **필수** |

내장 플랫폼: `instagram` `youtube` `threads` `tiktok` `x` `facebook` `naver`
`naver_blog` `kakao_talk` `github` `linkedin` `email` `rss` `custom`

목록에 없는 서비스는 `platform = "custom"` 과 `icon` 을 함께 씁니다. 자주 쓰이는
서비스라면 `Platform` enum 에 변형을 추가하는 PR 이 더 좋습니다 — 기여 받기 가장
쉬운 지점으로 일부러 열어둔 구조입니다.

### 아이콘을 직접 넣기

`icon` 은 **어느 플랫폼에나** 쓸 수 있고, 지정하면 내장 아이콘 대신 그것이
나옵니다.

```toml
[[socials]]
platform = "github"
url = "https://github.com/example"
icon = "assets/github.svg"   # 내장 글리프 대신 이 파일
```

내장 아이콘은 브랜드 마크를 **흉내 낸 단순한 모양**입니다. 공식 로고를 그대로
담으면 배포할 때 상표 문제가 생길 수 있어서 일부러 그렇게 두었고, 대신 진짜
로고를 쓰고 싶은 사람의 길을 막지 않으려고 이 칸을 열어놨습니다.

> 로고 파일을 골라 넣는 것은 **쓰는 사람의 몫**입니다. 대부분의 서비스는
> 자기 계정으로 가는 링크에 아이콘을 쓰는 것을 브랜드 가이드라인에서
> 허용하지만, 조건은 서비스마다 다릅니다. 만든 결과물을 공개 배포한다면
> 해당 서비스의 가이드라인을 한 번 확인해 주세요.

파일은 `<image>` 로 참조합니다. 내용을 읽어 인라인으로 펼치면 `currentColor`
(테마 색 따라가기)가 먹지만, 남의 SVG 를 그대로 문서에 심는 셈이라 스크립트가
섞여 들어올 수 있어 그렇게 하지 않습니다. **넣은 아이콘은 테마 색을 따르지
않고 파일에 있는 색 그대로 나옵니다.**

---

## `[[sections]]` — 섹션

**배열 순서가 화면 순서입니다.** 모든 섹션이 공통으로 갖는 키는 다음 넷입니다.

| 키 | 타입 | 필수 | 설명 |
|---|---|---|---|
| `type` | enum | ✅ | 아래 여섯 가지 |
| `title` | string | | 섹션 제목. 생략하면 제목 없이 본문만 렌더링됩니다 |
| `icon` | string | | 제목 앞 이모지. 두 글자 넘으면 경고 |
| `enabled` | bool | | 기본값 `true` |

### `type = "about"` — 자유 문단

```toml
[[sections]]
type = "about"
title = "소개"
icon = "👋"
body = """
빈 줄로 문단을 나눕니다.

두 번째 문단.
"""
```

### `type = "timeline"` — 학력·경력

세로 레일과 점으로 그립니다. **배열 순서가 곧 표시 순서**라 `period` 를 정렬에
쓰지 않습니다 — 그래서 형식을 강제하지 않고 자유 문자열로 둡니다.

```toml
[[sections]]
type = "timeline"
title = "학력"
icon = "🎓"

[[sections.items]]
period = "2014 – 2018"
title = "○○대학교 컴퓨터공학과"
subtitle = "학사"
description = "분산 시스템 연구실에서 학부 연구생으로 1년."
```

| 항목 키 | 타입 | 필수 | 설명 |
|---|---|---|---|
| `title` | string | ✅ | 항목 제목 |
| `period` | string | | `"2014 – 2018"` 처럼 자유 형식 |
| `subtitle` | string | | 소속, 학위 같은 부제 |
| `description` | string | | 설명 |
| `url` | string | | 있으면 제목이 링크가 됩니다 |
| `enabled` | bool | | 기본값 `true` |

### `type = "checklist"` — 버킷리스트

```toml
[[sections]]
type = "checklist"
title = "버킷리스트"
icon = "✨"
show_progress = true   # 기본값 true. 상단 진행률 표시

[[sections.items]]
text = "제주도 한 달 살기"
done = true
date = "2025-04"
note = "생각보다 비 오는 날이 많았습니다."
```

| 항목 키 | 타입 | 필수 | 설명 |
|---|---|---|---|
| `text` | string | ✅ | 항목 내용 |
| `done` | bool | | 기본값 `false` |
| `date` | string | | 달성 시점. 자유 형식 |
| `note` | string | | 한 줄 후기 |

`done = false` 인데 `date` 가 있으면 둘 중 하나를 빠뜨렸을 가능성이 커서
**경고**가 납니다.

달성 상태는 취소선과 색으로만 전하지 않습니다. 마크업에 숨은 "달성: " 텍스트가
함께 들어가 스크린리더에서도 구분됩니다.

### `type = "tags"` — 관심사·기술

항목은 세 가지 모양으로 적을 수 있습니다.

```toml
[[sections]]
type = "tags"
title = "관심사"
icon = "🧩"
items = [
  "Rust",                                            # 번역도 이모지도 없음
  { ko = "요리", en = "Cooking" },                    # 번역만
  { icon = "🍳", text = { ko = "요리", en = "Cooking" } },  # 이모지까지
]
```

이모지는 장식이라 `aria-hidden` 으로 나갑니다 — 낭독기가 "불꽃 요리" 처럼 읽으면
오히려 방해가 되기 때문입니다. 두 글자를 넘으면 경고가 납니다.

> `{ icon = ..., text = ... }` 와 번역 표 `{ ko = ..., en = ... }` 는 키 이름으로
> 구분됩니다. 그래서 `icon` 이나 `text` 를 언어 코드로 쓸 수는 없습니다.

### `type = "links"` — 링크 카드

```toml
[[sections]]
type = "links"
title = "블로그 & 채널"
icon = "🔗"

[[sections.items]]
title = "기술 블로그"
url = "https://example.com/blog"
subtitle = "서버 이야기 위주"
badge = "NEW"
highlight = true
```

| 항목 키 | 타입 | 필수 | 설명 |
|---|---|---|---|
| `title` | string | ✅ | 카드 제목 |
| `url` | string | ✅ | 허용 스킴만 |
| `subtitle` | string | | 제목 아래 작은 설명 |
| `thumbnail` | 경로 | | 카드 왼쪽 썸네일 |
| `enabled` | bool | | 기본값 `true` |
| `highlight` | bool | | 미세한 흔들림 애니메이션. `prefers-reduced-motion` 을 존중합니다 |
| `badge` | string | | `"NEW"` 같은 짧은 배지. 8자 넘으면 경고 |

### `type = "contact"` — 연락처

```toml
[[sections]]
type = "contact"
title = "연락처"
icon = "📮"

[[sections.items]]
kind = "email"
value = "me@example.com"
```

| 항목 키 | 타입 | 필수 | 설명 |
|---|---|---|---|
| `kind` | enum | ✅ | `email` \| `phone` \| `address` \| `website` \| `custom` |
| `value` | string | ✅ | 렌더러가 kind 에 맞는 링크로 감쌉니다 (`mailto:`, `tel:`) |
| `label` | string | | 화면에 보일 라벨. 생략하면 kind 의 기본 이름 |

`kind` 에 맞는 값인지 검증합니다. 이메일에 `@` 가 없거나 전화번호에 숫자가
7자 미만이면 **오류**입니다 — 렌더러가 링크로 감싸기 때문에, 형식이 틀리면
눌러도 아무 일이 없는 링크가 만들어집니다.

---

## `[theme]`

| 키 | 타입 | CSS 토큰 | 설명 |
|---|---|---|---|
| `accent` | 색상 | `--pf-accent` | 타임라인 점, 진행률 바, 태그 칩, 버튼 |

### `[theme.background]` → `--pf-background`

`type` 으로 분기하는 태그 유니온입니다.

```toml
[theme.background]
type = "solid"
color = "#ffffff"
```

```toml
[theme.background]
type = "gradient"
from = "#dff2fb"
to = "#bfe6f7"
angle = 165        # 0..=360, 기본값 180
```

무늬는 CSS 그라디언트로 그립니다. 이미지 파일이 필요 없습니다.

```toml
[theme.background]
type = "pattern"
name = "dots"            # dots | grid | stripes | checks
color = "#ffffff"        # 무늬 아래 바탕색
pattern_color = "#e6f4fb"
size = "22px"            # 무늬 한 칸 크기
```

바탕색과 무늬색이 같으면 무늬가 보이지 않으므로 **경고**가 납니다.

```toml
[theme.background]
type = "image"
src = "assets/bg.jpg"
fit = "cover"           # cover | contain | repeat
position = "center"     # CSS background-position
blur = "0px"            # 흐림 반경
overlay = "#ffffff80"   # 선택. 가독성용 반투명 덮개
```

`image` 만 전용 레이어(`.pf-backdrop`)를 씁니다. `background-image` 에는
`filter` 를 걸 수 없어서 흐림 처리를 하려면 별도 요소가 필요하기 때문입니다.
나머지 세 타입은 `--pf-background` 토큰 하나로 끝납니다.

배경 이미지를 어둡게 쓴다면 `[theme.sheet]` 과 `[theme.text]` 도 함께 맞춰야
합니다. 명함 본체는 배경과 독립적이라 자동으로 따라오지 않습니다.

### `[theme.decoration]`

색종이·스티커 장식 레이어입니다. 순수 장식이라 `aria-hidden` 으로 나가고
`pointer-events: none` 이 걸립니다. 생략하면 아무것도 그리지 않습니다.

```toml
[theme.decoration]
type = "preset"
name = "confetti"   # confetti | sparkle | bubble
```

```toml
[theme.decoration]
type = "custom"
top = "assets/decor-top.png"
bottom = "assets/decor-bottom.png"
```

장식 레이어는 뷰포트가 아니라 **문서**에 고정됩니다. `fixed` 로 두면 스크롤할 때
내용이 하단 장식 밑을 지나가면서 글자가 가려집니다.

### `[theme.sheet]` — 명함 본체

| 키 | 타입 | CSS 토큰 | 설명 |
|---|---|---|---|
| `background` | 색상 | `--pf-sheet-bg` | 종이 색 |
| `radius` | CSS 길이 | `--pf-sheet-radius` | 기본값 `"24px"` |
| `shadow` | bool | `--pf-sheet-shadow` | 기본값 `true` |

### `[theme.card]` — 섹션 안의 링크 카드

| 키 | 타입 | CSS 토큰 | 설명 |
|---|---|---|---|
| `background` | 색상 | `--pf-card-bg` | 카드 배경 |
| `radius` | CSS 길이 | `--pf-card-radius` | 기본값 `"14px"` |
| `shadow` | bool | `--pf-card-shadow` | 기본값 `false` |
| `style` | enum | `data-card-style` | `fill`(기본) \| `outline` \| `glass` |

### `[theme.text]`

네 값 모두 선택입니다. 일부만 지정하면 나머지는 기본값을 씁니다.
`[theme.sheet]`, `[theme.card]`, `[theme.font]`, `[features]`, `[footer]` 도
마찬가지로 부분 지정이 됩니다 — 값 하나 바꾸려고 테이블 전체를 적어야 하면
안 되기 때문입니다.

| 키 | CSS 토큰 | 쓰이는 곳 |
|---|---|---|
| `heading` | `--pf-text-heading` | 이름, 섹션 제목, 타임라인 항목 제목 |
| `body` | `--pf-text-body` | 본문, 푸터 |
| `card_title` | `--pf-text-card-title` | 링크 카드 제목, 연락처 값 |
| `muted` | `--pf-text-muted` | 기간, 부가 설명, 라벨 |

### `[theme.font]`

프리셋을 고르면 **폰트 스택과 웹폰트 CDN 주소가 함께 따라옵니다.** 주소를 직접
찾아 넣을 필요가 없고, 편집 UI 에서는 그대로 드롭다운이 됩니다.

```toml
[theme.font]
preset = "pretendard"
heading_preset = "jua"     # 선택. 제목만 다른 폰트로
heading_weight = 800
base_size = "15px"
line_height = 1.6
letter_spacing = "normal"
```

| 프리셋 | 이름 | 성격 |
|---|---|---|
| `system` | 기기 기본 | 웹폰트를 받지 않아 가장 빠름 (기본값) |
| `pretendard` | Pretendard | 가변 폰트, 굵기 100–900 |
| `noto_sans_kr` | 본고딕 | 무난한 고딕 |
| `nanum_gothic` | 나눔고딕 | 고딕 |
| `nanum_myeongjo` | 나눔명조 | 명조(세리프) |
| `gaegu` | 개구 | 손글씨 |
| `jua` | 주아 | 둥근 제목용, 굵기 400 하나뿐 |
| `ibm_plex_sans_kr` | IBM Plex Sans KR | 고딕 |
| `gowun_dodum` | 고운돋움 | 단정한 고딕, 굵기 400 하나뿐 |
| `custom` | 직접 지정 | `family` 필수 |

| 키 | 타입 | CSS 토큰 | 설명 |
|---|---|---|---|
| `preset` | enum | `--pf-font-family` | 본문 폰트. 기본값 `system` |
| `heading_preset` | enum | `--pf-font-heading-family` | 제목 폰트. 생략하면 본문과 동일 |
| `family` | string | | `preset = "custom"` 일 때 **필수** |
| `heading_family` | string | | `heading_preset = "custom"` 일 때 **필수** |
| `stylesheets` | string 배열 | | 프리셋 주소에 **더해서** 넣을 CSS URL |
| `heading_weight` | int | `--pf-font-heading-weight` | 기본값 `700` |
| `base_size` | CSS 길이 | `--pf-font-size-base` | 기본값 `"15px"` |
| `line_height` | float | `--pf-line-height` | 1.0 – 2.5. 기본값 `1.6` |
| `letter_spacing` | CSS 길이 | `--pf-letter-spacing` | `"normal"` 또는 길이값 |

**굵기 검사.** 프리셋이 실제로 제공하지 않는 `heading_weight` 를 쓰면 경고가
납니다. 없는 굵기를 요구하면 브라우저가 글자를 억지로 굵게 그려서(합성 볼드)
모양이 뭉개지는데, 화면만 봐서는 원인을 알기 어렵습니다.

```
[경고] theme.font.heading_weight: 주아 (둥근 제목) 에는 800 굵기가 없어
       브라우저가 합성 볼드로 그립니다. 사용 가능한 굵기: 400
```

**`base_size` 와 접근성.** 모든 글자 크기가 이 값에 상대적인 `rem` 이라, 이
하나로 전체 크기가 정해집니다. `px` 로 주면 크기가 고정되고, `rem` 으로
주면(예: `"1rem"`) 읽는 사람이 브라우저에 설정한 기본 글자 크기를 따라갑니다.
접근성 면에서는 `rem` 이 낫지만 기본값은 `px` 입니다 — 디자인이 어긋나 보이는
쪽이 먼저 문제로 인식되기 때문입니다. 시력이 낮은 방문자를 고려한다면
`base_size = "1rem"` 으로 바꾸는 편이 좋습니다.

## `[features]`

| 키 | 타입 | 설명 |
|---|---|---|
| `share_menu` | bool | 링크 카드 오른쪽 ⋮ 버튼(링크 복사 / OS 공유 시트). 기본값 `true` |
| `vcard_download` | bool | 상단 "연락처 저장" 버튼. `contact` 섹션과 `[profile]` 로 .vcf 를 만듭니다. 기본값 `true` |

## `[deploy]` — GitHub Pages

| 키 | 타입 | 설명 |
|---|---|---|
| `remote` | string | 올릴 git 리모트. 기본값 `"origin"` |
| `branch` | string | 배포 브랜치. 기본값 `"gh-pages"` |
| `cname` | string | 사용자 지정 도메인. 있으면 `dist/CNAME` 을 씁니다 |

**토큰을 받지 않습니다.** 이미 설정된 git 자격증명을 그대로 씁니다. 대신
리포지터리 생성과 Pages 활성화는 GitHub 웹에서 한 번 해야 합니다.

작업 트리는 건드리지 않습니다. 임시 인덱스로 트리를 만들고 `commit-tree` 로
커밋을 빚어 푸시하므로, 브랜치를 체크아웃하지 않고 편집 중이던 스테이징도
그대로 남습니다.

`dist/.nojekyll` 을 함께 올립니다. 없으면 GitHub 이 Jekyll 을 돌려 `_` 로
시작하는 파일을 조용히 빼버립니다.

`site.base_url` 이 실제 Pages 주소와 다르면 배포 후 **경고**가 납니다 —
화면은 멀쩡한데 공유 카드만 엉뚱한 곳을 가리키는 종류의 고장이라서입니다.

## `[footer]`

| 키 | 타입 | 설명 |
|---|---|---|
| `text` | string | 직접 넣는 푸터 문구 |
| `show_powered_by` | bool | 기본값 `true` |

---

## 값 형식

**경로** — `profile.toml` 이 있는 디렉터리 기준 상대 경로입니다. 파일이 실제로
없으면 검증에서 **오류**입니다. 빌드 후에 404 로 발견하는 것보다 낫습니다.

**색상** — `#rgb` `#rgba` `#rrggbb` `#rrggbbaa` 를 검사합니다. `#` 으로 시작하지
않으면 CSS 함수나 키워드로 보고 통과시킵니다(`rgb(...)`, `transparent` 등).

**URL** — `https://` `http://` `mailto:` `tel:` 만 허용합니다. `javascript:` 같은
스킴이 링크로 들어가면 생성된 페이지에 실행 가능한 코드가 그대로 박히므로
검증 단계에서 막습니다. `http://` 는 통과하지만 경고가 붙습니다.

## 검증

```bash
cargo run -p profileit-desktop   # 데스크톱 앱
cargo run -- build              # 검증 후 dist/ 생성
cargo run -- deploy             # 빌드 후 GitHub Pages 로 올리기
cargo run -- check              # 검증만
```

`build` 는 검증을 통과해야 실행됩니다 — 깨진 결과물이 배포되는 것보다 빌드가
멈추는 편이 낫기 때문입니다.

**오류**는 빌드를 중단시키고, **경고**는 빌드는 되지만 의도한 결과가 아닐
가능성이 큰 경우입니다. 위치는 `sections[2].items[0].url` 처럼 TOML 경로로
표시됩니다.
