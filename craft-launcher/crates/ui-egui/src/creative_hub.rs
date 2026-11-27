//! Creative Cloud Desktop 4-Tab Workspace Hub (`Dialog::CreativeHub`):
//!
//! 1. **Files**: interactive file-path router + bounded project folder scanner (`scan_creative_files_in_dir`)
//!    with auto-detected Craft Suite target app badges and 1-click **Open**.
//! 2. **Fonts**: interactive Font Specimen & Family Inspector listing bundled variable faces and
//!    system-detected CJK/monospace families, category filter, live specimen preview (including
//!    Latin & Korean pangrams), and preview size slider.
//! 3. **Libraries**: `.cclibs` / `.ase` / `.aco` Creative Library Inspector with live visual color
//!    swatches plus a starter `.cclibs` / `.ase` handoff generator for PhotoCraft, VectorCraft,
//!    and DesignCraft.
//! 4. **Plugins**: Craft Suite Extension & MCP Integration overview (`.drfx`/`.ograf`, `.atn`,
//!    WASM, CLAP, UXP/MCP) with `craft-ai-mcp` config copy and 1-click app launch.

use std::path::Path;
#[cfg(not(target_arch = "wasm32"))]
use std::sync::OnceLock;

use artcraft_launcher_core::creative_hub::{
    BUNDLED_FONTS, FontCategoryFilter, HubTab, LIBRARY_TARGET_APPS, LibraryFormat, LibraryInspection, SAMPLE_KOREAN_PANGRAM, SAMPLE_LATIN_PANGRAM,
    STARTER_STUDIO_SWATCHES, SUITE_PLUGINS, SpecimenStyle, SwatchColor, build_starter_ase, build_starter_cclibs, inspect_library_bytes,
};
use artcraft_launcher_core::mcp_config::{McpConfigFormat, mcp_config};
use egui::{Color32, CornerRadius, FontFamily, FontId, RichText, Sense, Stroke, Ui, vec2};

use crate::app::{Host, LauncherUi};
use crate::i18n::{current, fmt, tr_ctx};
use crate::sheet;
use crate::theme::{self, Tokens};
use crate::widgets::{self, Cell, Kind};

/// Maximum creative project files returned by [`scan_creative_files_in_dir`].
pub const MAX_SCAN_FILES: usize = 50;
#[cfg(not(target_arch = "wasm32"))]
const MAX_DIR_ENTRIES_EXAMINED: usize = 2048;

/// One creative project file discovered by [`scan_creative_files_in_dir`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScannedCreativeFile {
    pub name: String,
    pub path: String,
    pub extension: String,
    pub size_bytes: u64,
    pub app_id: &'static str,
}

/// Scan `dir` (non-recursively, bounded) for creative project files supported by the Craft App
/// Suite, returning at most `limit` (clamped to [`MAX_SCAN_FILES`]) entries sorted by filename.
pub fn scan_creative_files_in_dir(dir: &Path, limit: usize) -> Result<Vec<ScannedCreativeFile>, String> {
    #[cfg(not(target_arch = "wasm32"))]
    {
        let cap = limit.clamp(1, MAX_SCAN_FILES);
        let read_dir = std::fs::read_dir(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        let mut out = Vec::new();
        for entry_res in read_dir.take(MAX_DIR_ENTRIES_EXAMINED) {
            let Ok(entry) = entry_res else { continue };
            let Ok(ft) = entry.file_type() else { continue };
            if !ft.is_file() {
                continue;
            }
            let path = entry.path();
            let Some(app) = artcraft_launcher_core::counterpart::route_file_path(&path) else { continue };
            let name = entry.file_name().to_string_lossy().into_owned();
            let ext = path.extension().and_then(|e| e.to_str()).map(|e| format!(".{}", e.to_ascii_uppercase())).unwrap_or_default();
            let size_bytes = entry.metadata().map(|m| m.len()).unwrap_or(0);
            out.push(ScannedCreativeFile { name, path: path.to_string_lossy().into_owned(), extension: ext, size_bytes, app_id: app.id });
        }
        out.sort_by(|a, b| a.name.to_ascii_lowercase().cmp(&b.name.to_ascii_lowercase()).then_with(|| a.name.cmp(&b.name)));
        out.truncate(cap);
        Ok(out)
    }
    #[cfg(target_arch = "wasm32")]
    {
        let _ = (dir, limit);
        Ok(Vec::new())
    }
}

/// Read and inspect a `.cclibs`, `.ase`, or `.aco` file from disk.
pub fn inspect_library_file(path: &Path) -> Result<LibraryInspection, String> {
    #[cfg(not(target_arch = "wasm32"))]
    {
        let meta = std::fs::metadata(path).map_err(|e| format!("{}: {e}", path.display()))?;
        if !meta.is_file() {
            return Err(format!("{}: not a regular file", path.display()));
        }
        if meta.len() > artcraft_launcher_core::creative_hub::MAX_LIBRARY_BYTES as u64 {
            return Err(format!("{}: exceeds 16 MB inspection limit", path.display()));
        }
        let bytes = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("library");
        inspect_library_bytes(name, &bytes)
    }
    #[cfg(target_arch = "wasm32")]
    {
        let _ = path;
        Err("Local file inspection is available in the desktop app.".into())
    }
}

/// A bundled or system-detected font family shown in the Fonts tab.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DetectedFontFamily {
    pub family: String,
    pub role: String,
    pub details: String,
    pub bundled: bool,
    pub is_variable: bool,
    pub is_cjk: bool,
    pub is_monospace: bool,
    pub style: SpecimenStyle,
}

