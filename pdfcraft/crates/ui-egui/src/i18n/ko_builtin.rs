//! Built-in Korean (`ko`, `한국어`) UI catalog fallback when `CRAFT_I18N_DIR` is not embedded.
//!
//! Uses `craft-i18n` TSV syntax (`context\tsource\ttranslation`). Every placeholder `{...}` and
//! trailing ellipsis `…` matches the English source string so `I18N::check_all` passes cleanly.

pub const BUILTIN_KO_TSV: &str = "\
# Built-in Korean (한국어) UI translations for PdfCraft
@id\tlanguage.name\t한국어

# Menus & navigation chrome
\tMenu\t메뉴
\tFile\t파일
\tEdit\t편집
\tView\t보기
\tPages\t페이지
\tSign\t서명
\tWindow\t창
\tHelp\t도움말
\tOpen Recent\t최근 파일 열기
\tCreate\t만들기
\tExport\t내보내기
\tProtect\t보호
\tZoom\t확대/축소
\tRotate view\t보기 회전
\tPage navigation\t페이지 탐색
\tPage display\t페이지 표시
\tSide panels\t사이드 패널
\tDisplay theme\t화면 테마
\tTools\t도구
\tFill & Sign\t작성 및 서명
\tHome\t홈
\tAll tools\t모든 도구
\tRecent\t최근 항목
\tStarred\t즐겨찾기
\tFolders\t폴더
\tSearch…\t검색…
\tClose\t닫기
\tCancel\t취소
\tOK\t확인
\tApply\t적용
\tSave\t저장
\tSave as…\t다른 이름으로 저장…
\tOpen…\t열기…
\tRevert\t되돌리기
\tPrint…\t인쇄…
\tCopy\t복사
\tCut\t잘라내기
\tPaste\t붙여넣기
\tDelete\t삭제
\tSelect all\t모두 선택
\tUndo\t실행 취소
\tRedo\t다시 실행
\tFind…\t찾기…
\tReplace…\t바꾸기…
\tAdvanced search…\t고급 검색…
\tPreferences…\t환경설정…
\tDocument properties…\t문서 속성…
\tWord count…\t단어 수…
\tSummarize document…\t문서 요약…
\tPreflight…\t프리플라이트…
\tContent Credentials…\t콘텐츠 자격 증명…
\tNo recent files\t최근 파일 없음
\tClear Recent Files\t최근 파일 지우기
\tPin folder to Home…\t홈에 폴더 고정…
\tNew blank PDF\t새 빈 PDF
\tCombine files…\t파일 결합…
\tClose file\t파일 닫기
\tClose all\t모두 닫기
\tSelect\t선택
\tHand\t손 도구

# Side panels
\tComments\t주석
\tBookmarks\t책갈피
\tFields\t필드
\tLayers\t레이어
\tAttachments\t첨부 파일
\tSignatures\t서명
\tAccessibility Checker\t접근성 검사기
\tTags\t태그
\tSearch\t검색
\tCompare\t비교

# All tools categories & groups
\tCreate & Edit\t만들기 및 편집
\tReview & Compare\t검토 및 비교
\tForms & Signatures\t양식 및 서명
\tProtect & Standardize\t보호 및 표준화
\tCustomize & Automate\t사용자 지정 및 자동화
\tCreate a PDF\tPDF 만들기
\tEdit a PDF\tPDF 편집
\tExport a PDF\tPDF 내보내기
\tCombine files\t파일 결합
\tOrganize pages\t페이지 구성
\tScan & OCR\t스캔 및 OCR
\tCompress a PDF\tPDF 압축
\tAdd comments\t주석 추가
\tAdd a stamp\t스탬프 추가
\tCompare files\t문서 비교
\tCompare files…\t문서 비교…
\tSelect files to compare\t비교할 파일 선택
\tMeasure objects\t개체 측정
\tPrepare a form\t양식 준비
\tUse a certificate\t인증서 사용
\tProtect a PDF\tPDF 보호
\tRedact a PDF\tPDF 교정
\tPrepare for accessibility\t접근성 준비
\tApply PDF standards\tPDF 표준 적용
\tUse print production\t인쇄 제작 사용
\tUse guided actions\t가이드 동작 사용
\tAdd search index\t검색 색인 추가
\tUse JavaScript\tJavaScript 사용
\tAI assistant\tAI 도우미
\tOptional\t선택 사항

