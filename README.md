# ArtCraft Monorepo

## 기존 ArtCraft 대비 변경 사항

검증한 아래 변경의 소스가 `main`에 반영되어 있습니다. [통합 커밋 `4fd6a81`](https://github.com/hyegonKim/ArtCraft/commit/4fd6a810f6875c9f52e62d637aed06437060a9b6)

| 기능 | 달라진 점 |
| --- | --- |
| [Native 이미지 편집](./artcraft/artcraft-native/src/pages/edit_image.rs) | 320 × 200 고정 캔버스를 제거했습니다. 원본 해상도와 투명도를 유지해 편집하고 PNG로 저장할 수 있습니다. 레이어 마스크, 변형, RGB 및 채널별 Curves, 연속 브러시·지우개, 실행 취소·다시 실행을 추가했습니다. |
| [편집 프로젝트](./artcraft/artcraft-native/src/pages/image_project.rs) | `.acraft`로 레이어, 마스크, 변형, Levels, Curves를 보존하고 다시 열 수 있습니다. |
| [이미지·비디오 모델 미리보기](./artcraft-services/frontend/apps/artcraft-webapp/model-preview) | API 연결 없이 각 화면에서 모델 22개를 고를 수 있습니다. 미리보기의 생성 버튼은 비활성화되어 있습니다. |
| [웹 로그인](./artcraft-services/frontend/apps/artcraft-webapp/src/pages/login/login.tsx) | 빈 비밀번호 또는 비어 있거나 공백뿐인 사용자 이름은 서버 요청 전에 안내합니다. |
| [영상 프레임 추출](./artcraft-services/frontend/apps/artcraft-webapp/src/pages/frame-extractor) | MP4를 재생하기 전에 첫 프레임을 캡처하고 PNG로 저장할 수 있습니다. |
| [Craft Suite 실행 연동](./artcraft/artcraft-native/src/pages/craft_suite.rs) | Native 앱에서 설치된 Craft Suite 도구를 열 수 있습니다. |
| [Ghidra SDK 엔진](./ghidra-hopper-rs/sdk-engine) | Rust CLI에서 실제 Ghidra SDK로 분석할 수 있는 별도 크레이트를 추가했습니다. |

통합본에서 웹 테스트 45개, Native 테스트 166개, Ghidra 엔진 테스트 165개가 통과했습니다. 브라우저에서 모델 선택, 빈 로그인 입력 차단, 첫 프레임 캡처를 확인했고 실제 Ghidra SDK 디컴파일도 검증했습니다.

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
