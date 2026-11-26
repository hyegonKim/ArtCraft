//! Official Korean (`ko`) localization table for Adobe Photoshop 2027 (`28.0.0.20`) and
//! Adobe Camera Raw `19.0` (`19.0.0.2794`) features.
//!
//! Embedded directly so Korean builds and unit tests resolve every new Photoshop 2027 / ACR 19.0
//! menu item, command id, tool label, dialog control, and panel string even before `craft-i18n`
//! upstream syncs the 28.0 strings.

use std::collections::HashMap;
use std::sync::LazyLock;

/// Plain `(english, korean)` pairs. Every entry whose English source ends with `…` (`\u{2026}`)
/// also ends with `…` in Korean to satisfy `i18n::same_shape`.
pub const PLAIN_KO: &[(&str, &str)] = &[
    // Timelapse Recording & Export
    ("Timelapse", "타임랩스"),
    ("Export Timelapse…", "타임랩스 내보내기…"),
    ("Export Timelapse", "타임랩스 내보내기"),
    ("Start Timelapse", "타임랩스 시작"),
    ("Pause Timelapse", "타임랩스 일시 정지"),
    ("Resume Timelapse", "타임랩스 재개"),
    ("Clear Timelapse", "타임랩스 지우기"),
    ("Timelapse Options", "타임랩스 옵션"),
    ("Timelapse Auto-Start", "타임랩스 자동 시작"),
    ("Timelapse Resolution", "타임랩스 해상도"),
    ("Timelapse Quality", "타임랩스 품질"),
    ("Start on All Documents", "모든 문서에서 시작"),
    ("Start on New Documents", "새 문서에서 시작"),
    ("Start Manually", "수동으로 시작"),
    ("2560 (2K)", "2560 (2K)"),
    ("1920 (1080p)", "1920 (1080p)"),
    ("High", "높음"),
    ("Medium", "중간"),
    ("Low", "낮음"),
    ("Frames Captured", "캡처된 프레임"),
    ("Frame Rate (FPS)", "프레임 속도(FPS)"),
    ("Recording", "기록 중"),
    ("Paused", "일시 정지됨"),
    ("Stopped", "중지됨"),
    // NextGen Filters & Adjustments: Fireflies & Water Ripple
    ("NextGen Effects", "차세대 효과"),
    ("Fireflies", "반딧불이"),
    ("Fireflies…", "반딧불이…"),
    ("Quantity", "수량"),
    ("Depth Variation", "깊이 베리에이션"),
    ("Blurriness", "흐림"),
    ("Color", "컬러"),
    ("Progress", "진행률"),
    ("Water Ripple", "물결"),
    ("Water Ripple…", "물결…"),
    ("Horizon", "수평선"),
    ("Amplitude", "진폭"),
    ("Frequency", "주파수"),
    // Relight / Lightshop Layer (3D Depth-Normal Lights)
    ("New Relight Layer…", "새 재조명 레이어…"),
    ("Relight", "재조명"),
    ("Relight…", "재조명…"),
    ("Relight (3D)…", "재조명…"),
    ("Light 1", "조명 1"),
    ("Light 2", "조명 2"),
    ("Light 3", "조명 3"),
    ("Ambient", "주변광"),
    ("Surface Relief", "표면 릴리프"),
    ("Spread", "확산"),
    ("Z (Depth)", "Z (깊이)"),
    // Oil Brush Tool
    ("Oil Brush Tool", "유화 브러시 도구"),
    ("Oil Brush", "유화 브러시"),
    ("Blocky", "블록"),
    ("Glaze", "윤기"),
    ("Round", "원형"),
    ("Paint Mix", "페인트 혼합"),
    // Remove Ambient Effects
    ("Remove Ambient Effects…", "주변 효과 제거…"),
    ("Remove Ambient Effects", "주변 효과 제거"),
    ("Shadows", "그림자"),
    ("Reflections", "반사"),
    ("Lens flares", "렌즈 플레어"),
    ("Lens Flares", "렌즈 플레어"),
    ("Atmospheric", "분위기 있는"),
    ("Remove Related Elements", "관련 요소 제거"),
    ("Enhance", "향상"),
    // Type: Convert Raster to Text, Dynamic Fit, Copy/Paste Text Attributes
    ("Convert to Text Layers", "텍스트 레이어로 변환"),
    ("Dynamic Text Fit", "다이나믹 텍스트 맞춤"),
    ("Release Dynamic Fit", "다이나믹 맞춤 풀기"),
    ("Copy Text Attributes", "텍스트 속성 복사"),
    ("Paste Text Attributes", "텍스트 속성 붙여넣기"),
    // Generative Recompose & Restyle Layer
    ("Generative Recompose…", "생성형 재구성…"),
    ("Generative Recompose", "생성형 재구성"),
    ("Output Width", "출력 폭"),
    ("Output Height", "출력 높이"),
    ("Aspect Ratio", "종횡비"),
    ("Generative Restyle…", "생성형 스타일 변경…"),
    ("Generative Restyle", "생성형 스타일 변경"),
    ("New Restyle Layer…", "새 스타일 변경 레이어…"),
    ("Restyle Layer", "스타일 변경 레이어"),
    ("Capture as Layer", "레이어로 캡처"),
    ("Clear Live Effect", "라이브 효과 지우기"),
    ("Live Effect", "라이브 효과"),
    ("Warm Cinema", "따뜻한 시네마"),
    ("Watercolor", "수채화"),
    ("Cyber Neon", "사이버 네온"),
    ("Oil Impasto", "유화 임파스토"),
    ("Vintage Film", "빈티지 필름"),
    ("Noir Sketch", "누아르 스케치"),
    // Mockup Group & Environmental Lighting
    ("Create Mockup", "목업 만들기"),
    ("Mockup", "목업"),
    ("Environmental Lighting", "환경 조명"),
    ("Release Mockup", "목업 풀기"),
    // Agent Skills (8 One-Click Workflows)
    ("Agent Skills", "에이전트 스킬"),
    ("Cinematic Grade", "시네마틱 그레이드"),
    ("Enhance and Resize", "향상 및 크기 조정"),
    ("Perfect Portrait", "완벽한 인물 사진"),
    ("Product Studio Shot", "제품 스튜디오 샷"),
    ("Remove Background", "배경 제거"),
    ("Retouch Portrait", "인물 사진 리터치"),
    ("Social Media Hero", "소셜 미디어 히어로"),
    ("Subject Spotlight", "피사체 강조하기"),
    // Curves Channel Gradient Background & Grid Size
    ("Channel Gradient Background", "채널의 그레이디언트 배경"),
    ("Grid Size", "그리드 크기"),
    // Multithreaded PSD Write & Quick Compare
    ("Multithreaded PSD Saving", "멀티스레드 PSD 저장"),
    ("Quick Compare", "빠른 비교"),
    ("Quick Compare (Original)", "빠른 비교 (원본)"),
    // Camera Raw 19.0
    ("Distraction Removal", "방해 요소 제거"),
    ("Eyeglass Reflections", "안경 반사 제거"),
    ("Window Reflections", "창문 반사 제거"),
    ("Non-Raw Denoise", "비 Raw 노이즈 감소"),
    ("Adaptive Color", "적응형 컬러"),
    ("Adaptive Monochrome", "적응형 흑백"),
    ("Adobe Color", "Adobe 컬러"),
    ("Adaptive Amount", "적응형 양"),
    ("HDR Range", "HDR 범위"),
    ("HDR Mode", "HDR 모드"),
    // Substance 3D Studio (Sampler, Stager, Modeler, Designer, Painter)
    ("Substance 3D", "Substance 3D"),
    ("New 3D Stage Layer…", "새 3D 스테이지 레이어…"),
    ("New 3D Stage Layer", "새 3D 스테이지 레이어"),
    ("Material Sampler (Photo to PBR)…", "재질 샘플러(사진에서 PBR로)…"),
    ("Material Sampler (Photo to PBR)", "재질 샘플러(사진에서 PBR로)"),
    ("Photo to PBR…", "사진에서 PBR로…"),
    ("Procedural PBR Material…", "절차적 PBR 재질…"),
    ("Procedural PBR Material", "절차적 PBR 재질"),
    ("Procedural PBR…", "절차적 PBR…"),
    ("Make Seamless (Tileable)", "이음새 없는 타일 만들기"),
    ("Make Seamless", "이음새 없는 타일"),
    ("3D Shape", "3D 모양"),
    ("SDF Clay Boolean", "SDF 클레이 불리언"),
    ("Secondary Shape", "보조 모양"),
    ("Smooth Blend", "부드러운 혼합"),
    ("Material Preset", "재질 사전 설정"),
    ("Base Color", "기본 색상"),
    ("Metallic", "메탈릭"),
    ("Roughness", "거칠기"),
    ("Edge Wear", "모서리 마모"),
    ("Rotate X", "X 회전"),
    ("Rotate Y", "Y 회전"),
    ("Rotate Z", "Z 회전"),
    ("Object Scale", "오브젝트 비율"),
    ("Shadow Catcher", "그림자 캐처"),
    ("Match Background IBL", "배경 IBL 조명 일치"),
    ("Sphere", "구체"),
    ("Rounded Box", "둥근 상자"),
    ("Bottle", "병"),
    ("Beverage Can", "음료 캔"),
    ("Cylinder", "원기둥"),
    ("Torus", "토러스"),
    ("Display Card", "디스플레이 카드"),
    ("Custom OBJ Mesh", "사용자 정의 OBJ 메시"),
    ("Single Shape", "단일 모양"),
    ("Clay Smooth Union", "클레이 부드러운 결합"),
    ("Clay Smooth Subtract", "클레이 부드러운 차집합"),
    ("Clay Smooth Intersect", "클레이 부드러운 교집합"),
    ("Matte Plastic", "무광 플라스틱"),
    ("Brushed Metal", "브러시드 메탈"),
    ("Polished Gold", "광택 골드"),
    ("Frosted Glass", "불투명 유리"),
    ("Glazed Ceramic", "유약 세라믹"),
    ("Veined Marble", "대리석"),
    ("Weathered Copper", "풍화된 구리"),
    ("Wrap Active Layer as Decal", "활성 레이어를 데칼로 감싸기"),
    ("All 6 Maps as Layer Group", "6개 맵 모두 레이어 그룹으로 생성"),
    ("Base Color (Albedo)", "기본 색상(알베도)"),
    ("Normal (Tangent)", "노멀(탄젠트)"),
    ("Height (Displacement)", "높이(변위)"),
    ("Ambient Occlusion", "앰비언트 오클루전"),
    ("Delight Strength", "조명 제거 강도"),
    ("Normal Strength", "노멀 강도"),
    ("Roughness Base", "기본 거칠기"),
    ("Roughness Contrast", "거칠기 대비"),
    ("Metallic Level", "메탈릭 수준"),
    ("AO Strength", "AO 강도"),
    ("Tile Feather", "타일 페더"),
    ("Material Graph", "재질 그래프"),
    ("Pattern Scale", "패턴 크기"),
    ("Surface Wear", "표면 마모"),
    ("Light Angle", "조명 각도"),
    ("Light Elevation", "조명 고도"),
    ("Brushed Steel", "브러시드 스틸"),
    ("Carbon Hex Weave", "카본 헥사 직조"),
    ("Terrazzo Marble", "테라조 대리석"),
    ("Ceramic Tiles", "세라믹 타일"),
    ("Woven Fabric", "직물 패브릭"),
];