/// Returns bundled variable font families followed by system-detected CJK and monospace families.
pub fn available_font_families() -> Vec<DetectedFontFamily> {
    let bundled = BUNDLED_FONTS.iter().map(|f| DetectedFontFamily {
        family: f.family.to_string(),
        role: f.role.to_string(),
        details: format!("{} · {}", f.axes, f.license),
        bundled: true,
        is_variable: f.is_variable,
        is_cjk: f.is_cjk,
        is_monospace: f.is_monospace,
        style: f.style,
    });

    #[cfg(not(target_arch = "wasm32"))]
    {
        static SYSTEM_DETECTED: OnceLock<Vec<DetectedFontFamily>> = OnceLock::new();
        let detected = SYSTEM_DETECTED.get_or_init(detect_system_fonts);
        bundled.chain(detected.iter().cloned()).collect()
    }
    #[cfg(target_arch = "wasm32")]
    {
        bundled.collect()
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn detect_system_fonts() -> Vec<DetectedFontFamily> {
    use std::path::PathBuf;

    struct Probe {
        family: &'static str,
        role: &'static str,
        is_variable: bool,
        is_cjk: bool,
        is_monospace: bool,
        style: SpecimenStyle,
        rel_paths: &'static [&'static str],
    }

    let mut roots: Vec<PathBuf> = Vec::new();
    #[cfg(target_os = "windows")]
    {
        let sys = std::env::var_os("SystemRoot").map_or_else(|| PathBuf::from(r"C:\Windows"), PathBuf::from);
        roots.push(sys.join("Fonts"));
        if let Some(local) = std::env::var_os("LOCALAPPDATA") {
            roots.push(PathBuf::from(local).join(r"Microsoft\Windows\Fonts"));
        }
    }
    #[cfg(target_os = "macos")]
    {
        roots.push(PathBuf::from("/System/Library/Fonts"));
        roots.push(PathBuf::from("/System/Library/Fonts/Supplemental"));
        roots.push(PathBuf::from("/Library/Fonts"));
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        for dir in ["/usr/share/fonts", "/usr/local/share/fonts"] {
            roots.push(PathBuf::from(dir));
        }
        if let Some(home) = std::env::var_os("HOME") {
            roots.push(PathBuf::from(home).join(".local/share/fonts"));
        }
    }

    const PROBES: &[Probe] = &[
        Probe {
            family: "Malgun Gothic",
            role: "Korean UI & Hangul typography",
            is_variable: false,
            is_cjk: true,
            is_monospace: false,
            style: SpecimenStyle::Proportional,
            rel_paths: &["malgun.ttf"],
        },
        Probe {
            family: "Microsoft YaHei",
            role: "Simplified Chinese UI & CJK typography",
            is_variable: false,
            is_cjk: true,
            is_monospace: false,
            style: SpecimenStyle::Proportional,
            rel_paths: &["msyh.ttc"],
        },
        Probe {
            family: "Microsoft JhengHei",
            role: "Traditional Chinese UI & CJK typography",
            is_variable: false,
            is_cjk: true,
            is_monospace: false,
            style: SpecimenStyle::Proportional,
            rel_paths: &["msjh.ttc"],
        },
        Probe {
            family: "Yu Gothic",
            role: "Japanese UI & Kana/Kanji typography",
            is_variable: false,
            is_cjk: true,
            is_monospace: false,
            style: SpecimenStyle::Proportional,
            rel_paths: &["YuGothR.ttc", "meiryo.ttc", "msgothic.ttc"],
        },
        Probe {
            family: "Apple SD Gothic Neo",
            role: "macOS Korean Hangul system family",
            is_variable: false,
            is_cjk: true,
            is_monospace: false,
            style: SpecimenStyle::Proportional,
            rel_paths: &["AppleSDGothicNeo.ttc"],
        },
        Probe {
            family: "PingFang SC",
            role: "macOS Chinese CJK system family",
            is_variable: false,
            is_cjk: true,
            is_monospace: false,
            style: SpecimenStyle::Proportional,
            rel_paths: &["PingFang.ttc"],
        },
        Probe {
            family: "Hiragino Sans",
            role: "macOS Japanese CJK system family",
            is_variable: false,
            is_cjk: true,
            is_monospace: false,
            style: SpecimenStyle::Proportional,
            rel_paths: &["ヒラギノ角ゴシック W3.ttc"],
        },
        Probe {
            family: "Noto Sans CJK",
            role: "Pan-CJK open-source typeface",
            is_variable: true,
            is_cjk: true,
            is_monospace: false,
            style: SpecimenStyle::Proportional,
            rel_paths: &[
                "NotoSansCJK-Regular.ttc",
                "NotoSansCJK-VF.ttc",
                "opentype/noto/NotoSansCJK-Regular.ttc",
                "noto-cjk/NotoSansCJK-Regular.ttc",
                "google-noto-cjk/NotoSansCJK-Regular.ttc",
                "NotoSansCJKtc-Regular.otf",
                "NotoSansCJKkr-Regular.otf",
            ],
        },
        Probe {
            family: "Segoe UI Variable",
            role: "Windows optical variable system UI",
            is_variable: true,
            is_cjk: false,
            is_monospace: false,
            style: SpecimenStyle::Proportional,
            rel_paths: &["SegUIVar.ttf"],
        },
        Probe {
            family: "Cascadia Mono",
            role: "Variable developer monospace",
            is_variable: true,
            is_cjk: false,
            is_monospace: true,
            style: SpecimenStyle::Monospace,
            rel_paths: &["CascadiaMono.ttf", "CascadiaCode.ttf"],
        },
        Probe {
            family: "Consolas",
            role: "System fixed-width code typeface",
            is_variable: false,
            is_cjk: false,
            is_monospace: true,
            style: SpecimenStyle::Monospace,
            rel_paths: &["consola.ttf"],
        },
        Probe {
            family: "SF Mono / Menlo",
            role: "macOS fixed-width developer typeface",
            is_variable: false,
            is_cjk: false,
            is_monospace: true,
            style: SpecimenStyle::Monospace,
            rel_paths: &["SFNSMono.ttf", "Menlo.ttc"],
        },
        Probe {
            family: "DejaVu Sans Mono",
            role: "Unix fixed-width monospace typeface",
            is_variable: false,
            is_cjk: false,
            is_monospace: true,
            style: SpecimenStyle::Monospace,
            rel_paths: &["truetype/dejavu/DejaVuSansMono.ttf", "TTF/DejaVuSansMono.ttf", "DejaVuSansMono.ttf"],
        },
    ];

    let mut out = Vec::new();
    for probe in PROBES {
        let found = roots.iter().find_map(|root| probe.rel_paths.iter().map(|rel| root.join(rel)).find(|p| p.is_file()));
        if let Some(path) = found {
            let file_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            out.push(DetectedFontFamily {
                family: probe.family.to_string(),
                role: probe.role.to_string(),
                details: file_name.to_string(),
                bundled: false,
                is_variable: probe.is_variable,
                is_cjk: probe.is_cjk,
                is_monospace: probe.is_monospace,
                style: probe.style,
            });
        }
    }
    out
}

/// Interactive state for `Dialog::CreativeHub`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CreativeHubState {
    pub file_path: String,
    pub scan_folder: String,
    pub scanned_files: Option<Result<Vec<ScannedCreativeFile>, String>>,
    pub font_filter: FontCategoryFilter,
    pub specimen: String,
    pub preview_size: u32,
    pub library_path: String,
    pub inspected_library: Option<Result<LibraryInspection, String>>,
    pub starter_name: String,
    pub starter_use_ase: bool,
}

