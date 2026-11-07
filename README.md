# ArtCraft Monorepo

## 기존 ArtCraft 대비 변경 사항

아래 표는 **같은 제품의 기존 버전과 현재 버전**을 비교합니다. 기준은 19개 프로젝트를 처음 모은 커밋 `484843e`입니다. 현재 버전 열에는 GitHub `main`에 반영된 변경을, 마지막 열에는 후속 검증 결과와 반영 상태를 기록합니다. [GitHub 소스 비교](https://github.com/hyegonKim/ArtCraft/compare/484843e...main)

### VPS 클라우드 저장·동기화 — 2026-11-07

[비공개 스테이징](https://artcraft-web-private-staging.netlify.app/)에 기존 VPS의 미디어 저장소와 프로젝트 API를 연결했습니다. 검증한 기능 소스는 [`fbafc550`](https://github.com/hyegonKim/ArtCraft/commit/fbafc5501ff52eb26c4d3059a0cb9a953d877a10)입니다. 최종 배포는 `6aeeee7743023190c3b66265`, 번들은 `assets/index-BnfSdNyv.js`이며 `artcraft-build.json`의 소스와 실제 로드한 번들을 확인했습니다.

| 제품·항목         | 기존 인증 전용 스테이징                                          | 추가·수정 및 실제 검증                                                                                                                                                                                                                                                                                                          |
| ----------------- | ---------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 이미지 편집기     | 클라우드 업로드가 연결되지 않았습니다.                           | 실제 PNG 업로드, 계정 라이브러리 표시, 원본 1280 × 720 이미지와 이력 썸네일 로드, 라이브러리의 Edit on Canvas와 편집기의 Pick from Library → Show uploads → Use selected로 원본 재열기를 확인했습니다.                                                                                                                          |
| 비디오 편집기     | IndexedDB에 프로젝트와 가져온 영상을 저장하는 로컬 모드였습니다. | 계정별 서버 자동 저장과 실패 후 재시도를 연결했습니다. 실제 MP4 가져오기, 타임라인 추가, 이름 변경, 위치 X=12 편집 후 목록에서 재열기·새로고침·저장소 재시작 뒤 복원을 확인했습니다.                                                                                                                                            |
| 비디오 라이브러리 | 내보내기는 디스크 다운로드로 검증했습니다.                       | 클라우드 라이브러리 내보내기·목록 표시·재열기·실제 재생을 확인했습니다. 내려받은 결과는 H.264, 64 × 64, 25fps, 25프레임, 1초입니다. 가져오기·내보내기 파일을 숨김용 중간 파일로 표시하던 기본값도 수정했습니다.                                                                                                                 |
| 3D 편집기         | 캡처 업로드는 실패했고 로컬 PNG 다운로드가 동작했습니다.         | 서버 Save / Load my scene을 연결하고 카메라 X=0.25를 저장·재열기·저장소 재시작 뒤 복원했습니다. 최종 배포의 새 창에서 로그인 유지·장면 불러오기를 재확인했고, Capture로 1280 × 720 PNG를 업로드해 라이브러리 표시와 뷰어의 실제 원본 로드를 확인했습니다.                                                                       |
| 저장소·인증       | MySQL·Redis·인증 API만 실행했습니다.                             | 같은 VPS에 영속 볼륨을 사용하는 SeaweedFS와 경량 게이트웨이를 추가했습니다. 검색 서버는 실행하지 않습니다. 세션·소유권을 검사하고, 파일은 30분 서명 URL로 스트리밍합니다. 원본 바이트 일치·동영상 Range 206, 익명 업로드·목록 401, 다른 계정의 프로젝트 읽기·수정 404, 쿠키 없는 서명 세션, 64MiB 요청 제한 413을 확인했습니다. |
| 자동 검사         | 앞선 전체 테스트는 243개였습니다.                                | 프런트엔드 전체 66개 빌드 대상 통과. 최종 웹앱과 의존성 53개 프로덕션 재빌드, 웹앱 TypeScript 검사, 63개 테스트 대상의 실제 테스트 268개 및 게이트웨이 테스트 14개 통과. 전체 테스트는 유효한 Nx 캐시를 포함합니다. 저장 게이트웨이 중단 시 오류 알림·복구 후 5초 간격 자동 재시도와 서버 저장도 실제 UI에서 통과했습니다.      |
| 비공개 유지       | Production·Deploy Preview가 Private였습니다.                     | Production·Deploy Preview 모두 Private를 유지했습니다. 기본 주소·이미지/3D/비디오 편집·최종 JS 번들·Ammo JS/WASM·빌드 정보·새 배포별 주소의 비로그인 요청 9개가 모두 HTTP 401입니다.                                                                                                                                            |

추가 수정: 최초 저장의 임시 프로젝트 ID와 이전 이름을 재열기에 그대로 쓰던 문제를 서버 메타데이터 기준으로 수정했습니다. 이미지 편집기·공유 뷰어가 서명 URL에 두 번째 `?`를 붙여 접근을 거부당하던 문제를 수정했고, 미디어 URL의 파일 확장자를 유지해 영상이 이미지로 잘못 열리지 않게 했습니다. 스테이징 응답에는 캐시 재검증 설정을 추가했습니다. 무거운 3D 첫 실행에서 세션 응답이 1.5초를 넘으면 로그인을 잃던 문제도 수정했습니다. 늦게 도착한 유효 응답을 적용하고, 일시적 통신 실패에는 기존 로그인 상태를 보존하며, 로그아웃 뒤 도착한 이전 응답은 무시합니다. 테스트용 개발 JSX가 배포물에 섞여 초기화가 중단되던 문제는 의존성까지 프로덕션 Nx 빌드로 다시 만드는 전용 빌드 스크립트로 수정했습니다. 개발용 `jsxDEV` 호출이 남은 산출물은 ZIP 생성 전에 차단하며, 실제 실패 번들로 차단 검사를 통과했습니다.

범위: 프로젝트 Save / Open UI는 **비디오·3D**에 연결했습니다. 이미지 업로드·라이브러리 재열기는 지원하지만, **2D 레이어 프로젝트의 Save / Open UI는 아직 연결하지 않았습니다**. 2D 프로젝트 생성·수정 API는 실제 서버 검사에서 통과했습니다. 기존 브라우저 로컬 프로젝트의 자동 이관, 실시간 공동 편집·충돌 병합, 오프라인 업로드 큐, 비동기 썸네일 워커는 포함하지 않습니다. 64MiB는 multipart를 포함한 요청 본문 제한입니다. 미디어 서명 URL은 발급 후 30분 동안 그 URL을 가진 사람이 사용할 수 있으므로 공개하지 않아야 합니다. 유료 생성과 결제는 실행하지 않았습니다. 배포 방법과 메모리·영속 저장 구성이 [자체 호스팅 README](./artcraft-services/build/selfhost/README.md#private-staging-cloud-storage)에 있습니다.

### 이전 WASM 수정·세 편집기 실제 UI 검증 — 2026-11-07

[비공개 스테이징](https://artcraft-web-private-staging.netlify.app/)에 게시한 앱의 기준 소스는 [`ddc9da82`](https://github.com/hyegonKim/ArtCraft/commit/ddc9da82e0b4e3d21730bac4a607d2fa990f0c28), 최종 배포 ID는 `6aeec364f5b4bc32e48a40e3`, 번들은 `assets/index-Ca42bwNO.js`입니다. 배포된 `artcraft-build.json`의 커밋을 확인하고 실제 Chrome에서 아래 작업을 수행한 뒤 검증본을 `main`에 반영했습니다.

**수정:** 스테이징 CSP에서 내장 WASM의 `data:` 읽기와 로컬 영상 복원의 `blob:` 읽기를 허용했습니다. 외부 API 출처·경로 허용 목록은 유지합니다. Ammo JS·WASM을 같은 출처의 루트 경로에 함께 배포하고, 중복 초기화·실패 후 재시도를 처리합니다. 3D 캡처는 업로드가 실패해도 다운로드할 수 있습니다. 비디오 프로젝트 목록 오류 화면에서도 새 편집을 시작할 수 있습니다.

**로컬 비디오 편집:** 이 스테이징은 `VITE_ARTCRAFT_LOCAL_EDITING=true`로 빌드했습니다. 프로젝트와 원본 영상은 이 브라우저의 IndexedDB에 저장하고 결과는 로컬 파일로 내보냅니다. 기본 서버 기반 어댑터는 이 옵션을 켜지 않은 빌드에서 유지됩니다.

| 검증 대상 | 최종 결과 |
| -------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Spark WASM     | 이전 CSP에서 `Failed to fetch`를 재현한 뒤, 실제 내장 WASM 32,266바이트 읽기·컴파일과 내보내기 6개 확인을 통과했습니다.                                                 |
| Ammo 물리 WASM | 설치된 Three.js의 JS·WASM 쌍을 배포물에 포함했습니다. 브라우저에서 런타임 초기화와 벡터 `(1, 2, 3)` 계산을 확인했고, 최종 3D 진입 로그에 해당 초기화 오류가 없었습니다. |
| 이미지 편집기  | Full HD 1920 × 1080 빈 캔버스 생성, 실제 브러시 획, Undo로 획 제거·Redo로 복원을 최종 배포에서 확인했습니다.                                                            |
| 3D 편집기      | Cube 드래그 추가, 위치 변경·Undo·Redo, Build ↔ Record 전환 후 객체 유지, 프레임 캡처와 로컬 PNG 다운로드를 확인했습니다. 내려받은 PNG는 1280 × 720입니다.               |
| 비디오 편집기  | 새 프로젝트, 로컬 MP4 가져오기·미리보기, 길이 트림·Undo·Redo, 프로젝트 이름 변경, 디스크 MP4 내보내기, 새로고침 후 프로젝트·원본 영상·트림 복원과 재생을 확인했습니다.  |
| 실제 MP4 파일  | H.264, 64 × 64, 25fps, 13프레임, 0.52초. 1초짜리 테스트 영상을 트림한 결과와 일치했습니다. 내보내기에서 Save to disk만 선택했습니다.                                    |
| 자동 검사      | Nx 테스트 63개 대상 실행·실제 테스트 243개 통과. 영향받은 3D 25개·웹앱 46개 테스트, 웹앱 TypeScript 검사, 3D 라이브러리·웹앱 프로덕션 빌드 통과.                        |
| 비공개 접근    | Production·Deploy Preview 모두 Private. 기본·이미지 편집·3D 편집·비디오 편집·새 번들·Ammo JS·Ammo WASM·새 배포별 주소의 비로그인 HTTP 요청 8개 모두 401.                |

당시 범위: 아래는 클라우드 연결 전 인증 전용 배포의 기록이며, 당시 클라우드 갤러리 업로드·서버 프로젝트 동기화는 지원하지 않았습니다. 현재 지원 범위는 위 클라우드 검증에 기록했습니다. 3D 캡처의 서버 업로드는 실패 상태를 유지하며 로컬 다운로드가 동작하는 것을 확인했습니다. 유료 AI 생성, 결제, 모든 영상 포맷·코덱과 브라우저의 호환성을 검증한 결과는 아닙니다. 아래 기록의 WASM 초기화·로컬 영상 복원 오류는 위 수정 전 관찰입니다.

### 이전 인증 스테이징 재배포·실제 검증 — 2026-11-07

[ArtCraft 비공개 스테이징](https://artcraft-web-private-staging.netlify.app/)을 [검증된 프런트엔드 소스 `cbd1dc09`](https://github.com/hyegonKim/ArtCraft/commit/cbd1dc095f0107b32263dc8e4730bf48a7aa3618)로 다시 빌드해 기존 사이트에 게시했습니다. 아래는 새 배포 주소에서 실제 Chrome UI와 비로그인 HTTP 요청으로 확인한 결과입니다.

| 항목          | 실제 배포·검증 결과                                                                                                                         |
| ------------- | ------------------------------------------------------------------------------------------------------------------------------------------- |
| 기준 소스     | GitHub main 검증본 `cbd1dc095f0107b32263dc8e4730bf48a7aa3618`. 배포된 `artcraft-build.json`의 커밋과 브라우저가 불러온 번들을 확인했습니다. |
| 게시 배포     | `6aeeae405ca34d4620ca48ee` · 2026-11-07 13:51 KST · `assets/index-AeQiLn3A.js`.                                                             |
| 빌드·설정     | 프로덕션 Vite 빌드 통과. 기존 스테이징 인증 API·Google 웹 클라이언트와 CSP·라우팅 설정을 적용했습니다.                                      |
| 이메일 인증   | 새 전용 테스트 계정 가입 성공·환영 화면 확인, 로그아웃 후 이메일 재로그인, 새로고침 후 세션 유지 모두 통과했습니다.                         |
| Google 인증   | 공식 Google 버튼에서 기존 테스트 계정을 선택해 `/pricing`으로 이동했고, 사용자 메뉴와 새로고침 후 같은 계정의 세션 유지를 확인했습니다.     |
| 이미지 모델   | 내장 모델 22개를 각각 모두 클릭해 선택 반영을 확인했습니다. 모든 선택에서 Create가 비활성 상태였습니다.                                     |
| 비디오 모델   | 내장 모델 22개를 각각 모두 클릭해 선택 반영을 확인했습니다. 모든 선택에서 Create가 비활성 상태였습니다.                                     |
| 프롬프트·생성 | 두 화면 모두 비어 있지 않은 프롬프트를 입력한 뒤에도 Create가 비활성 상태였습니다.                                                          |
| 비공개 설정   | Netlify Production visibility와 Deploy Preview visibility 모두 Private로 유지했습니다.                                                      |
| 비로그인 차단 | 쿠키 없는 HTTP 요청 8개 모두 401: 기본 주소, 가입, 로그인, 이미지, 비디오, 새 번들, 빌드 정보, 새 배포별 주소.                              |

모델 검증은 목록 표시뿐 아니라 **이미지 22개·비디오 22개, 총 44개 모델 전체의 실제 선택**을 포함합니다. 실제 AI 생성과 결제는 실행하지 않았고, 이메일 수신·비밀번호 재설정 메일과 모든 편집기의 GPU 동작은 이번 검증 범위에 포함하지 않았습니다. 초기 로드에서 별도 WASM 초기화 요청의 `Failed to fetch` 로그가 관찰됐으며, 위 인증·모델 선택 검증은 완료했습니다.

이 문서 갱신은 새 앱 배포를 만들지 않습니다. 게시된 앱의 기준 소스와 배포 ID는 위 값으로 유지하며, 공개 배포는 하지 않았습니다. 테스트 계정 자격증명과 서버 비밀값은 저장소에 포함하지 않습니다.

### 초기 인증 스테이징 배포와 실제 검증 — 2026-11-07

웹 프런트엔드는 [비공개 Netlify 스테이징](https://artcraft-web-private-staging.netlify.app/)에, 인증 API는 Hostinger 실행 VPS에 배포했습니다. 별도 Hostinger VPS에서 Linux 이미지를 빌드한 뒤 실행 VPS로 옮겼습니다. Google 로그인은 ArtCraft Staging 전용 프로젝트의 웹 클라이언트와 테스트 계정으로 검증했습니다.

| 항목         | 배포 구성 / 실제 확인 결과                                                                                                        |
| ------------------ | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 인증 서버      | storyteller-web + MySQL 8.4 + Redis 7. 격리된 Compose 프로젝트·볼륨을 사용하고 API를 127.0.0.1:12346에 바인딩합니다.                          |
| HTTPS      | artcraft-auth-staging.ncubestudio.cloud. 기존 Traefik과 Let’s Encrypt 인증서를 사용하며 지정한 스테이징 출처·인증 경로·메서드만 라우팅합니다.             |
| 프런트엔드      | VITE_ARTCRAFT_API_ORIGIN과 VITE_GOOGLE_CLIENT_ID로 연결합니다. Google SDK의 실제 버튼을 표시하고 CSP에 필요한 인증 경로와 Google 스크립트·프레임을 허용합니다. |
| 이메일 가입·로그인 | 실제 Netlify 주소에서 테스트 계정 생성, 계정 생성 성공·환영 화면 확인, 로그아웃, 같은 계정으로 재로그인, 사용자 메뉴 표시를 확인했습니다.                                    |
| Google 로그인 | 실제 Google 계정 선택과 사용자 동의를 거쳐 /pricing으로 이동하고 사용자 메뉴가 표시됐습니다. 새로고침 후에도 로그인 세션이 유지됐습니다.                                    |
| 접근 제어      | Netlify 기본 주소·배포별 주소·웹 번들에 대한 비로그인 요청은 401입니다. 인증 API의 다른 출처와 생성·관리 경로는 404로 차단됐습니다.                                    |
| 모델 미리보기    | 이미지·비디오 화면 각각 22개 모델 표시, 다른 모델 선택 반영, Create 비활성 상태를 실제 배포 주소에서 확인했습니다.                                                 |
| 소스 검사      | CORS 회귀 테스트 79개와 웹 로그인·브리지 테스트 26개 통과. Rust 백엔드 검사·Linux 바이너리/런타임 이미지 빌드, 개별 Vite 라이브러리·웹앱 빌드, Compose 병합 검사를 통과했습니다.   |

초기 검증 배포는 `6aee8f4dd234b04876ca4ed6`이며 빌드의 기준 소스는 `678be91`에 이번 인증 수정을 적용한 상태입니다. 게시 중 확인된 개발용 JSX 혼입 오류는 프로덕션 라이브러리를 다시 빌드해 수정했고, 실제 가입·로그인 검증은 수정 배포에서 수행했습니다. 당시 남아 있던 Nx 프로젝트 그래프·전체 빌드 문제의 후속 검증은 아래에 별도로 기록했습니다. 위 표의 실제 인증 결과는 해당 배포에서 확인한 결과입니다.

Netlify 접근 제어는 프로젝트의 비공개 설정에서 관리합니다. 이 소스 커밋이 Netlify 사이트를 자동 재배포하지는 않습니다. 이메일 수신·비밀번호 재설정 메일, 결제, 유료 AI 생성은 이번 검증 범위에 포함하지 않았습니다. 서버 비밀값과 테스트 계정 자격증명은 저장소에 포함하지 않습니다. [실행 구성과 재현 방법](./artcraft-services/build/selfhost/README.md#private-staging-authentication)

### 프런트엔드 전체 재검증 — 2026-11-07

Node.js 22.23.4에서 독립된 의존성을 설치하고 `npm ci --ignore-scripts --no-audit --no-fund`로 잠금 파일의 재현성을 확인했습니다. 이전 GenHub의 `getStatus` 그래프 오류는 새 설치와 캐시 없는 그래프 재생성에서 재현되지 않았습니다. 전체 실행을 막던 PageDraw·PageScene의 TypeScript 프로젝트 참조를 `nx sync`로 동기화했습니다.

추가로 잘못된 `build-env` 공개 내보내기를 수정하고, 설치된 Three.js 0.171에 맞는 타입 의존성과 변환 이벤트 선언을 추가했습니다. 3D 카메라 초점 이동은 현재 `PointerLockControls.object`를 사용하고 빈 선택을 처리하며, WebGL2에서 불필요한 구형 셰이더 확장 설정을 제거했습니다. 품질 선택기의 Auto·Max·Extra High 라벨도 보완했습니다. 오래된 컴포넌트 테스트를 현재 속성·내보내기에 맞게 갱신하고, jsdom의 브라우저·네이티브 경계를 명시했습니다.

| 검증 대상 | 최종 결과 |
| --------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| Nx 그래프·참조 | 캐시 없는 프로젝트 그래프 생성과 `nx sync:check` 통과. |
| Nx 전체 빌드 | 66개 프로젝트 통과. 선언 파일 생성 로그의 TypeScript 오류 0건. |
| Nx 전체 테스트 | 63개 대상 실행 성공. 67개 테스트 파일에서 실제 테스트 236개 통과. 이 중 빈 테스트 파일 1개는 테스트 수에 포함하지 않으며, 별도 18개 대상은 테스트 파일이 없습니다. |
| Dashboard | 고정 pnpm 잠금 파일 설치, TypeScript·프로덕션 Vite 빌드, 테스트 4개 통과. |
| Next 웹사이트 | `npm ci`, 프로덕션 빌드, `typecheck`, Playwright 미디어 브라우저 테스트 31개 통과. 브라우저 테스트는 로컬 픽스처 API를 사용했습니다. |
| 비공개 스테이징 | 기본 주소·기존 배포별 주소·게시 중 JavaScript 번들에 대한 비로그인 요청이 모두 HTTP 401. 기존 배포와 비공개 설정을 유지했습니다. |

Nx 명령은 `artcraft-services/frontend`에서 실행했습니다. 테스트 파일이 없는 대상도 전체 실행에 포함하기 위해 `--passWithNoTests`를 사용했으며 실패한 테스트를 제외하거나 건너뛰도록 바꾸지 않았습니다.

```bash
NX_DAEMON=false NX_CACHE_PROJECT_GRAPH=false npx nx sync:check
NODE_ENV=production NX_DAEMON=false NX_CACHE_PROJECT_GRAPH=false NODE_OPTIONS=--max-old-space-size=6144 npx nx run-many -t build --all --parallel=2 --skip-nx-cache
NX_DAEMON=false NODE_OPTIONS=--max-old-space-size=4096 npx nx run-many -t test --all --parallel=1 --excludeTaskDependencies --skip-nx-cache -- --passWithNoTests --maxWorkers=2
```

기존 React 관련 peer dependency·폐기 예정 API·번들 크기 경고는 남아 있습니다. 이번 결과는 로컬 전체 빌드·자동화 테스트 결과이며 새 Netlify 배포나 모든 편집기의 실제 GPU/UI 재검증을 뜻하지 않습니다. 이후 실제 재배포·인증·모델 선택 결과는 맨 위의 최신 비공개 스테이징 검증에 기록했습니다.

### 제품별 비교

| 제품 | 기존 버전 | 현재 `main`의 추가·수정 | 후속 검증 및 반영 상태 |
| ---------------------------------------- | ------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| [ArtCraft Native / Desktop](./artcraft) | Native 이미지 편집은 320 × 200 고정 캔버스를 사용했습니다. | **추가:** 연결 레이어 마스크, 비파괴 변형, RGB·채널별 Curves, 편집 가능한 `.acraft`, Craft Suite 실행 메뉴. **수정:** 원본 크기와 투명도를 유지하는 편집·PNG 저장, 연속 브러시·지우개와 획 단위 Undo/Redo, 웹 API 요청의 명시적인 `false`·`0`·빈 문자열 보존. | API 라이브러리·앱 테스트 13개와 라이브러리 빌드 통과. `main` 반영. |
| [ArtCraft Services](./artcraft-services) | 모델 선택 화면은 API 목록에 의존했습니다. 빈 로그인도 요청을 보냈고, 재생 전 첫 프레임 캡처가 실패할 수 있었습니다. | **추가:** API 없이 이미지·비디오 모델을 각 22개 고르는 독립 미리보기와 HTTPS 인증 스테이징. **수정:** 일반 웹앱에서도 모델 API에 연결할 수 없으면 내장 목록으로 전환하고 생성 버튼을 비활성화합니다. 환경변수로 자체 인증 API·Google 웹 클라이언트를 연결하고 공식 Google 버튼을 표시합니다. Nx 프로젝트 참조·빌드 환경 진입점·Three.js 타입과 카메라 접근을 수정했습니다. 빈 로그인 입력을 요청 전에 차단하고, 재생 전 첫 프레임 캡처를 안정화했습니다. 웹 API·Dashboard 요청의 명시적인 `false`·`0`·빈 문자열을 보존하고, OpenAPI의 미해결 참조 4개와 경로 매개변수 127곳을 바로잡았습니다. **추가:** 현재 VPS의 계정별 클라우드 업로드·비디오/3D 프로젝트 서버 저장·자동 저장 재시도, 서명 링크와 파일 소유권 검사를 연결했습니다. | 실제 이메일 가입·재로그인·Google 로그인·새로고침 후 세션 유지, Netlify 비로그인 401 차단, CORS 79개·웹 인증 26개 통과. 후속 Nx 66개 전체 빌드·실제 테스트 236개, Dashboard·Next 빌드와 테스트 4개·31개 통과. `cbd1dc09` 재배포 후 이메일·Google 인증, 모델 44개 전체 선택·Create 비활성화를 확인했습니다. 후속 WASM·편집기 수정본 `ddc9da82`에서 이미지·3D·비디오 실제 편집과 다운로드, 로컬 영상 재열기, 비로그인 8개 경로 401을 확인했습니다. 웹 API 테스트 10개, Dashboard 테스트 4개, 생성된 OpenAPI 계약 검사 4개 통과. `main` 반영. 최신 클라우드 연결·실제 UI·전체 검사 결과는 위 VPS 클라우드 검증 표에 기록했습니다. |
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
| --------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------- |
| Offline model preview | The Image and Video screens each list 22 models from the repository. Each screen keeps its selection while you switch between them. The preview works without an API and disables Generate. | [Build and run the preview](./artcraft-services/frontend/apps/artcraft-webapp/model-preview) |
| Login form validation | Empty credentials and whitespace-only usernames are rejected before an API request. The form announces the validation message to assistive technology. | [Login fix and regression tests](https://github.com/hyegonKim/ArtCraft/commit/a0208f9b73af081b2383ac717200dde84ffe8b72) |

### Desktop editing, frame extraction, and Ghidra

- **Original-resolution editing and PNG export.** ArtCraft Native replaced the fixed 320 × 200 canvas with a document sized to the imported image. Editing and PNG export preserve the document dimensions and transparency. Fit, 100%, zoom, and pan control the view.
- **Layer editing and project files.** Added linked layer masks, non-destructive transforms, RGB and per-channel Curves, continuous brush and eraser strokes, and gesture-level Undo/Redo. Editable `.acraft` files preserve layers, masks, transforms, Levels, and Curves.
- **Video frame extraction.** The web tool can capture the first frame of an imported MP4 before playback. Browser verification covered stepping one frame forward, capturing again, and saving a 640 × 360 PNG.
- **Desktop workflows.** Verified the basic launch and interaction flows for ArtCraft Native and 13 Craft Suite tools. ArtCraftX verification also exercised five workspaces and four native Tauri read commands.
- **Ghidra integration.** Built the Rust engine and verified it against the actual Ghidra SDK. The standalone SDK engine is in [`ghidra-hopper-rs/sdk-engine`](./ghidra-hopper-rs/sdk-engine). The integration covered 16 analysis cases and seven production CLI checks.

The native editor verification covered editing, PNG export, Undo/Redo, and reopening a 4097 × 2305 project on macOS. The web preview was tested in a browser with API access blocked. Email sign-up, password sign-in, and Google sign-in were verified against the private staging authentication backend on 2026-11-07. Billing and paid AI generation were outside this authentication verification. The Adobe products listed below are workflow references.

## Applications & Suites

| Directory | Project | Adobe / Industry Counterpart |
| ------------------------------------- | ------------------------------------------ | --------------------------------------------------------------------------------------------------------------------- |
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
| ------------------------------------------- | --------------------------------------------------------------------------------- |
| [`craft-libs/`](./craft-libs) | Shared UI widgets, print, update, and platform crates |
| [`craft-i18n/`](./craft-i18n) | Localization & internationalization infrastructure |
| [`artcraft-services/`](./artcraft-services) | Backend services and asset pipelines |
| [`artcraftx/`](./artcraftx) | Extended ArtCraft experimental & cross-platform modules |
| [`ghidra-hopper-rs/`](./ghidra-hopper-rs) | Rust-native SLEIGH/P-Code decompiler & Hopper ByteType reverse-engineering engine |
