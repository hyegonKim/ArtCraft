//! Script fallback fonts (Arabic and Korean) for the UI. egui's default fonts do not include
//! Arabic or Hangul glyphs, so translated UIs load fallback fonts from the operating system.

use std::sync::Arc;

use egui::{FontData, FontFamily};

/// Candidate Arabic system font files: (path, face index in a collection).
#[cfg(not(target_arch = "wasm32"))]
fn arabic_system_files() -> Vec<(std::path::PathBuf, u32)> {
    use std::path::PathBuf;
    let mut v: Vec<(PathBuf, u32)> = Vec::new();
    if cfg!(windows) {
        let fonts = PathBuf::from(std::env::var("WINDIR").unwrap_or_else(|_| "C:\\Windows".into())).join("Fonts");
        let files: &[&str] = &["segoeui.ttf", "tahoma.ttf", "arial.ttf"];
        v.extend(files.iter().map(|f| (fonts.join(f), 0)));
    } else if cfg!(target_os = "macos") {
        let files: &[&str] = &["/System/Library/Fonts/SFArabic.ttf", "/System/Library/Fonts/Supplemental/GeezaPro.ttc"];
        v.extend(files.iter().map(|f| (PathBuf::from(f), 0)));
        v.push((PathBuf::from("/System/Library/Fonts/Supplemental/Arial Unicode.ttf"), 0));
    } else {
        for f in [
            "/usr/share/fonts/truetype/noto/NotoSansArabic-Regular.ttf",
            "/usr/share/fonts/noto/NotoSansArabic-Regular.ttf",
            "/usr/share/fonts/google-noto/NotoSansArabic-Regular.ttf",
            "/usr/local/share/fonts/noto/NotoSansArabic-Regular.ttf",
            "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
            "/usr/share/fonts/dejavu/DejaVuSans.ttf",
            "/usr/local/share/fonts/dejavu/DejaVuSans.ttf",
        ] {
            v.push((PathBuf::from(f), 0));
        }
    }
    v
}

/// Candidate Korean Hangul system font files: (path, face index in a collection).
#[cfg(not(target_arch = "wasm32"))]
fn korean_system_files() -> Vec<(std::path::PathBuf, u32)> {
    use std::path::PathBuf;
    let mut v: Vec<(PathBuf, u32)> = Vec::new();
    if cfg!(windows) {
        let fonts = PathBuf::from(std::env::var("WINDIR").unwrap_or_else(|_| "C:\\Windows".into())).join("Fonts");
        v.push((fonts.join("malgun.ttf"), 0));
        v.push((fonts.join("gulim.ttc"), 0));
    } else if cfg!(target_os = "macos") {
        v.push((PathBuf::from("/System/Library/Fonts/AppleSDGothicNeo.ttc"), 0));
        v.push((PathBuf::from("/System/Library/Fonts/Supplemental/AppleGothic.ttf"), 0));
    } else {
        for (f, idx) in [
            ("/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc", 1u32),
            ("/usr/share/fonts/noto-cjk/NotoSansCJK-Regular.ttc", 1),
            ("/usr/share/fonts/truetype/nanum/NanumGothic.ttf", 0),
            ("/usr/share/fonts/nanum/NanumGothic.ttf", 0),
        ] {
            v.push((PathBuf::from(f), idx));
        }
    }
    v
}

#[cfg(not(target_arch = "wasm32"))]
fn load_first(candidates: Vec<(std::path::PathBuf, u32)>, label: &str) -> Option<Arc<FontData>> {
    for (path, index) in candidates {
        let Ok(meta) = std::fs::metadata(&path) else { continue };
        if !meta.is_file() || meta.len() == 0 || meta.len() > (128 << 20) {
            continue;
        }
        if let Ok(bytes) = std::fs::read(&path) {
            let mut data = FontData::from_owned(bytes);
            data.index = index;
            let arc = Arc::new(data);
            log::info!("UI font fallback for {label}: {}", path.display());
            return Some(arc);
        }
    }
    log::warn!("no font found for {label} text; system fonts may show boxes");
    None
}

/// Fallback fonts manager for the egui context.
#[derive(Default)]
pub struct UiFonts {
    arabic_installed: bool,
    korean_installed: bool,
    arabic_data: Option<Arc<FontData>>,
    korean_data: Option<Arc<FontData>>,
}

impl UiFonts {
    /// Update fallback fonts according to active language and whether the preferences dialog is open.
    pub fn update(&mut self, ctx: &egui::Context, language: &str, preferences_open: bool) {
        let want_arabic = language == "ar" || preferences_open;
        let want_korean = language == "ko" || preferences_open;

        let need_ar = want_arabic && !self.arabic_installed;
        let need_ko = want_korean && !self.korean_installed;
        if !need_ar && !need_ko {
            return;
        }

        if need_ar && self.arabic_data.is_none() {
            #[cfg(not(target_arch = "wasm32"))]
            {
                self.arabic_data = load_first(arabic_system_files(), "Arabic");
            }
            self.arabic_installed = true;
        }
        if need_ko && self.korean_data.is_none() {
            #[cfg(not(target_arch = "wasm32"))]
            {
                self.korean_data = load_first(korean_system_files(), "Korean");
            }
            self.korean_installed = true;
        }

        let mut defs = crate::theme::font_definitions();
        let mut any = false;
        if let Some(ar) = &self.arabic_data {
            let name = "script-Arab".to_string();
            defs.font_data.insert(name.clone(), ar.clone());
            defs.families.entry(FontFamily::Proportional).or_default().push(name.clone());
            defs.families.entry(FontFamily::Monospace).or_default().push(name);
            any = true;
        }
        if let Some(ko) = &self.korean_data {
            let name = "script-Hang".to_string();
            defs.font_data.insert(name.clone(), ko.clone());
            defs.families.entry(FontFamily::Proportional).or_default().push(name.clone());
            defs.families.entry(FontFamily::Monospace).or_default().push(name);
            any = true;
        }
        if any {
            ctx.set_fonts(defs);
        }
    }

    /// Whether an Arabic fallback font has been found and loaded.
    pub fn covers_arabic(&self) -> bool {
        self.arabic_data.is_some()
    }

    /// Whether a Korean fallback font has been found and loaded.
    pub fn covers_korean(&self) -> bool {
        self.korean_data.is_some()
    }
}