# Bates, Actions, Standards, Preflight, C2PA, Summary
\tBates numbering\t베이츠 번호 매기기
\tAdd Bates numbering…\t베이츠 번호 매기기 추가…
\tRemove Bates numbering\t베이츠 번호 매기기 제거
\tAction Wizard\t동작 마법사
\tAction Wizard…\t동작 마법사…
\tPrepare for distribution\t배포 준비
\tPrepare for distribution…\t배포 준비…
\tOptimize scanned documents\t스캔한 문서 최적화
\tOptimize scanned documents…\t스캔한 문서 최적화…
\tStandards\t표준
\tPDF/A…\tPDF/A…
\tSave as PDF/A\tPDF/A로 저장
\tSave as PDF/X\tPDF/X로 저장
\tSave as PDF/UA\tPDF/UA로 저장
\tVerify\t검증
\tPreflight\t프리플라이트
\tPrint production\t인쇄 제작
\tPDF/A-2b archival\tPDF/A-2b 보존용
\tPDF/A-3b archival\tPDF/A-3b 보존용
\tOnline & digital publishing\t온라인 및 디지털 출판
\tInteractive actions & hidden content\t대화형 동작 및 숨겨진 콘텐츠
\tProfile:\t프로파일:
\tChecks effective image resolution, trim and bleed boxes, output intents, and embedded fonts.\t유효 이미지 해상도, 재단 및 도련 상자, 출력 의도, 임베딩된 글꼴을 검사합니다.
\tChecks ISO 19005-2 structural rules for self-contained archival documents.\t자체 완결형 보존 문서를 위한 ISO 19005-2 구조 규칙을 검사합니다.
\tChecks ISO 19005-3 structural rules including associated embedded files.\t연결된 첨부 파일을 포함한 ISO 19005-3 구조 규칙을 검사합니다.
\tChecks document title, language tag, tagged structure, embedded fonts, and encryption.\t문서 제목, 언어 태그, 태그 구조, 임베딩된 글꼴 및 암호화를 검사합니다.
\tAudits scripts, launch and external actions, attachments, unapplied redactions, and XFA.\t스크립트, 외부 실행 동작, 첨부 파일, 미적용 교정 표시 및 XFA를 감사합니다.
\tErrors: {e}  ·  Warnings: {w}  ·  Info: {i}\t오류: {e}  ·  경고: {w}  ·  정보: {i}
\tStructural preflight checks found no problems for this profile.\t이 프로파일에 대한 구조적 프리플라이트 검사에서 문제가 발견되지 않았습니다.
\tError\t오류
\tWarning\t경고
\tInfo\t정보
\tPage {p}\t{p}페이지
\tAnalyze\t분석
\tCopy report\t보고서 복사
\tContent Credentials\t콘텐츠 자격 증명
\tInspects C2PA JUMBF manifests in Associated Files, XMP metadata, and embedded images.\t연결된 파일, XMP 메타데이터 및 포함된 이미지의 C2PA JUMBF 매니페스트를 검사합니다.
\tNo Content Credentials were found in this document.\t이 문서에서 콘텐츠 자격 증명을 찾을 수 없습니다.
\tXMP provenance: {uri}\tXMP 출처 링크: {uri}
\tC2PA manifests found: {n}\t발견된 C2PA 매니페스트: {n}개
\tProduced with: {tool}\t제작 도구: {tool}
\tAsset title: {title}\t자산 제목: {title}
\tAssertions: {list}\t어설션: {list}
\tSigned by: {subject} (Issuer: {issuer})\t서명자: {subject} (발급자: {issuer})
\tCopy summary\t요약 복사
\tSummarize\t문서 요약
\tDocument summary\t문서 요약
\tDocument summarization stopped unexpectedly.\t문서 요약 작업이 예기치 않게 중단되었습니다.
\tKey points:\t핵심 요약 수:
\tRuns locally using TF-IDF key terms and sentence centrality.\tTF-IDF 핵심 용어와 문장 중심도를 사용하여 로컬에서 실행됩니다.
\tSummarizing… {n} pages read\t요약 중… {n}페이지 읽음
\tWords: {w}  ·  Reading time: {m} min\t단어 수: {w}  ·  예상 읽기 시간: {m}분
\tKey terms: {terms}\t핵심 키워드: {terms}
\tNo extractable sentences found. Scanned images need OCR first.\t추출 가능한 문장이 없습니다. 스캔한 이미지는 먼저 OCR을 실행하세요.
\tCopy Markdown\tMarkdown 복사