/// Command `@id` -> Korean menu/command label pairs.
pub const ID_KO: &[(&str, &str)] = &[
    ("window.toggle.timelapse", "타임랩스"),
    ("file.export.timelapse", "타임랩스 내보내기…"),
    ("timelapse.start", "타임랩스 시작"),
    ("timelapse.pause", "타임랩스 일시 정지"),
    ("timelapse.resume", "타임랩스 재개"),
    ("timelapse.clear", "타임랩스 지우기"),
    ("timelapse.configure", "타임랩스 옵션"),
    ("timelapse.setOptions", "타임랩스 옵션"),
    ("filter.nge.fireflies", "반딧불이…"),
    ("filter.nge.waterRipple", "물결…"),
    ("layer.newAdjustmentLayer.fireflies", "반딧불이…"),
    ("layer.newAdjustmentLayer.waterRipple", "물결…"),
    ("layer.newRelightLayer", "새 재조명 레이어…"),
    ("filter.render.relight3d", "재조명…"),
    ("tools.select.oilBrush", "유화 브러시 도구"),
    ("tools.setOilBrush", "유화 브러시 옵션"),
    ("paint.oilBrush", "유화 브러시"),
    ("edit.removeAmbientEffects", "주변 효과 제거…"),
    ("type.rasterToText", "텍스트 레이어로 변환"),
    ("type.setDynamicFit", "다이나믹 텍스트 맞춤"),
    ("type.releaseDynamicFit", "다이나믹 맞춤 풀기"),
    ("edit.copyTextAttributes", "텍스트 속성 복사"),
    ("edit.pasteTextAttributes", "텍스트 속성 붙여넣기"),
    ("type.copyAttributes", "텍스트 속성 복사"),
    ("type.pasteAttributes", "텍스트 속성 붙여넣기"),
    ("edit.generativeRecompose", "생성형 재구성…"),
    ("filter.stylize.restyle", "생성형 스타일 변경…"),
    ("layer.newRestyleLayer", "새 스타일 변경 레이어…"),
    ("layer.restyle.captureAsLayer", "레이어로 캡처"),
    ("layer.liveFx.setParams", "라이브 효과 설정"),
    ("layer.liveFx.clear", "라이브 효과 지우기"),
    ("layer.mockup.create", "목업 만들기"),
    ("layer.mockup.setLighting", "환경 조명"),
    ("layer.mockup.release", "목업 풀기"),
    ("agent.skill.cinematicGrade", "시네마틱 그레이드"),
    ("agent.skill.enhanceAndResize", "향상 및 크기 조정"),
    ("agent.skill.perfectPortrait", "완벽한 인물 사진"),
    ("agent.skill.productStudioShot", "제품 스튜디오 샷"),
    ("agent.skill.removeBackground", "배경 제거"),
    ("agent.skill.retouchPortrait", "인물 사진 리터치"),
    ("agent.skill.socialMediaHero", "소셜 미디어 히어로"),
    ("agent.skill.subjectSpotlight", "피사체 강조하기"),
    ("view.quickCompare", "빠른 비교"),
    ("layer.new3dStageLayer", "새 3D 스테이지 레이어…"),
    ("filter.material.samplePbr", "재질 샘플러(사진에서 PBR로)…"),
    ("filter.material.proceduralPbr", "절차적 PBR 재질…"),
    ("filter.material.makeSeamless", "이음새 없는 타일 만들기"),
];

static PLAIN_MAP: LazyLock<HashMap<&'static str, &'static str>> =
    LazyLock::new(|| PLAIN_KO.iter().copied().collect());

static ID_MAP: LazyLock<HashMap<&'static str, &'static str>> =
    LazyLock::new(|| ID_KO.iter().copied().collect());

/// Look up a plain English string in the Korean PS2027/ACR19 catalog.
pub fn plain(s: &str) -> Option<&'static str> {
    PLAIN_MAP.get(s).copied()
}

/// Whether the Korean PS2027/ACR19 catalog contains `s`.
pub fn has_plain(s: &str) -> bool {
    PLAIN_MAP.contains_key(s)
}

/// Look up a context-qualified or plain string in the Korean PS2027/ACR19 catalog.
pub fn ctx_or_plain(_context: &str, s: &str) -> Option<&'static str> {
    plain(s)
}

/// Look up a command id in the Korean PS2027/ACR19 catalog.
pub fn by_id(id: &str) -> Option<&'static str> {
    ID_MAP.get(id).copied()
}