impl Default for CreativeHubState {
    fn default() -> Self {
        Self {
            file_path: String::new(),
            scan_folder: String::new(),
            scanned_files: None,
            font_filter: FontCategoryFilter::All,
            specimen: SAMPLE_LATIN_PANGRAM.to_string(),
            preview_size: 22,
            library_path: String::new(),
            inspected_library: None,
            starter_name: "ArtCraft Studio Swatches".to_string(),
            starter_use_ase: false,
        }
    }
}

fn hub_tab_label(tab: HubTab) -> &'static str {
    match tab {
        HubTab::Files => tl!("Files"),
        HubTab::Fonts => tl!("Fonts"),
        HubTab::Libraries => tl!("Libraries"),
        HubTab::Plugins => tl!("Plugins"),
    }
}

fn font_filter_label(filter: FontCategoryFilter) -> &'static str {
    match filter {
        FontCategoryFilter::All => tl!("All Fonts"),
        FontCategoryFilter::Variable => tl!("Variable"),
        FontCategoryFilter::Cjk => tl!("CJK & World"),
        FontCategoryFilter::Monospace => tl!("Monospace"),
    }
}

/// Replaces any codepoint absent from `font_id`'s active glyph stack with `'·'` so specimen text
/// never triggers missing-glyph tofu or `egui_kittest`'s strict missing-glyph panic.
fn renderable_specimen(ctx: &egui::Context, font_id: &FontId, text: &str) -> String {
    ctx.fonts_mut(|fonts| {
        if fonts.has_glyphs(font_id, text) {
            return text.to_string();
        }
        text.chars()
            .map(|ch| {
                if ch.is_whitespace() {
                    return ch;
                }
                let mut buf = [0u8; 4];
                let s = ch.encode_utf8(&mut buf);
                if fonts.has_glyphs(font_id, s) { ch } else { '·' }
            })
            .collect()
    })
}

fn sanitize_stem(raw: &str) -> String {
    let cleaned: String =
        raw.trim().chars().map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' { c.to_ascii_lowercase() } else { '-' }).collect();
    let trimmed = cleaned.trim_matches('-');
    if trimmed.is_empty() { "artcraft-swatches".to_string() } else { trimmed.chars().take(48).collect() }
}

impl LauncherUi {
    /// Draws the 4-tab Creative Cloud Workspace Hub modal (`Files · Fonts · Libraries · Plugins`).
    pub(crate) fn creative_hub_dialog(&mut self, ui: &mut Ui, tab: &mut HubTab, state: &mut CreativeHubState, host: &mut dyn Host) -> bool {
        let t = Tokens::get(ui.ctx());
        let size = sheet::size(ui.ctx(), vec2(1020.0, 820.0));
        ui.set_width(size.x);
        ui.set_max_height(size.y);

        let hero_h =
            sheet::hero(ui, Some(crate::icons::artcraft_mark()), tl!("Creative Workspace Hub"), tl!("Files · Fonts · Libraries · Plugins"), |_| {});
        let footer_h = theme::CONTROL_H + 32.0;
        let body_h = (size.y - hero_h - footer_h - 2.0).max(0.0);
        let wide_rail = size.x >= theme::DIALOG_COLUMNS_FROM;

        let mut close_dialog = false;

        if wide_rail {
            let rail_w = 184.0;
            ui.allocate_ui_with_layout(vec2(size.x, body_h), egui::Layout::left_to_right(egui::Align::Min), |ui| {
                ui.spacing_mut().item_spacing.x = 0.0;
                // Left rail matching Creative Cloud Desktop (`Files · Fonts · Libraries · Plugins`)
                egui::Frame::new().fill(t.sunken).inner_margin(egui::Margin::symmetric(12, 16)).show(ui, |ui| {
                    ui.set_width(rail_w - 24.0);
                    ui.set_min_height((body_h - 32.0).max(0.0));
                    ui.vertical(|ui| {
                        ui.spacing_mut().item_spacing.y = 4.0;
                        for candidate in HubTab::ALL {
                            let selected = *tab == candidate;
                            let label = hub_tab_label(candidate);
                            let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), theme::CONTROL_H), Sense::click());
                            let (fill, ink) = match (selected, resp.hovered()) {
                                (true, _) => (t.invert_bg, t.invert_fg),
                                (false, true) => (t.bg, t.ink_strong),
                                (false, false) => (Color32::TRANSPARENT, t.muted),
                            };
                            ui.painter().rect_filled(rect, CornerRadius::ZERO, fill);
                            if !selected && resp.hovered() {
                                ui.painter().rect_stroke(rect, CornerRadius::ZERO, t.hairline(), egui::StrokeKind::Inside);
                            }
                            let galley = widgets::caps(ui, label, theme::HUD_SIZE, theme::mono_bold(), theme::HUD_SPACING, ink);
                            let text_pos = egui::pos2(rect.left() + 12.0, rect.center().y - galley.size().y / 2.0);
                            ui.painter().galley_with_override_text_color(text_pos, galley, ink);
                            resp.widget_info(|| egui::WidgetInfo::selected(egui::Role::Button, true, selected, label));
                            if resp.on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
                                *tab = candidate;
                            }
                        }
                    });
                });
                let rail_x = ui.cursor().left();
                ui.painter().vline(rail_x, ui.max_rect().y_range(), t.hairline());

