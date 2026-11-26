//! Interface localization (`en` and `ko`).
//!
//! [`Locale::En`] is the default. When [`Locale::Ko`] is active, [`SlideApp::note_ui_text`]
//! registers [`HANGUL_UI_GLYPHS`] with [`crate::uifonts::UiFonts`] so an installed Hangul outline
//! font (such as Malgun Gothic, Apple SD Gothic Neo or Noto Sans CJK) is borrowed automatically.
//! [`tr_ui`] checks glyph coverage before returning non-ASCII text so `egui` 0.37 never encounters
//! a missing glyph on headless systems without a Korean font installed.

use serde::{Deserialize, Serialize};

/// Supported UI display languages.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Locale {
    #[default]
    En,
    Ko,
}

impl Locale {
    pub const ALL: [Locale; 2] = [Locale::En, Locale::Ko];

    pub fn parse(code: &str) -> Option<Self> {
        let c = code.trim();
        if c.eq_ignore_ascii_case("en") || c.eq_ignore_ascii_case("en-us") || c.eq_ignore_ascii_case("english") {
            Some(Self::En)
        } else if c.eq_ignore_ascii_case("ko") || c.eq_ignore_ascii_case("ko-kr") || c.eq_ignore_ascii_case("korean") || c == "한국어" {
            Some(Self::Ko)
        } else {
            None
        }
    }

    pub fn code(self) -> &'static str {
        match self {
            Self::En => "en",
            Self::Ko => "ko",
        }
    }

    pub fn english_name(self) -> &'static str {
        match self {
            Self::En => "English",
            Self::Ko => "Korean",
        }
    }

    pub fn native_name(self) -> &'static str {
        match self {
            Self::En => "English",
            Self::Ko => "한국어",
        }
    }
}

/// Representative Hangul characters used across the Korean UI catalog so `UiFonts::note` borrows a
/// Hangul font face on the first frame after switching to [`Locale::Ko`].
pub const HANGUL_UI_GLYPHS: &str = concat!(
    "한국어영어언어파일편집보기삽입서식정렬슬라이드쇼도움말홈그리기디자인전환애니메이션녹화검토",
    "새프레젠테이션열기닫기저장다른이름으로내보내기비디오인쇄속성실행취소다시잘라복사붙여넣기",
    "모두선택복제삭제찾기바꾸기기본여러장노트개요읽기마스터유인물눈금자안내선확대축소창맞춤새창",
    "구역텍스트상자장식날짜및시간번호표차트그림사진앨범오디오글꼴단락굵게기울임꼴밑줄왼쪽가운데",
    "오른쪽양쪽크기위치방향맨앞뒤로그룹해제재그룹회전대칭처음부터현재숨기기설정정보업데이트확인",
    "명령팔레트환경설정일반어두운테마측정단위인치센티미터편집중스마트격자에맞춤따옴표자동고침",
    "작성자이름댓글완료취소형식해상도프레임속도유지시간초숨겨진포함개체효과무한반복만들기변경",
    "다이어그램도형표디자인레이아웃재생발표검색옵션문서페이지용지가로세로투명배경사용자지정너비높이",
);

