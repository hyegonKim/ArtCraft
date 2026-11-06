# ArtCraft Monorepo

## 기존 ArtCraft 대비 변경 사항

아래 표는 **같은 제품의 기존 버전과 현재 버전**을 비교합니다. 기준은 19개 프로젝트를 처음 모은 커밋 `484843e`입니다. 현재 버전 열에는 GitHub `main`에 반영된 변경을, 마지막 열에는 후속 검증 결과와 반영 상태를 기록합니다. [GitHub 소스 비교](https://github.com/hyegonKim/ArtCraft/compare/484843e...main)

### 제품별 비교

| 제품 | 기존 버전 | 현재 `main`의 추가·수정 | 후속 검증 및 반영 상태 |
| --- | --- | --- | --- |
| [ArtCraft Native / Desktop](./artcraft) | Native 이미지 편집은 320 × 200 고정 캔버스를 사용했습니다. | **추가:** 연결 레이어 마스크, 비파괴 변형, RGB·채널별 Curves, 편집 가능한 `.acraft`, Craft Suite 실행 메뉴. **수정:** 원본 크기와 투명도를 유지하는 편집·PNG 저장, 연속 브러시·지우개와 획 단위 Undo/Redo, 웹 API 요청의 명시적인 `false`·`0`·빈 문자열 보존. | API 라이브러리·앱 테스트 13개와 라이브러리 빌드 통과. `main` 반영. |
| [ArtCraft Services](./artcraft-services) | 모델 선택 화면은 API 목록에 의존했습니다. 빈 로그인도 요청을 보냈고, 재생 전 첫 프레임 캡처가 실패할 수 있었습니다. | **추가:** API 없이 이미지·비디오 모델을 각 22개 고르는 독립 미리보기. **수정:** 일반 웹앱에서도 모델 API에 연결할 수 없으면 내장 목록으로 전환하고 생성 버튼을 비활성화합니다. 빈 로그인 입력을 요청 전에 차단하고, 재생 전 첫 프레임 캡처를 안정화했습니다. 웹 API·Dashboard 요청의 명시적인 `false`·`0`·빈 문자열을 보존하고, OpenAPI의 미해결 참조 4개와 경로 매개변수 127곳을 바로잡았습니다. | 웹 API 테스트 10개, Dashboard 테스트 4개, 생성된 OpenAPI 계약 검사 4개 통과. `main` 반영. |
| [ArtCraftX](./artcraftx) | 이미지·비디오·오디오·3D 생성 화면을 갖춘 별도 데스크톱 앱입니다. | **수정:** 웹 API 요청의 명시적인 `false`·`0`·빈 문자열 보존. | API 라이브러리·앱 테스트 12개와 라이브러리 빌드 통과. `main` 반영. 앞선 실제 Tauri 검증에서는 작업 화면 5개와 네이티브 읽기 명령 4개를 확인했습니다. |
| [FilmCraft](./filmcraft) | 모노레포 최초 스냅샷에 타임라인과 편집·내보내기 도구가 있었습니다. | **추가:** FilmCraft 0.15 통합과 Windows 설치 정책 검사. **수정:** 공식 소스와 Mac·PC 변경을 합치면서 한국어 전사 흐름과 내보내기 준비 동작을 보존하고, 전사 작업의 Undo와 실험적 Premiere 프로젝트 가져오기의 마커 호환성을 수정했습니다. [통합 기록](./filmcraft/docs/source-integration.json) EDL에서 프레임레이트나 드롭 프레임 모드와 맞지 않는 타임코드에 경고를 표시합니다. | 교환 형식 테스트 139개 통과. UI 테스트를 직렬로 재실행해 문서 테스트·WebAssembly를 포함한 제품 전체 검사 통과. `main` 반영. |
| [PhotoCraft](./photocraft) | 레이어와 마스크를 편집하고 PSD 및 일반 이미지로 내보낼 수 있었습니다. | **수정:** 레이어별 파일 내보내기에서 포맷 변환 경고를 UI와 API 결과에 전달하고, 깊게 중첩된 Affinity 문서를 안정적으로 파싱합니다. | Affinity 32개, 레이어 내보내기 엔진·API 4개와 제품 전체 검사 9단계 통과. 첫 실행 메뉴를 한국어·일본어·영어로 확인. `main` 반영. |
| [VectorCraft](./vectorcraft) | SVG 가져오기·내보내기를 지원했으며, 가로·세로 강도가 다른 Gaussian Blur를 하나의 평균값으로 근사했습니다. | **수정:** SVG Gaussian Blur의 가로·세로 강도를 각각 보존해 렌더링하고, SVG로 다시 저장할 때도 두 값을 유지합니다. | 흐림 회귀 테스트 9개와 기본 프로필의 대형 편집 성능 검사 4개, 번들 글꼴 미사용·문서·WebAssembly를 포함한 제품 전체 검사 12단계 통과. `main` 반영. |
| [LightCraft](./lightcraft) | 일부 ORF RAW를 디코딩할 수 있었지만 지원 여부 검사에서 ORF를 제외했습니다. | **수정:** 디코딩 가능한 비압축 ORF가 지원 파일로 인식돼 정상 RAW 디코딩 경로를 사용합니다. | RAW 테스트 472개, 지정 글꼴로 접근성 테스트 재검증, HEIF와 WebAssembly를 포함한 전체 검사 8단계 통과. `main` 반영. |
| [EffectCraft](./effectcraft) | 합성·키프레임·타임라인 내보내기가 있었으며, 프리렌더에서 조정 레이어 효과가 빠질 수 있었습니다. | **수정:** 조정 레이어가 포함된 합성의 프리렌더에 원래 렌더 결과를 반영하고, 편집 가능한 요소가 평면화될 때 경고를 표시합니다. | 교환 형식 15개와 내보내기·재가져오기 통합 테스트 3개 통과. 소켓 테스트의 버퍼 조건을 안정화한 뒤 제품 전체 검사 통과. `main` 반영. |
| [DesignCraft](./designcraft) | 자동 텍스트 재배치가 사용자가 명시적으로 삭제한 페이지를 다시 만들 수 있었습니다. | **수정:** 페이지를 삭제해도 스토리 텍스트를 보존하고, 조판에 영향을 주는 변경에서만 자동 재배치합니다. 아랍어와 macOS 단축키의 글꼴 대체 표시도 보완했습니다. | 엔진 999개와 UI 테스트 738개, 문서·WebAssembly를 포함한 제품 전체 검사 9단계 통과. `main` 반영. |
| [SoundCraft](./soundcraft) | 오디오 편집·믹싱·DSP와 플러그인 호스트를 갖췄습니다. | 추가·수정 없음. | 빌드·기본 실행을 확인했습니다. 별도 기능 변경은 확인되지 않았습니다. |
| [FigCraft](./figcraft) | 프레임·컴포넌트·프로토타입과 SVG 텍스트 편집이 있었습니다. | **수정:** macOS Command와 물리 Ctrl 입력을 구분하고, SVG에서 가져온 텍스트를 편집한 뒤 Undo/Redo할 때 저장 문서 상태를 복원합니다. | 입력·타이포그래피·이력 테스트 12개와 기본 프로필의 제품 전체 검사 통과. `main` 반영. |
| [WordCraft](./wordcraft) | DOCX 편집·저장과 PDF 출력이 있었습니다. | 추가·수정 없음. | 실제 UI에서 DOCX 편집·Undo/Redo·저장·재열기를 확인했습니다. 별도 기능 변경은 확인되지 않았습니다. |
| [DeckCraft](./deckcraft) | 프레젠테이션·PPTX 처리와 단축키 표시가 있었습니다. | **수정:** 중첩된 문서 로더의 스택 사용을 안정화하고, 단축키 글리프가 없으면 텍스트로 표시합니다. | 포맷·UI 회귀 테스트와 문서 테스트·WebAssembly를 포함한 제품 전체 검사 8단계 통과. `main` 반영. |
| [GridCraft](./gridcraft) | XLSX·CSV와 수식 계산을 지원했습니다. | **수정:** LAMBDA 인수를 평가할 때 기존 오류 값을 보존합니다. | 수식 계산 테스트 95개와 지정 글꼴을 사용한 제품 전체 검사 10단계 통과. 생성된 문서와 단축키 파일의 줄바꿈을 검사 기준에 맞췄습니다. `main` 반영. |
| [CadCraft](./cadcraft) | 2D 도면 편집, 기하 제약과 DXF/DWG 처리 경로가 있었습니다. | 추가·수정 없음. | 빌드·기본 실행을 확인했습니다. 별도 기능 변경은 확인되지 않았습니다. |
| [Craft Launcher](./craft-launcher) | 도구 카탈로그와 설치·업데이트·실행을 관리했습니다. | **수정:** Linux 관리 경로에서 심볼릭 링크를 차단하고, `bin`·`share` 갱신 중 오류가 나면 기존 설치를 복원합니다. | 네이티브 테스트 168개, Clippy 및 WebAssembly 검사 통과. 기본 실행에서 제외하도록 표시된 기존 테스트 4개는 제외. `main` 반영. |
| [Craft Libraries](./craft-libs) | UI·미디어·글꼴 등 공통 기능을 제공했습니다. | **수정:** 번들 글꼴 인덱스의 줄바꿈을 고정 체크섬에 맞추고, 글꼴 임베딩에서 UTF-8로 표현할 수 없는 경로를 명시적으로 거부합니다. | 워크스페이스 포맷·Clippy·테스트와 번들 인덱스 고정값 검사 통과. `main` 반영. |
| [Craft i18n](./craft-i18n) | 앱별 번역 카탈로그와 카탈로그 검사 도구를 제공했습니다. | 추가·수정 없음. | 기존 빌드와 검사를 확인했습니다. 별도 기능 변경은 확인되지 않았습니다. |
| [ghidra-hopper-rs](./ghidra-hopper-rs) | 번들 Rust CLI에서 분석·PE 검사·MCP 기능을 제공했습니다. | **추가:** Rust CLI에서 실제 headless Ghidra SDK로 디컴파일하는 [독립 엔진](./ghidra-hopper-rs/sdk-engine). | Ghidra SDK로 테스트 바이너리의 함수 분석을 다시 확인했습니다. |

### 기존 ArtCraft에 없었던 기능

- **편집 가능한 Native 프로젝트** — `.acraft`로 원본 레이어, 마스크, 변형, Levels와 Curves를 보존하고 다시 열 수 있습니다. [프로젝트 구현](./artcraft/artcraft-native/src/pages/image_project.rs)
- **원본 해상도 편집** — 기존 고정 캔버스의 한계를 없애고 원본 크기와 투명도를 유지하는 편집·PNG 저장을 구현했습니다. macOS에서 4097 × 2305 프로젝트의 편집과 재열기를 확인했습니다. [편집기 구현](./artcraft/artcraft-native/src/pages/edit_image.rs)
- **API 없는 모델 선택** — 이미지와 비디오 화면에서 모델을 고르고, 두 화면을 오가도 선택이 유지됩니다. 실제 생성은 미리보기에서 비활성화되어 있습니다. [실행 및 빌드 방법](./artcraft-services/frontend/apps/artcraft-webapp/model-preview)
- **Native에서 도구 실행** — ArtCraft Native의 Craft Suite 메뉴에서 설치된 편집기를 열 수 있습니다. [실행 메뉴 구현](./artcraft/artcraft-native/src/pages/craft_suite.rs)
- **실제 Ghidra SDK 호출** — 별도 Rust 명령으로 headless Ghidra에 분석을 요청하고 디컴파일 결과를 받을 수 있습니다. [SDK 엔진 실행 방법](./ghidra-hopper-rs/sdk-engine)

통합본에서 웹 테스트 45개, Native 테스트 166개, Ghidra 엔진 테스트 165개가 통과했습니다. 브라우저에서 모델 선택, 빈 로그인 입력 차단과 첫 프레임 캡처를 확인했고 실제 Ghidra SDK 디컴파일도 검증했습니다. [기능 통합 커밋](https://github.com/hyegonKim/ArtCraft/commit/4fd6a810f6875c9f52e62d637aed06437060a9b6)

ArtCraft monorepo containing all 19 projects, including the integrated FilmCraft upstream, Mac Codex, and PC changes.

Unified **ArtCraft & Craft Suite** repository containing all 19 applications, libraries, services, and reverse-engineering tools under their respective project directories:

## Changes from the original ArtCraft

### Web changes

| Change | What changed | Source |
| :--- | :--- | :--- |
| Offline model preview | The Image and Video screens each list 22 models from the repository. Each screen keeps its selection while you switch between them. The preview works without an API and disables Generate. | [Build and run the preview](./artcraft-services/frontend/apps/artcraft-webapp/model-preview) |
| Login form validation | Empty credentials and whitespace-only usernames are rejected before an API request. The form announces the validation message to assistive technology. | [Login fix and regression tests](https://github.com/hyegonKim/ArtCraft/commit/a0208f9b73af081b2383ac717200dde84ffe8b72) |

### Desktop editing, frame extraction, and Ghidra

- **Original-resolution editing and PNG export.** ArtCraft Native replaced the fixed 320 × 200 canvas with a document sized to the imported image. Editing and PNG export preserve the document dimensions and transparency. Fit, 100%, zoom, and pan control the view.
- **Layer editing and project files.** Added linked layer masks, non-destructive transforms, RGB and per-channel Curves, continuous brush and eraser strokes, and gesture-level Undo/Redo. Editable `.acraft` files preserve layers, masks, transforms, Levels, and Curves.
- **Video frame extraction.** The web tool can capture the first frame of an imported MP4 before playback. Browser verification covered stepping one frame forward, capturing again, and saving a 640 × 360 PNG.
- **Desktop workflows.** Verified the basic launch and interaction flows for ArtCraft Native and 13 Craft Suite tools. ArtCraftX verification also exercised five workspaces and four native Tauri read commands.
- **Ghidra integration.** Built the Rust engine and verified it against the actual Ghidra SDK. The standalone SDK engine is in [`ghidra-hopper-rs/sdk-engine`](./ghidra-hopper-rs/sdk-engine). The integration covered 16 analysis cases and seven production CLI checks.

The native editor verification covered editing, PNG export, Undo/Redo, and reopening a 4097 × 2305 project on macOS. The web preview was tested in a browser with API access blocked. Live account sign-in, billing, and paid AI generation require a configured backend. The Adobe products listed below are workflow references.

## Applications & Suites

| Directory | Project | Adobe / Industry Counterpart |
| :--- | :--- | :--- |
| [`artcraft/`](./artcraft) | **ArtCraft (`artcraft-native` & Desktop)** | Adobe Creative Cloud / Firefly AI Studio (Photoshop v27.11 GPT Image 2 & Edge Enhancement) |
| [`photocraft/`](./photocraft) | **PhotoCraft** | Adobe Photoshop v27.11 / LTS v26.11.8 & Camera Raw v18.7 |
| [`vectorcraft/`](./vectorcraft) | **VectorCraft** | Adobe Illustrator v30.8.2 / LTS v29.8.11 |
| [`filmcraft/`](./filmcraft) | **FilmCraft** | Adobe Premiere Pro v26.5.2 / v25.6.6 / v24.6.4 & Media Encoder v26.5.2 (`hyegonKim/filmcraft` Korean Transcript base) |
| [`lightcraft/`](./lightcraft) | **LightCraft** | Adobe Lightroom Classic & Camera Raw v18.7 |
| [`effectcraft/`](./effectcraft) | **EffectCraft** | Adobe After Effects 2026 |
| [`designcraft/`](./designcraft) | **DesignCraft** | Adobe InDesign 2026 |
| [`soundcraft/`](./soundcraft) | **SoundCraft** | Adobe Audition 2026 |
| [`figcraft/`](./figcraft) | **FigCraft** | Figma / Adobe XD |
| [`wordcraft/`](./wordcraft) | **WordCraft** | Adobe Acrobat DC / Pro & Word Processor |
| [`cadcraft/`](./cadcraft) | **CadCraft** | AutoCAD 2D/Parametric DCM |
| [`deckcraft/`](./deckcraft) | **DeckCraft** | Presentation & PDF Handouts |
| [`gridcraft/`](./gridcraft) | **GridCraft** | Spreadsheet & Formula Engine |
| [`craft-launcher/`](./craft-launcher) | **Craft Launcher** | Adobe Creative Cloud Desktop (Recent Updates & App Management) |

## Shared Libraries, Services & Tooling

| Directory | Description |
| :--- | :--- |
| [`craft-libs/`](./craft-libs) | Shared UI widgets, print, update, and platform crates |
| [`craft-i18n/`](./craft-i18n) | Localization & internationalization infrastructure |
| [`artcraft-services/`](./artcraft-services) | Backend services and asset pipelines |
| [`artcraftx/`](./artcraftx) | Extended ArtCraft experimental & cross-platform modules |
| [`ghidra-hopper-rs/`](./ghidra-hopper-rs) | Rust-native SLEIGH/P-Code decompiler & Hopper ByteType reverse-engineering engine |