                let content_w = (size.x - rail_w - 1.0).max(100.0);
                ui.allocate_ui_with_layout(vec2(content_w, body_h), egui::Layout::top_down(egui::Align::Min), |ui| {
                    let saved = sheet::accent_scroll(ui, &t, t.accent);
                    egui::ScrollArea::vertical().id_salt(("hub-body", *tab as u8)).max_height(body_h).auto_shrink([false, false]).show(ui, |ui| {
                        *ui.visuals_mut() = saved;
                        egui::Frame::new().inner_margin(egui::Margin::same(theme::PAD as i8)).show(ui, |ui| {
                            ui.set_width(ui.available_width());
                            close_dialog |= self.hub_tab_body(ui, *tab, state, host);
                        });
                    });
                });
            });
        } else {
            egui::Frame::new().inner_margin(egui::Margin::symmetric(theme::PAD as i8, 12)).show(ui, |ui| {
                let opts = HubTab::ALL.map(|c| {
                    let l = hub_tab_label(c);
                    (c, Cell { label: l, tip: l, count: None })
                });
                widgets::cells(ui, tab, &opts);
            });
            let inner_h = (body_h - (theme::CONTROL_H + 24.0)).max(0.0);
            let saved = sheet::accent_scroll(ui, &t, t.accent);
            egui::ScrollArea::vertical().id_salt(("hub-body-narrow", *tab as u8)).max_height(inner_h).auto_shrink([false, true]).show(ui, |ui| {
                *ui.visuals_mut() = saved;
                egui::Frame::new().inner_margin(egui::Margin::same(theme::PAD as i8)).show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    close_dialog |= self.hub_tab_body(ui, *tab, state, host);
                });
            });
        }

        let rule_y = ui.cursor().top();
        ui.painter().hline(ui.max_rect().x_range(), rule_y, t.hairline());
        egui::Frame::new().inner_margin(egui::Margin::symmetric(theme::PAD as i8, 16)).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                close_dialog |= widgets::button(ui, tl!("Done"), Kind::Primary, 90.0).clicked();
            });
        });
        close_dialog
    }

    fn hub_tab_body(&mut self, ui: &mut Ui, tab: HubTab, state: &mut CreativeHubState, host: &mut dyn Host) -> bool {
        match tab {
            HubTab::Files => self.hub_files_tab(ui, state, host),
            HubTab::Fonts => {
                self.hub_fonts_tab(ui, state);
                false
            }
            HubTab::Libraries => self.hub_libraries_tab(ui, state, host),
            HubTab::Plugins => self.hub_plugins_tab(ui),
        }
    }

    /// Tab 1 (`Files`): interactive file router + bounded directory scanner with 1-click Open.
    fn hub_files_tab(&mut self, ui: &mut Ui, state: &mut CreativeHubState, host: &mut dyn Host) -> bool {
        let t = Tokens::get(ui.ctx());
        let mut close_dialog = false;

        sheet::panel(ui, tl!("Open File in Craft App"), |ui| {
            let label = ui.label(tl!("File path or extension"));
            ui.horizontal(|ui| {
                let browse_w = widgets::button_width(ui, tl!("Browse…"), 96.0);
                let field_w = (ui.available_width() - browse_w - ui.spacing().item_spacing.x).max(120.0);
                ui.add(
                    egui::TextEdit::singleline(&mut state.file_path)
                        .id_salt("hub_file_router_path")
                        .hint_text(".psd, .ai, .prproj, .indd, .pdf, .glb, .cclibs…")
                        .desired_width(field_w),
                )
                .labelled_by(label.id);
                if widgets::button(ui, tl!("Browse…"), Kind::Secondary, browse_w).clicked()
                    && let Some(selected) = host.pick_file((!state.file_path.is_empty()).then_some(state.file_path.as_str()))
                {
                    state.file_path = selected;
                }
            });
            ui.add_space(8.0);
            let trimmed = state.file_path.trim().trim_matches(['"', '\'']).to_string();
            let routed = (!trimmed.is_empty()).then(|| artcraft_launcher_core::counterpart::route_file_path(Path::new(&trimmed))).flatten();
            match (trimmed.is_empty(), routed) {
                (true, _) => {
                    ui.label(RichText::new(tl!("Enter a file path or extension to open it in its matching Craft app.")).color(t.muted));
                }
                (false, Some(app)) => {
                    egui::Frame::new().fill(t.sunken).stroke(t.hairline()).inner_margin(egui::Margin::symmetric(12, 10)).show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        ui.horizontal(|ui| {
                            if let Some(icon) = crate::icons::app(app.id) {
                                ui.add(egui::Image::new(icon).fit_to_exact_size(vec2(32.0, 32.0)));
                            }
                            ui.add_space(6.0);
                            ui.vertical(|ui| {
                                ui.spacing_mut().item_spacing.y = 2.0;
                                widgets::wordmark(ui, app, 16.0);
                                let tagline = tr_ctx(current(), app.id, app.tagline);
                                ui.label(RichText::new(tagline).size(12.5).color(t.muted));
                            });
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                if self.caps.manage_apps && self.is_installed(app.id) {
                                    let btn = fmt(tl!("Open in {name}"), &[("name", app.name)]);
                                    if widgets::button(ui, &btn, Kind::Primary, 0.0).clicked() {
                                        self.launch_app_with_file(app.id, &trimmed);
                                        close_dialog = true;
                                    }
                                } else if self.caps.manage_apps {
                                    let btn = fmt(tl!("Install {name}"), &[("name", app.name)]);
                                    if widgets::button(ui, &btn, Kind::Primary, 0.0).clicked() {
                                        close_dialog = true;
                                        self.install_app(app.id);
                                    }
                                } else if widgets::button(ui, tl!("App details"), Kind::Primary, 0.0).clicked() {
                                    self.detail = Some(app.id.to_string());
                                    close_dialog = true;
                                }
                            });
                        });
                    });
                }
                (false, None) => {
                    ui.label(RichText::new(tl!("No matching Craft app for this file format.")).color(t.bad));
                }
            }
        });

        sheet::panel(ui, tl!("Project folder scanner"), |ui| {
            let folder_label = ui.label(tl!("Folder to scan"));
            let mut run_scan = false;
            ui.horizontal(|ui| {
                let pick_w = widgets::button_width(ui, tl!("Choose folder…"), 120.0);
                let scan_w = widgets::button_width(ui, tl!("Scan folder"), 110.0);
                let field_w = (ui.available_width() - pick_w - scan_w - 2.0 * ui.spacing().item_spacing.x).max(120.0);
                let edit = ui
                    .add(egui::TextEdit::singleline(&mut state.scan_folder).id_salt("hub_scan_folder").desired_width(field_w))
                    .labelled_by(folder_label.id);
                if edit.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                    run_scan = true;
                }
                if widgets::button(ui, tl!("Choose folder…"), Kind::Secondary, pick_w).clicked()
                    && let Some(chosen) = host.pick_folder((!state.scan_folder.is_empty()).then_some(state.scan_folder.as_str()))
                {
                    state.scan_folder = chosen;
                    run_scan = true;
                }
                if widgets::button(ui, tl!("Scan folder"), Kind::Primary, scan_w).clicked() {
                    run_scan = true;
                }
            });

            if run_scan {
                let clean = state.scan_folder.trim().trim_matches(['"', '\'']);
                if !clean.is_empty() {
                    state.scanned_files = Some(scan_creative_files_in_dir(Path::new(clean), MAX_SCAN_FILES));
                }
            }

            ui.add_space(10.0);
            match &state.scanned_files {
                None => {
                    ui.label(
                        RichText::new(tl!("Scan a folder to list up to 50 creative project files and open them in their Craft app.")).color(t.muted),
                    );
                }
                Some(Err(err)) => {
                    ui.label(RichText::new(err).color(t.bad));
                }
                Some(Ok(files)) if files.is_empty() => {
                    ui.label(RichText::new(tl!("No creative project files found in this folder.")).color(t.muted));
                }
                Some(Ok(files)) => {
                    let count_str = files.len().to_string();
                    widgets::hud(ui, &fmt(tl!("Found {count} creative files"), &[("count", &count_str)]), t.accent_ink);
                    ui.add_space(8.0);
                    let snapshot = files.clone();
                    for item in snapshot {
                        let Some(app) = artcraft_launcher_core::CraftApp::by_id(item.app_id) else { continue };
                        egui::Frame::new().fill(t.sunken).stroke(t.hairline()).inner_margin(egui::Margin::symmetric(12, 8)).show(ui, |ui| {
                            ui.set_width(ui.available_width());
                            ui.horizontal(|ui| {
                                if let Some(icon) = crate::icons::app(app.id) {
                                    ui.add(egui::Image::new(icon).fit_to_exact_size(vec2(24.0, 24.0)));
                                }
                                ui.vertical(|ui| {
                                    ui.spacing_mut().item_spacing.y = 2.0;
                                    ui.label(RichText::new(&item.name).family(theme::medium()).color(t.ink_strong));
                                    ui.label(RichText::new(widgets::human_bytes(item.size_bytes)).size(11.5).color(t.faint));
                                });
                                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                    if widgets::button(ui, tl!("Open"), Kind::Primary, 80.0).on_hover_text(&item.path).clicked() {
                                        let _ = self.route_and_open_file(&item.path);
                                        close_dialog = true;
                                    }
                                    widgets::badge(ui, app.name, t.app_ink(app));
                                    if !item.extension.is_empty() {
                                        widgets::badge(ui, &item.extension, t.muted);
                                    }
                                });
                            });
                        });
                        ui.add_space(6.0);
                    }
                }
            }
        });

        close_dialog
    }

    /// Tab 2 (`Fonts`): interactive Font Specimen & Family Inspector.
    fn hub_fonts_tab(&mut self, ui: &mut Ui, state: &mut CreativeHubState) {
        let t = Tokens::get(ui.ctx());

        sheet::panel(ui, tl!("Font specimen preview"), |ui| {
            let filter_opts = FontCategoryFilter::ALL.map(|f| {
                let l = font_filter_label(f);
                (f, Cell { label: l, tip: l, count: None })
            });
            widgets::cells(ui, &mut state.font_filter, &filter_opts);
            ui.add_space(10.0);

            let spec_label = ui.label(tl!("Specimen text"));
            // Guard the TextEdit display when Hangul hasn't activated yet: if the specimen contains
            // Korean Hangul and no CJK font is active in `ctx`, request the Korean system font so
            // the next frame renders real Hangul glyphs.
            let has_hangul = state.specimen.chars().any(|c| ('\u{ac00}'..='\u{d7a3}').contains(&c));
            if has_hangul && self.cjk_font.is_none() && crate::theme::cjk_font_available(crate::i18n::Lang::KO) {
                crate::theme::install_fonts(ui.ctx(), Some(crate::i18n::Lang::KO));
                self.cjk_font = Some(crate::i18n::Lang::KO);
                ui.ctx().request_repaint();
            }

            let body_font = FontId::new(14.0, FontFamily::Proportional);
            let input_covered = ui.ctx().fonts_mut(|f| f.has_glyphs(&body_font, &state.specimen));
            if input_covered {
                ui.add(egui::TextEdit::singleline(&mut state.specimen).id_salt("hub_font_specimen").desired_width(f32::INFINITY))
                    .labelled_by(spec_label.id);
            } else {
                let mut safe_copy = renderable_specimen(ui.ctx(), &body_font, &state.specimen);
                if ui
                    .add(egui::TextEdit::singleline(&mut safe_copy).id_salt("hub_font_specimen_safe").desired_width(f32::INFINITY))
                    .labelled_by(spec_label.id)
                    .changed()
                {
                    state.specimen = safe_copy;
                }
            }

            ui.add_space(6.0);
            ui.horizontal_wrapped(|ui| {
                if widgets::button(ui, tl!("Latin pangram"), Kind::Secondary, 0.0).clicked() {
                    state.specimen = SAMPLE_LATIN_PANGRAM.to_string();
                }
                if widgets::button(ui, tl!("Korean pangram"), Kind::Secondary, 0.0).clicked() {
                    state.specimen = SAMPLE_KOREAN_PANGRAM.to_string();
                    if self.cjk_font.is_none() && crate::theme::cjk_font_available(crate::i18n::Lang::KO) {
                        crate::theme::install_fonts(ui.ctx(), Some(crate::i18n::Lang::KO));
                        self.cjk_font = Some(crate::i18n::Lang::KO);
                        ui.ctx().request_repaint();
                    }
                }
                ui.add_space(12.0);
                let size_label = ui.label(tl!("Preview size"));
                let mut px = state.preview_size.clamp(12, 48) as f32;
                if ui.add(egui::Slider::new(&mut px, 12.0..=48.0).suffix(" px")).labelled_by(size_label.id).changed() {
                    state.preview_size = px.round() as u32;
                }
            });
        });

        sheet::panel(ui, tl!("Font families"), |ui| {
            let families = available_font_families();
            let preview_text = if state.specimen.trim().is_empty() { SAMPLE_LATIN_PANGRAM } else { state.specimen.as_str() };
            let px = state.preview_size.clamp(12, 48) as f32;

            for entry in families.into_iter().filter(|f| state.font_filter.matches(f.is_variable, f.is_cjk, f.is_monospace)) {
                egui::Frame::new().fill(t.sunken).stroke(t.hairline()).inner_margin(egui::Margin::symmetric(14, 12)).show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.horizontal_wrapped(|ui| {
                        ui.label(RichText::new(&entry.family).size(15.0).family(theme::semibold()).color(t.ink_strong));
                        widgets::badge(
                            ui,
                            if entry.bundled { tl!("Bundled") } else { tl!("System") },
                            if entry.bundled { t.accent_ink } else { t.muted },
                        );
                        if entry.is_variable {
                            widgets::badge(ui, tl!("Variable"), t.good);
                        }
                        if entry.is_cjk {
                            widgets::badge(ui, tl!("CJK & World"), t.warn);
                        }
                        if entry.is_monospace {
                            widgets::badge(ui, tl!("Monospace"), t.muted);
                        }
                        ui.label(RichText::new(format!("{} · {}", entry.role, entry.details)).size(12.0).color(t.faint));
                    });
                    ui.add_space(8.0);
                    let family = match entry.style {
                        SpecimenStyle::Proportional => FontFamily::Proportional,
                        SpecimenStyle::Display => theme::display(),
                        SpecimenStyle::Monospace => FontFamily::Monospace,
                    };
                    let font_id = FontId::new(px, family);
                    let shown = renderable_specimen(ui.ctx(), &font_id, preview_text);
                    ui.add(egui::Label::new(RichText::new(shown).font(font_id).color(t.ink)).wrap());
                });
                ui.add_space(8.0);
            }
        });
    }

    /// Tab 3 (`Libraries`): `.cclibs` / `.ase` / `.aco` Creative Library Inspector & Starter Handoff.
    fn hub_libraries_tab(&mut self, ui: &mut Ui, state: &mut CreativeHubState, host: &mut dyn Host) -> bool {
        let t = Tokens::get(ui.ctx());
        let mut close_dialog = false;

        sheet::panel(ui, tl!("Creative Library inspector"), |ui| {
            let lib_label = ui.label(tl!("Library or swatch file (.cclibs, .ase, .aco)"));
            let mut do_inspect = false;
            ui.horizontal(|ui| {
                let browse_w = widgets::button_width(ui, tl!("Browse…"), 96.0);
                let inspect_w = widgets::button_width(ui, tl!("Inspect library"), 120.0);
                let field_w = (ui.available_width() - browse_w - inspect_w - 2.0 * ui.spacing().item_spacing.x).max(120.0);
                let edit = ui
                    .add(
                        egui::TextEdit::singleline(&mut state.library_path)
                            .id_salt("hub_library_path")
                            .hint_text("palette.ase, swatches.aco, brand.cclibs")
                            .desired_width(field_w),
                    )
                    .labelled_by(lib_label.id);
                if edit.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                    do_inspect = true;
                }
                if widgets::button(ui, tl!("Browse…"), Kind::Secondary, browse_w).clicked()
                    && let Some(selected) = host.pick_file((!state.library_path.is_empty()).then_some(state.library_path.as_str()))
                {
                    state.library_path = selected;
                    do_inspect = true;
                }
                if widgets::button(ui, tl!("Inspect library"), Kind::Primary, inspect_w).clicked() {
                    do_inspect = true;
                }
            });

            if do_inspect {
                let clean = state.library_path.trim().trim_matches(['"', '\'']);
                if !clean.is_empty() {
                    state.inspected_library = Some(inspect_library_file(Path::new(clean)));
                }
            }

            if let Some(result) = &state.inspected_library {
                ui.add_space(10.0);
                match result {
                    Err(err) => {
                        ui.label(RichText::new(err).color(t.bad));
                    }
                    Ok(info) => {
                        close_dialog |= self.draw_library_inspection(ui, info, &state.library_path);
                    }
                }
            }
        });

        sheet::panel(ui, tl!("Starter library handoff"), |ui| {
            ui.horizontal_wrapped(|ui| {
                let format_opts =
                    [(false, Cell { label: ".cclibs", tip: ".cclibs", count: None }), (true, Cell { label: ".ase", tip: ".ase", count: None })];
                widgets::cells(ui, &mut state.starter_use_ase, &format_opts);
                ui.add_space(8.0);
                let name_label = ui.label(tl!("Library name"));
                ui.add(egui::TextEdit::singleline(&mut state.starter_name).id_salt("hub_starter_name").desired_width(240.0))
                    .labelled_by(name_label.id);
            });
            ui.add_space(10.0);

            // Live swatch preview strip for the starter studio palette
            let starter_swatches: Vec<SwatchColor> =
                STARTER_STUDIO_SWATCHES.iter().map(|(n, rgb)| SwatchColor { name: (*n).to_string(), rgb: *rgb, model: "RGB" }).collect();
            draw_swatch_grid(ui, &starter_swatches, &t);
            ui.add_space(10.0);

            ui.horizontal_wrapped(|ui| {
                if widgets::button(ui, tl!("Preview starter swatches"), Kind::Secondary, 0.0).clicked() {
                    let (fname, payload) = build_starter_payload(&state.starter_name, state.starter_use_ase);
                    state.inspected_library = Some(inspect_library_bytes(&fname, &payload));
                }
                for app_id in LIBRARY_TARGET_APPS {
                    let Some(app) = artcraft_launcher_core::CraftApp::by_id(app_id) else { continue };
                    let label = fmt(tl!("Create & Open in {name}"), &[("name", app.name)]);
                    if widgets::button(ui, &label, Kind::Primary, 0.0).clicked() {
                        close_dialog |= self.handoff_starter_library(state, app);
                    }
                }
            });
        });

        close_dialog
    }

    fn draw_library_inspection(&mut self, ui: &mut Ui, info: &LibraryInspection, current_path: &str) -> bool {
        let t = Tokens::get(ui.ctx());
        let mut close_dialog = false;
        egui::Frame::new().fill(t.sunken).stroke(t.hairline()).inner_margin(egui::Margin::symmetric(14, 12)).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal_wrapped(|ui| {
                widgets::tag(ui, info.format.badge(), t.accent, Color32::WHITE);
                ui.label(RichText::new(&info.library_name).size(15.0).family(theme::semibold()).color(t.ink_strong));
                widgets::badge(ui, &info.version_label, t.muted);
                let count_label = fmt(tl!("{count} swatches"), &[("count", &info.swatches.len().to_string())]);
                widgets::badge(ui, &count_label, t.good);
            });
            if !info.swatches.is_empty() {
                ui.add_space(10.0);
                draw_swatch_grid(ui, &info.swatches, &t);
            }
            let trimmed_path = current_path.trim().trim_matches(['"', '\'']);
            if !trimmed_path.is_empty() {
                ui.add_space(10.0);
                ui.horizontal_wrapped(|ui| {
                    for &app_id in info.target_apps {
                        let Some(app) = artcraft_launcher_core::CraftApp::by_id(app_id) else { continue };
                        let btn = fmt(tl!("Open in {name}"), &[("name", app.name)]);
                        if widgets::button(ui, &btn, Kind::Secondary, 0.0).clicked() {
                            if self.caps.manage_apps && self.is_installed(app.id) {
                                self.launch_app_with_file(app.id, trimmed_path);
                            } else if self.caps.manage_apps {
                                self.install_app(app.id);
                            } else {
                                self.detail = Some(app.id.to_string());
                            }
                            close_dialog = true;
                        }
                    }
                });
            }
        });
        close_dialog
    }

    fn handoff_starter_library(&mut self, state: &mut CreativeHubState, app: &'static artcraft_launcher_core::CraftApp) -> bool {
        let (fname, payload) = build_starter_payload(&state.starter_name, state.starter_use_ase);
        state.inspected_library = Some(inspect_library_bytes(&fname, &payload));
        #[cfg(not(target_arch = "wasm32"))]
        {
            let dir = std::env::temp_dir().join("artcraft-creative-hub");
            if let Err(e) = std::fs::create_dir_all(&dir) {
                self.toast_from_ui(format!("{}: {e}", dir.display()), true);
                return false;
            }
            let target = dir.join(&fname);
            if let Err(e) = std::fs::write(&target, &payload) {
                self.toast_from_ui(format!("{}: {e}", target.display()), true);
                return false;
            }
            let path_str = target.to_string_lossy().into_owned();
            state.library_path = path_str.clone();
            if self.caps.manage_apps && self.is_installed(app.id) {
                self.launch_app_with_file(app.id, &path_str);
            } else if self.caps.manage_apps {
                self.install_app(app.id);
            } else {
                self.detail = Some(app.id.to_string());
            }
            true
        }
        #[cfg(target_arch = "wasm32")]
        {
            self.detail = Some(app.id.to_string());
            true
        }
    }

    /// Tab 4 (`Plugins`): Craft Suite Extension & MCP Integration overview + quick launch.
    fn hub_plugins_tab(&mut self, ui: &mut Ui) -> bool {
        let t = Tokens::get(ui.ctx());
        let mut close_dialog = false;

        sheet::panel(ui, tl!("MCP gateway & agent config"), |ui| {
            let gateway_cmd = "craft-ai-mcp";
            ui.horizontal_wrapped(|ui| {
                if widgets::button(ui, tl!("Copy MCP JSON"), Kind::Secondary, 0.0).clicked() {
                    ui.ctx().copy_text(mcp_config(McpConfigFormat::Json, gateway_cmd));
                    self.toast_from_ui(tl!("MCP configuration copied to clipboard."), false);
                }
                if widgets::button(ui, tl!("Copy VS Code MCP"), Kind::Secondary, 0.0).clicked() {
                    ui.ctx().copy_text(mcp_config(McpConfigFormat::VsCode, gateway_cmd));
                    self.toast_from_ui(tl!("MCP configuration copied to clipboard."), false);
                }
                if widgets::button(ui, tl!("Copy Codex TOML"), Kind::Secondary, 0.0).clicked() {
                    ui.ctx().copy_text(mcp_config(McpConfigFormat::CodexToml, gateway_cmd));
                    self.toast_from_ui(tl!("MCP configuration copied to clipboard."), false);
                }
            });
        });

        sheet::panel(ui, tl!("Suite extensions & plug-in formats"), |ui| {
            for spec in &SUITE_PLUGINS {
                let Some(app) = artcraft_launcher_core::CraftApp::by_id(spec.app_id) else { continue };
                let installed = self.is_installed(app.id);
                egui::Frame::new().fill(t.sunken).stroke(t.hairline()).inner_margin(egui::Margin::symmetric(14, 10)).show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.horizontal(|ui| {
                        if let Some(icon) = crate::icons::app(app.id) {
                            ui.add(egui::Image::new(icon).fit_to_exact_size(vec2(32.0, 32.0)));
                        }
                        ui.add_space(6.0);
                        ui.vertical(|ui| {
                            ui.spacing_mut().item_spacing.y = 4.0;
                            ui.horizontal_wrapped(|ui| {
                                widgets::wordmark(ui, app, 15.5);
                                for fmt_tag in spec.formats {
                                    widgets::badge(ui, fmt_tag, t.app_ink(app));
                                }
                            });
                            ui.label(RichText::new(spec.runtime).size(12.0).color(t.muted));
                            ui.label(RichText::new(spec.summary).size(12.0).color(t.faint));
                        });
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if self.caps.manage_apps && installed {
                                let acc = fmt(tl!("Open in {name}"), &[("name", app.name)]);
                                if widgets::labelled_button(ui, tl!("Open"), &acc, Kind::Primary, 86.0).clicked() {
                                    self.launch_app(app.id);
                                    close_dialog = true;
                                }
                            } else if self.caps.manage_apps {
                                let acc = fmt(tl!("Install {name}"), &[("name", app.name)]);
                                if widgets::labelled_button(ui, tl!("Install"), &acc, Kind::Secondary, 86.0).clicked() {
                                    self.install_app(app.id);
                                    close_dialog = true;
                                }
                            } else {
                                let acc = fmt(tl!("{name} details"), &[("name", app.name)]);
                                if widgets::labelled_button(ui, tl!("App details"), &acc, Kind::Secondary, 96.0).clicked() {
                                    self.detail = Some(app.id.to_string());
                                    close_dialog = true;
                                }
                            }
                        });
                    });
                });
                ui.add_space(8.0);
            }
        });

        close_dialog
    }
}