/// Pure catalog lookup: returns the localized label for `key` in `locale` (falling back to `key`
/// when untranslated).
pub fn tr(locale: Locale, key: &'static str) -> &'static str {
    match locale {
        Locale::En => key,
        Locale::Ko => ko_lookup(key).unwrap_or(key),
    }
}

fn fonts_cover(ctx: &egui::Context, text: &str) -> bool {
    let reg = egui::FontId::proportional(13.0);
    let bold = crate::theme::bold(13.0);
    ctx.fonts_mut(|f| f.has_glyphs(&reg, text) && f.has_glyphs(&bold, text))
}

/// Glyph-safe UI lookup: returns [`tr(locale, key)`] when the active `egui` font set covers every
/// character of the translated string, or `key` (ASCII English) when a Hangul font is not yet
/// active or unavailable on a headless host.
pub fn tr_ui(ctx: &egui::Context, locale: Locale, key: &'static str) -> &'static str {
    if locale == Locale::En {
        return key;
    }
    let translated = tr(locale, key);
    if translated.is_ascii() || fonts_cover(ctx, translated) {
        translated
    } else {
        key
    }
}

/// Display label for `locale` in a combo box or menu, falling back to ASCII when Hangul glyphs are
/// not available in `ctx`.
pub fn locale_display_label(ctx: &egui::Context, locale: Locale) -> &'static str {
    match locale {
        Locale::En => "English",
        Locale::Ko => {
            let rich = "한국어 (Korean)";
            if fonts_cover(ctx, rich) {
                rich
            } else {
                "Korean (ko)"
            }
        }
    }
}

fn ko_lookup(key: &str) -> Option<&'static str> {
    Some(match key {
        // Top-level menus & ribbon tabs
        "File" => "파일",
        "Edit" => "편집",
        "View" => "보기",
        "Insert" => "삽입",
        "Format" => "서식",
        "Arrange" => "정렬",
        "Slide Show" => "슬라이드 쇼",
        "Help" => "도움말",
        "Home" => "홈",
        "Draw" => "그리기",
        "Design" => "디자인",
        "Transitions" => "전환",
        "Animations" => "애니메이션",
        "Record" => "녹화",
        "Review" => "검토",
        "Diagram Design" => "다이어그램 디자인",
        "Shape Format" => "도형 서식",
        "Picture Format" => "그림 서식",
        "Table Design" => "표 디자인",
        "Layout" => "레이아웃",
        "Chart Design" => "차트 디자인",
        "Playback" => "재생",
        "Present" => "발표",
        "  Present  " => "  발표  ",
        "Search commands…" => "명령 검색…",

        // File menu & Backstage
        "New" => "새로 만들기",
        "New Presentation" => "새 프레젠테이션",
        "Open" => "열기",
        "Open…" => "열기…",
        "Info" => "정보",
        "Close" => "닫기",
        "Save" => "저장",
        "Save As" => "다른 이름으로 저장",
        "Save As…" => "다른 이름으로 저장…",
        "Export" => "내보내기",
        "Export…" => "내보내기…",
        "Export Animated Video" => "애니메이션 비디오 내보내기",
        "Export Animated Video…" => "애니메이션 비디오 내보내기…",
        "Export Animated Video / GIF" => "애니메이션 비디오 내보내기",
        "Export Animated Video / GIF…" => "애니메이션 비디오 내보내기…",
        "Create Animated Video / GIF" => "애니메이션 비디오 내보내기",
        "Create PDF Document" => "PDF 문서 만들기",
        "Create a PDF Document" => "PDF 문서 만들기",
        "Create PDF" => "PDF 만들기",
        "Create Handouts" => "유인물 만들기",
        "Change File Type" => "파일 형식 변경",
        "Print" => "인쇄",
        "Print…" => "인쇄…",
        "Properties" => "속성",
        "About" => "정보",
        "Options" => "옵션",

        // Edit menu
        "Undo" => "실행 취소",
        "Redo" => "다시 실행",
        "Cut" => "잘라내기",
        "Copy" => "복사",
        "Paste" => "붙여넣기",
        "Paste and Match Formatting" => "서식 유지 없이 붙여넣기",
        "Paste Special…" => "선택하여 붙여넣기…",
        "Select All" => "모두 선택",
        "Duplicate" => "복제",
        "Delete Slide" => "슬라이드 삭제",
        "Find…" => "찾기…",
        "Replace…" => "바꾸기…",
        "Find and Replace" => "찾기 및 바꾸기",

        // View menu & ribbon
        "Normal" => "기본",
        "Slide Sorter" => "여러 슬라이드",
        "Notes Page" => "슬라이드 노트",
        "Outline View" => "개요 보기",
        "Reading View" => "읽기용 보기",
        "Slide Master" => "슬라이드 마스터",
        "Handout Master" => "유인물 마스터",
        "Notes Master" => "슬라이드 노트 마스터",
        "Ruler" => "눈금자",
        "Gridlines" => "눈금선",
        "Guides" => "안내선",
        "Grid Options…" => "눈금 옵션…",
        "Grid Options" => "눈금 옵션",
        "Notes" => "슬라이드 노트",
        "Zoom" => "확대/축소",
        "Zoom…" => "확대/축소…",
        "Zoom In" => "확대",
        "Zoom Out" => "축소",
        "Fit to Window" => "창에 맞춤",
        "New Window" => "새 창",
        "Grayscale" => "회색조",
        "Dark Mode" => "어두운 모드",
        "Language" => "언어",
        "Language:" => "언어:",

        // Insert menu
        "New Slide" => "새 슬라이드",
        "Duplicate Slide" => "슬라이드 복제",
        "Section" => "구역",
        "Text Box" => "텍스트 상자",
        "Decorative Text" => "장식 텍스트",
        "Date and Time" => "날짜 및 시간",
        "Slide Number" => "슬라이드 번호",
        "Table…" => "표…",
        "Insert Table" => "표 삽입",
        "Chart" => "차트",
        "Picture from File…" => "파일에서 그림…",
        "Photo Album…" => "사진 앨범…",
        "Edit Photo Album…" => "사진 앨범 편집…",
        "Audio from File…" => "파일에서 오디오…",
        "Video from File…" => "파일에서 비디오…",

        // Format & Arrange menus
        "Font" => "글꼴",
        "Font…" => "글꼴…",
        "Paragraph" => "단락",
        "Paragraph…" => "단락…",
        "Bold" => "굵게",
        "Italic" => "기울임꼴",
        "Underline" => "밑줄",
        "Align Left" => "왼쪽 맞춤",
        "Center" => "가운데 맞춤",
        "Align Right" => "오른쪽 맞춤",
        "Justify" => "양쪽 맞춤",
        "Size and Position" => "크기 및 위치",
        "Size and Position…" => "크기 및 위치…",
        "Right-to-Left Text Direction" => "오른쪽에서 왼쪽 텍스트 방향",
        "Bring to Front" => "맨 앞으로 가져오기",
        "Send to Back" => "맨 뒤로 보내기",
        "Bring Forward" => "앞으로 가져오기",
        "Send Backward" => "뒤로 보내기",
        "Group" => "그룹",
        "Ungroup" => "그룹 해제",
        "Regroup" => "다시 그룹",
        "Rotate Left 90°" => "왼쪽으로 90° 회전",
        "Rotate Right 90°" => "오른쪽으로 90° 회전",
        "Flip Horizontal" => "좌우 대칭",
        "Flip Vertical" => "상하 대칭",

        // Slide Show & Help menus
        "Play from Start" => "처음부터 재생",
        "Play from Current Slide" => "현재 슬라이드부터 재생",
        "Hide Slide" => "슬라이드 숨기기",
        "Set Up Show…" => "슬라이드 쇼 설정…",
        "Set Up Slide Show" => "슬라이드 쇼 설정",
        "About DeckCraft" => "DeckCraft 정보",
        "Check for Updates…" => "업데이트 확인…",
        "Command Palette" => "명령 팔레트",

        // Preferences & Animated Video Export dialogs
        "Preferences" => "환경설정",
        "Preferences…" => "환경설정…",
        "General" => "일반",
        "Dark appearance" => "어두운 테마",
        "Measurement units:" => "측정 단위:",
        "Inches" => "인치",
        "Centimeters" => "센티미터",
        "Editing" => "편집",
        "Show smart guides" => "스마트 가이드 표시",
        "Snap objects to grid" => "개체를 격자에 맞춤",
        "Replace straight quotes with smart quotes" => "곧은 따옴표를 스마트 따옴표로 바꾸기",
        "AutoCorrect as you type" => "입력할 때 자동 고침",
        "AutoCorrect Options…" => "자동 고침 옵션…",
        "Your name (comments):" => "작성자 이름(댓글):",
        "Done" => "완료",
        "OK" => "확인",
        "Cancel" => "취소",
        "Format:" => "형식:",
        "Resolution:" => "해상도:",
        "Width (pixels):" => "너비(픽셀):",
        "Height (pixels):" => "높이(픽셀):",
        "Frame rate (FPS):" => "프레임 속도 (FPS):",
        "Slide hold (seconds):" => "슬라이드 유지 시간(초):",
        "From slide:" => "시작 슬라이드:",
        "To slide:" => "끝 슬라이드:",
        "Include hidden slides" => "숨겨진 슬라이드 포함",
        "Include speaker notes" => "발표자 노트 포함",
        "Include build animations" => "개체 애니메이션 포함",
        "Include slide transitions" => "슬라이드 전환 효과 포함",
        "Loop forever" => "무한 반복",
        "Transparent background" => "투명 배경",
        _ => return None,
    })
}