# Word count & Document Properties
\tWord count\t단어 수
\tCurrent page only\t현재 페이지만
\tExclude tagged tables\t태그된 표 제외
\tExclude tagged code blocks\t태그된 코드 블록 제외
\tExclude image/OCR text\t이미지/OCR 텍스트 제외
\tWords\t단어 수
\tCharacters\t글자 수
\tCharacters without spaces\t글자 수(공백 제외)
\tPages counted\t집계된 페이지 수
\tCounting words… {n} pages read\t단어 수 계산 중… {n}페이지 읽음
\tDocument Properties\t문서 속성
\tDescription\t설명
\tInitial View\t초기 보기
\tSecurity\t보안
\tFonts\t글꼴
\tCustom\t사용자 지정
\tAdvanced\t고급
\tSpace Audit\t공간 사용 감사

# View, Zoom, Theme & Preferences
\tActual size\t실제 크기
\tZoom to page level\t페이지에 맞춤
\tFit to width\t너비에 맞춤
\tFit to height\t높이에 맞춤
\tFit visible\t보이는 영역에 맞춤
\tZoom in\t확대
\tZoom out\t축소
\tMarquee zoom\t영역 확대
\tSingle page\t단일 페이지
\tTwo-page view\t두 페이지 보기
\tContinuous scrolling\t연속 스크롤
\tShow gaps between pages\t페이지 간격 표시
\tFull screen mode\t전체 화면 모드
\tRead mode\t읽기 모드
\tSplit\t분할
\tUse system setting\t시스템 설정 사용
\tLight gray\t밝은 회색
\tDark gray\t어두운 회색
\tPreferences\t환경설정
\tLanguage\t언어

# Signatures, Redaction, OCR, Export & Optimize
\tDigitally sign\t디지털 서명
\tCertify (visible signature)\t인증(보이는 서명)
\tCertify (invisible signature)\t인증(보이지 않는 서명)
\tValidate all signatures\t모든 서명 검증
\tSignatures panel\t서명 패널
\tSignature valid\t서명이 유효합니다
\tValidity unknown\t유효성을 알 수 없습니다
\tSignature invalid\t서명이 유효하지 않습니다
\tDigitally signed by\t디지털 서명자:
\tDate: \t날짜: 
\tReason: \t사유: 
\tLocation: \t위치: 
\tDN: \tDN: 
\tRedact text and images\t텍스트 및 이미지 교정
\tRedact pages…\t페이지 교정…
\tFind text and redact…\t텍스트 찾기 및 교정…
\tRedaction properties…\t교정 속성…
\tApply redactions…\t교정 적용…
\tClear redaction marks\t교정 표시 지우기
\tRemove hidden information…\t숨겨진 정보 제거…
\tSanitize document…\t문서 정리…
\tRecognize text…\t텍스트 인식…
\tEnhance scanned file…\t스캔한 파일 보정…
\tEnhance camera image\t카메라 이미지 보정
\tReduce file size…\t파일 크기 축소…
\tOptimize PDF…\tPDF 최적화…
\tExport to image…\t이미지로 내보내기…
\tExport to text…\t텍스트로 내보내기…
\tExport to Word…\tWord로 내보내기…
\tExport to PowerPoint…\tPowerPoint로 내보내기…
\tExport to HTML…\tHTML로 내보내기…
\tExport to RTF…\tRTF로 내보내기…
\tImage pasted\t이미지를 붙여넣었습니다
\tNo image on clipboard to paste\t클립보드에 붙여넣을 이미지가 없습니다
\tPinned {tool} to Home\t홈에 {tool} 고정됨
\tUnpinned {tool} from Home\t홈에서 {tool} 고정 해제됨

# Plurals (Korean CLDR has 1 form: other)
@plural\t{n} page|{n} pages\t{n}페이지
@plural\t{n} file|{n} files\t파일 {n}개
@plural\t{n} word|{n} words\t단어 {n}개
@plural\t{n} match|{n} matches\t일치 항목 {n}개
@plural\t{n} comment|{n} comments\t주석 {n}개
@plural\t{n} item|{n} items\t항목 {n}개
";