fn build_starter_payload(raw_name: &str, use_ase: bool) -> (String, Vec<u8>) {
    let ext = if use_ase { LibraryFormat::Ase.extension() } else { LibraryFormat::CcLibs.extension() };
    let fname = format!("{}.{ext}", sanitize_stem(raw_name));
    let payload =
        if use_ase { build_starter_ase(raw_name, &STARTER_STUDIO_SWATCHES) } else { build_starter_cclibs(raw_name, &STARTER_STUDIO_SWATCHES) };
    (fname, payload)
}

fn draw_swatch_grid(ui: &mut Ui, swatches: &[SwatchColor], t: &Tokens) {
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = vec2(8.0, 8.0);
        for sw in swatches.iter().take(24) {
            let color = Color32::from_rgb(sw.rgb[0], sw.rgb[1], sw.rgb[2]);
            let hex = sw.hex();
            egui::Frame::new().fill(t.bg).stroke(t.hairline()).inner_margin(egui::Margin::symmetric(8, 6)).show(ui, |ui| {
                ui.horizontal(|ui| {
                    let (chip, _) = ui.allocate_exact_size(vec2(20.0, 20.0), Sense::hover());
                    ui.painter().rect(chip, CornerRadius::ZERO, color, Stroke::new(1.0, t.line_strong), egui::StrokeKind::Inside);
                    ui.vertical(|ui| {
                        ui.spacing_mut().item_spacing.y = 1.0;
                        ui.label(RichText::new(&sw.name).size(12.0).color(t.ink));
                        ui.label(RichText::new(format!("{hex} · {}", sw.model)).size(10.5).family(theme::mono()).color(t.faint));
                    });
                });
            });
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn folder_scanner_discovers_creative_files_and_ignores_unknown_extensions() {
        let dir = std::env::temp_dir().join(format!("artcraft-hub-scan-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("create temp dir");
        std::fs::write(dir.join("poster.psd"), b"8BPS").expect("write psd");
        std::fs::write(dir.join("logo.svg"), b"<svg/>").expect("write svg");
        std::fs::write(dir.join("edit.prproj"), b"<PremiereData/>").expect("write prproj");
        std::fs::write(dir.join("spread.indd"), b"indd").expect("write indd");
        std::fs::write(dir.join("notes.unknownext"), b"ignored").expect("write unknown");

        let scanned = scan_creative_files_in_dir(&dir, 50).expect("scan succeeds");
        let summary: Vec<(&str, &str)> = scanned.iter().map(|f| (f.name.as_str(), f.app_id)).collect();
        assert_eq!(
            summary,
            vec![("edit.prproj", "filmcraft"), ("logo.svg", "vectorcraft"), ("poster.psd", "photocraft"), ("spread.indd", "designcraft"),]
        );

        let capped = scan_creative_files_in_dir(&dir, 2).expect("bounded scan");
        assert_eq!(capped.len(), 2);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn bundled_fonts_and_filters_match_expected_categories() {
        let all = available_font_families();
        assert!(all.len() >= 3);
        assert!(all.iter().any(|f| f.family == "Inter Variable" && f.bundled && f.is_variable));
        assert!(all.iter().any(|f| f.family == "Geist Mono Variable" && f.is_monospace));
    }
}
