//! Dialogs (new graph, preferences, shortcuts, about, home), the console and history panels, file actions behind the File menu, and the function editor

use std::collections::HashMap;
use std::sync::mpsc::Receiver;

use egui::{Color32, RichText, TextureHandle, Ui, vec2};
use egui_phosphor::regular as icon;
use materialcraft_engine::core::FunctionGraph;
use materialcraft_engine::core::NodeId;
use serde_json::{Value, json};

use crate::MaterialCraftApp;
use crate::i18n::tr;
use crate::shell::modality::Modality;
use crate::theme::{Accent, Brightness, Tokens};

/// Template previews for the New graph dialog (rendered in the background on first open)
#[derive(Default)]
pub struct TemplatePreviews {
    tex: HashMap<String, TextureHandle>,
    rx: Option<Receiver<crate::thumbs::Previews>>,
    started: bool,
}

const PREVIEW: u32 = 96;

fn render_templates() -> crate::thumbs::Previews {
    let light = materialcraft_render::Lighting::new("studio");
    let scene = materialcraft_render::Scene::builtin("sphere");
    let settings = materialcraft_render::Settings { distance: 2.7, supersample: 2, ..Default::default() };
    materialcraft_engine::samples::TEMPLATES
        .iter()
        .filter_map(|(id, _)| {
            let mut s = materialcraft_engine::Session::new(materialcraft_engine::core::Package::new("t"));
            s.pkg.graphs.push(materialcraft_engine::samples::template(id, "t", 8).ok()?);
            s.preview_size = Some(256);
            s.cook();
            let maps = materialcraft_render::Maps::from_usages(s.material_maps());
            Some((id.to_string(), materialcraft_render::render(&scene, &maps, &light, &settings, PREVIEW, PREVIEW)))
        })
        .collect()
}

impl TemplatePreviews {
    fn get(&mut self, ctx: &egui::Context, id: &str) -> Option<egui::TextureId> {
        if !self.started {
            self.started = true;
            let (tx, rx) = std::sync::mpsc::channel();
            #[cfg(not(target_arch = "wasm32"))]
            {
                let c = ctx.clone();
                let _ = std::thread::Builder::new().name("materialcraft-templates".into()).spawn(move || {
                    let _ = tx.send(render_templates());
                    c.request_repaint();
                });
            }
            #[cfg(target_arch = "wasm32")]
            let _ = tx.send(render_templates());
            self.rx = Some(rx);
        }
        if let Some(rx) = &self.rx
            && let Ok(list) = rx.try_recv()
        {
            for (k, px) in list {
                let img = egui::ColorImage::from_rgba_unmultiplied([PREVIEW as usize, PREVIEW as usize], &px);
                self.tex.insert(k.clone(), ctx.load_texture(format!("tpl-{k}"), img, egui::TextureOptions::LINEAR));
            }
            self.rx = None;
        }
        self.tex.get(id).map(TextureHandle::id)
    }
}

thread_local! {
    static TEMPLATES: std::cell::RefCell<TemplatePreviews> = std::cell::RefCell::new(TemplatePreviews::default());
}

pub fn show(app: &mut MaterialCraftApp, ctx: &egui::Context) {
    app.thumbs.tick(ctx);
    if app.ui.windows.new_graph {
        new_graph(app, ctx);
    }
    if app.ui.windows.export {
        crate::export::show(app, ctx);
    }
    if app.ui.windows.preferences {
        preferences(app, ctx);
    }
    if app.ui.windows.about {
        about(app, ctx);
    }
    if app.ui.windows.shortcuts {
        shortcuts(app, ctx);
    }
    if app.ui.windows.home {
        home(app, ctx);
    }
    crate::unsaved::show(app, ctx);
    crate::explorer::dialogs(app, ctx);
    crate::properties::ExposeParameterDialog::show(app, ctx);
    #[cfg(target_arch = "wasm32")]
    crate::web_files::save_dialog(app, ctx);
    #[cfg(not(target_arch = "wasm32"))]
    crate::updates_ui::show(app, ctx);
}

pub(crate) fn modal(ctx: &egui::Context, id: &str, title: &str, open: &mut bool, size: egui::Vec2, body: impl FnOnce(&mut Ui)) {
    egui::Window::new(title)
        .id(egui::Id::unique(id))
        .open(open)
        .collapsible(false)
        .resizable(false)
        .default_size(size)
        .pivot(egui::Align2::CENTER_CENTER)
        .default_pos(ctx.content_rect().center())
        .show(ctx, body);
}

fn new_graph(app: &mut MaterialCraftApp, ctx: &egui::Context) {
    use materialcraft_engine::core::GraphKind;

    let mut open = true;
    let mut create = false;
    modal(ctx, "new-graph", tr("New graph"), &mut open, vec2(440.0, 290.0), |ui| {
        ui.horizontal(|ui| {
            ui.selectable_value(&mut app.ui.new_graph_kind, GraphKind::Material, tr("Graph"));
            ui.selectable_value(&mut app.ui.new_graph_kind, GraphKind::Function, tr("Function Graph"));
        });
        ui.add_space(12.0);
        ui.label(tr("Graph name"));
        let name = ui.text_edit_singleline(&mut app.ui.new_graph_name);
        crate::widgets::name(&name, egui::Role::TextInput, "Graph name");

        if app.ui.new_graph_kind == GraphKind::Material {
            ui.add_space(8.0);
            crate::widgets::combo(ui, tr("Template"), app.ui.new_graph_template.clone(), |ui| {
                for (id, label) in materialcraft_engine::samples::TEMPLATES {
                    ui.selectable_value(&mut app.ui.new_graph_template, id.to_string(), tr(label));
                }
            });
            let size = egui::ComboBox::from_label(tr("Size")).selected_text(format!("{} px", app.ui.new_graph_size)).show_ui(ui, |ui| {
                for size in [256u32, 512, 1024, 2048, 4096] {
                    ui.selectable_value(&mut app.ui.new_graph_size, size, format!("{size} px"));
                }
            });
            crate::widgets::name(&size.response, egui::Role::ComboBox, "Size");
        }

        ui.add_space(12.0);
        ui.label(format!("{}: {}", tr("Package"), app.session.pkg.name));
        ui.add_space(16.0);
        ui.horizontal(|ui| {
            if ui.add(crate::widgets::primary_button(ui, tr("Create"))).clicked() {
                create = true;
            }
            if ui.button(tr("Cancel")).clicked() {
                app.ui.windows.new_graph = false;
            }
        });
    });

    if create {
        let parameters =
            json!({"kind": app.ui.new_graph_kind, "template": app.ui.new_graph_template, "name": app.ui.new_graph_name, "size": app.ui.new_graph_size});
        if app.run("file.new", parameters).is_some() {
            app.ui.frame_all = true;
            open = false;
        }
    }
    if !open {
        app.ui.windows.new_graph = false;
    }
}

fn preferences(app: &mut MaterialCraftApp, ctx: &egui::Context) {
    let mut open = true;
    modal(ctx, "prefs", tr("Preferences"), &mut open, vec2(380.0, 340.0), |ui| {
        let p = &mut app.ui.prefs;
        let current_lang = crate::i18n::current();
        crate::widgets::combo(ui, tr("Language"), current_lang.name, |ui| {
            for l in &crate::i18n::LANGUAGES {
                if ui.selectable_label(p.language == l.code, l.name).clicked() {
                    p.language = l.code.to_owned();
                    crate::i18n::set_language(&p.language);
                }
            }
        });
        crate::widgets::combo(ui, tr("Brightness"), tr(p.brightness.label()), |ui| {
            for b in Brightness::ALL {
                ui.selectable_value(&mut p.brightness, b, tr(b.label()));
            }
        });
        crate::widgets::combo(ui, tr("Accent color"), tr(p.accent.label()), |ui| {
            for a in Accent::ALL {
                ui.selectable_value(&mut p.accent, a, tr(a.label()));
            }
        });
        crate::widgets::slider(ui, tr("Interface scale"), &mut p.ui_scale, 0.75..=2.0, 2);
        let mut auto = p.autosave_minutes as f32;
        if crate::widgets::slider(ui, tr("Autosave every (minutes, 0 = off)"), &mut auto, 0.0..=60.0, 0).changed() {
            p.autosave_minutes = auto.round() as u32;
        }
        let mut undo_limit = p.history.undo_limit as f32;
        if crate::widgets::slider(ui, tr("Undo history (steps)"), &mut undo_limit, 1.0..=10_000.0, 0).changed() {
            p.history.undo_limit = undo_limit.round() as u32;
        }
        crate::widgets::combo(ui, tr("Preview size while editing"), crate::preview_resolution::label(p.preview_size), |ui| {
            crate::preview_resolution::choices(ui, &mut p.preview_size);
        });
        ui.checkbox(&mut p.thumbnails, tr("Show node thumbnails"));
        crate::widgets::slider(ui, tr("3D pan limit (mesh radii)"), &mut p.camera.pan_limit, 0.0..=20.0, 1);
        crate::widgets::slider(ui, tr("3D pan speed"), &mut p.camera.pan_speed, 0.1..=4.0, 2);
    });
    if !open {
        app.ui.windows.preferences = false;
    }
}

fn about(app: &mut MaterialCraftApp, ctx: &egui::Context) {
    let mut open = true;
    modal(ctx, "about", tr("About MaterialCraft"), &mut open, vec2(400.0, 260.0), |ui| {
        let tk = Tokens::get(ui.ctx());
        ui.vertical_centered(|ui| {
            ui.label(RichText::new("MaterialCraft").size(22.0).strong().color(tk.accent));
            ui.label(format!("{} {}", tr("Version"), materialcraft_engine::VERSION));
            ui.add_space(6.0);
            ui.label(tr("Node-based procedural materials, built in pure Rust."));
            ui.label(tr("An open-source Crafting App by the ArtCraft team and community."));
            ui.add_space(6.0);
            ui.hyperlink_to("getartcraft.com/apps/materialcraft", "https://getartcraft.com/apps/materialcraft");
            ui.hyperlink_to("discord.gg/artcraft", "https://discord.gg/artcraft");
            ui.add_space(6.0);
            ui.label(RichText::new("MIT OR Apache-2.0").size(11.0).color(tk.text_dim));
        });
    });
    if !open {
        app.ui.windows.about = false;
    }
}

fn shortcuts(app: &mut MaterialCraftApp, ctx: &egui::Context) {
    let mut open = true;
    modal(ctx, "shortcuts", tr("Shortcuts"), &mut open, vec2(420.0, 420.0), |ui| {
        egui::ScrollArea::vertical().max_height(380.0).show(ui, |ui| {
            egui::Grid::new("sc").striped(true).show(ui, |ui| {
                for i in crate::menus::CATALOG.iter().filter(|i| i.shortcut.is_some()) {
                    ui.label(tr(i.label));
                    ui.label(RichText::new(crate::menus::shortcut_label(ctx, i.shortcut.unwrap_or_default())).monospace());
                    ui.end_row();
                }
                for (k, what) in [
                    ("Space / Tab", "Node search (over the graph)"),
                    ("F", "Frame all / the selection"),
                    ("Wheel", "Zoom the graph and the views"),
                    ("Middle drag / Alt+drag", "Pan the graph"),
                    ("Double-click node", "Show in the 2D view"),
                    ("Shift+D", "Disable selection"),
                    ("Middle drag / Alt+drag (3D view)", "Pan the camera"),
                    ("Shift+drag (3D view)", "Turn the environment"),
                ] {
                    ui.label(tr(what));
                    ui.label(RichText::new(k).monospace());
                    ui.end_row();
                }
            });
        });
    });
    if !open {
        app.ui.windows.shortcuts = false;
    }
}

fn home(app: &mut MaterialCraftApp, ctx: &egui::Context) {
    let mut open = true;
    let mut create: Option<(&str, &str)> = None;
    let modality = Modality::capture(app, ctx);
    let free = !modality.is_blocked();
    modal(ctx, "home", tr("Home"), &mut open, vec2(760.0, 470.0), |ui| {
        let tk = Tokens::get(ui.ctx());
        ui.horizontal_top(|ui| {
            // Left rail: actions and links
            ui.vertical(|ui| {
                ui.set_width(150.0);
                if ui.add_enabled(free, crate::widgets::primary_button(ui, tr("New graph"))).clicked() {
                    app.ui.windows.home = false;
                    app.ui.windows.new_graph = true;
                }
                ui.add_space(4.0);
                if ui.add_enabled(modality.allows("file.open"), egui::Button::new(format!("{}  {}", icon::FOLDER_OPEN, tr("Open package...")))).clicked() {
                    app.ui.windows.home = false;
                    crate::menus::invoke(app, ui.ctx(), "file.open");
                }
                ui.add_space(16.0);
                ui.hyperlink_to(tr("Documentation"), "https://github.com/storytold/materialcraft#readme");
                ui.hyperlink_to(tr("Community (Discord)"), "https://discord.gg/artcraft");
                ui.hyperlink_to("getartcraft.com", "https://getartcraft.com/apps/materialcraft");
                ui.add_space(16.0);
                ui.checkbox(&mut app.ui.prefs.show_home, tr("Show at startup"));
            });
            ui.separator();
            ui.vertical(|ui| {
                ui.label(RichText::new(tr("Welcome to MaterialCraft")).size(18.0).strong().color(tk.text_strong));
                ui.label(
                    RichText::new(tr(
                        "Build tileable materials from noises, patterns and filters, see them live in 2D and 3D, and export the maps for any engine.",
                    ))
                    .color(tk.text_dim),
                );
                ui.add_space(8.0);
                ui.label(RichText::new(tr("Samples")).strong());
                ui.horizontal_wrapped(|ui| {
                    if !free {
                        ui.disable();
                    }
                    for (id, label) in materialcraft_engine::samples::TEMPLATES.iter().filter(|(id, _)| !matches!(*id, "empty" | "pbr")) {
                        let (r, resp) = ui.allocate_exact_size(vec2(92.0, 116.0), egui::Sense::click());
                        crate::widgets::name(&resp, egui::Role::ListItem, label);
                        if free && resp.hovered() {
                            ui.painter().rect_filled(r, 6.0, tk.row_hover);
                        }
                        let img = egui::Rect::from_min_size(r.min + vec2(6.0, 4.0), vec2(80.0, 80.0));
                        match TEMPLATES.with(|t| t.borrow_mut().get(ui.ctx(), id)) {
                            Some(t) => {
                                ui.painter().image(t, img, egui::Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(1.0, 1.0)), Color32::WHITE);
                            }
                            None => {
                                ui.painter().circle_filled(img.center(), 36.0, tk.row);
                            }
                        }
                        let g = ui.painter().layout(tr(label).to_string(), egui::FontId::proportional(11.0), tk.text, 90.0);
                        ui.painter().galley(egui::pos2(r.center().x - g.size().x * 0.5, img.bottom() + 6.0), g, tk.text);
                        if free && resp.clicked() {
                            create = Some((id, label));
                        }
                    }
                });
                ui.add_space(8.0);
                ui.label(RichText::new(tr("Recent")).strong());
                if app.ui.recent.is_empty() {
                    ui.label(RichText::new(tr("No recent packages")).color(tk.text_dim));
                }
                #[cfg(not(target_arch = "wasm32"))]
                for p in app.ui.recent.clone().into_iter().take(6) {
                    if ui.add_enabled(free, egui::Link::new(&p)).clicked() {
                        open_path(app, &p);
                        app.ui.windows.home = false;
                    }
                }
            });
        });
    });
    if let Some((id, label)) = create {
        if app.run("file.new", json!({"template": id, "name": label.replace(' ', "_"), "size": 1024})).is_some() {
            app.ui.frame_all = true;
        }
        open = false;
    }
    if !open {
        app.ui.windows.home = false;
    }
}

pub fn console(app: &mut MaterialCraftApp, ui: &mut Ui) {
    let tk = Tokens::get(ui.ctx());
    let console_line = |text: String, color: Color32| RichText::new(text).monospace().size(11.0).color(color);

    egui::ScrollArea::vertical().stick_to_bottom(true).auto_shrink([false, false]).show(ui, |ui| {
        for line in &app.session.log {
            let warning = line.starts_with(materialcraft_engine::WARNING_PREFIX);
            ui.label(console_line(line.clone(), if warning { tk.warning } else { tk.text }));
        }

        // Placeholders for missing SBS nodes are already listed in the warnings, so they get one line
        let (failed, placeholders) = app.session.cook_failures();
        for (id, error) in failed {
            ui.label(console_line(format!("node {}: {error}", id.0), tk.danger));
        }
        if placeholders > 0 {
            let text = format!("{placeholders} {}", tr("placeholder node(s) stand in for nodes MaterialCraft doesn't have yet (see the warnings above)"));
            ui.label(console_line(text, tk.warning));
        }
    });
}

pub fn history(app: &mut MaterialCraftApp, ui: &mut Ui) {
    let tk = Tokens::get(ui.ctx());
    egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
        for (i, h) in app.session.document_history_labels(crate::history::document(app)).iter().enumerate() {
            ui.label(RichText::new(format!("{:>3}  {h}", i + 1)).size(11.5).color(tk.text));
        }
        ui.horizontal(|ui| {
            if ui.add_enabled(crate::menus::enabled(app, "edit.undo"), egui::Button::new(format!("{} {}", icon::ARROW_U_UP_LEFT, tr("Undo")))).clicked() {
                app.run("edit.undo", json!({}));
            }
            if ui.add_enabled(crate::menus::enabled(app, "edit.redo"), egui::Button::new(format!("{} {}", icon::ARROW_U_UP_RIGHT, tr("Redo")))).clicked() {
                app.run("edit.redo", json!({}));
            }
        });
    });
}

/// The function parameter of a pixel/value processor: a button opening the function graph editor, and the function as JSON for copy and paste
pub fn function_summary(app: &mut MaterialCraftApp, ui: &mut Ui, node: NodeId, param: &str, f: &FunctionGraph) {
    let tk = Tokens::get(ui.ctx());
    if ui.add(crate::widgets::primary_button(ui, &format!("{}  {}", icon::FLOW_ARROW, tr("Edit function graph...")))).clicked() {
        crate::graph::subgraph_navigation::open_function(app, node, crate::function::FunctionTarget::Parameter(param.to_string()));
    }
    ui.label(RichText::new(format!("{} {}", f.nodes.len(), tr("function nodes"))).size(10.5).color(tk.text_dim));
    let id = egui::Id::unique(("fn-edit", node.0, param));
    egui::CollapsingHeader::new(tr("As JSON")).id_salt(("fn-json", node.0, param)).default_open(false).show(ui, |ui| {
        let mut text: String = ui.data(|d| d.get_temp(id)).unwrap_or_else(|| serde_json::to_string_pretty(f).unwrap_or_default());
        if ui.add(egui::TextEdit::multiline(&mut text).code_editor().desired_rows(8).desired_width(f32::INFINITY)).changed() {
            ui.data_mut(|d| d.insert_temp(id, text.clone()));
        }
        ui.horizontal(|ui| {
            if ui.button(tr("Apply")).clicked() {
                match serde_json::from_str::<FunctionGraph>(&text) {
                    Ok(g) => {
                        let v = serde_json::to_value(materialcraft_engine::core::Value::Function(Box::new(g))).unwrap_or(Value::Null);
                        app.run("graph.node.param", json!({"id": node.0, "param": param, "value": v}));
                        ui.data_mut(|d| d.remove::<String>(id));
                    }
                    Err(e) => app.toast(format!("{}: {e}", tr("Bad function")), true),
                }
            }
            if ui.button(tr("Revert")).clicked() {
                ui.data_mut(|d| d.remove::<String>(id));
            }
        });
    });
}

/// Project formats offered by File ▸ Open.
pub const OPEN_EXTS: &[&str] = &["sbs", "mcraft"];

pub(crate) fn package_filters(open: bool, archive: bool) -> Vec<(&'static str, &'static [&'static str])> {
    let mut filters: Vec<(&str, &[&str])> = vec![("SBS XML Package (*.sbs)", &["sbs"]), ("MaterialCraft Project (*.mcraft)", &["mcraft"])];
    if archive {
        filters.reverse();
    }
    if open {
        filters.insert(0, ("All formats (*.sbs *.mcraft)", OPEN_EXTS));
    }
    filters
}

#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn package_dialog(open: bool, archive: bool) -> craft_dialogs::FileDialog {
    let mut dialog = if open { craft_dialogs::FileDialog::open() } else { craft_dialogs::FileDialog::save() };
    for (name, extensions) in package_filters(open, archive) {
        dialog = dialog.filter(name, extensions);
    }

    dialog
}

#[cfg(not(target_arch = "wasm32"))]
fn resource_dialog(vector: bool) -> craft_dialogs::FileDialog {
    let mut dialog = craft_dialogs::FileDialog::open();
    for (label, extensions) in resource_filters(vector) {
        dialog = dialog.filter(label, extensions);
    }

    dialog
}

pub(crate) fn resource_filters(vector: bool) -> Vec<(String, &'static [&'static str])> {
    use materialcraft_engine::formats::{BITMAP_FORMATS, VECTOR_FORMATS};
    let formats = if vector { VECTOR_FORMATS } else { BITMAP_FORMATS };
    formats
        .iter()
        .map(|(label, extensions)| {
            let patterns = extensions.iter().map(|extension| format!("*.{extension}")).collect::<Vec<_>>().join(" ");
            (format!("{label} ({patterns})"), *extensions)
        })
        .collect()
}

/// Import an asset into the active package without creating a graph node.
pub fn import_resource(app: &mut MaterialCraftApp, ctx: &egui::Context, vector: bool) {
    import_resource_into(app, ctx, vector, "Resources");
}

pub(crate) fn import_resource_into(app: &mut MaterialCraftApp, ctx: &egui::Context, vector: bool, folder: &str) {
    if !app.session.has_package() {
        return;
    }

    #[cfg(not(target_arch = "wasm32"))]
    {
        let _ = ctx;
        let then = crate::native_dialogs::Then::ImportResource { document: app.session.document_id, vector, folder: folder.to_string() };
        crate::native_dialogs::ask(app, resource_dialog(vector), then);
    }
    #[cfg(target_arch = "wasm32")]
    crate::web_files::import_resource(ctx, app.session.document_id, vector, folder);
}

/// After a package opened with warnings (an SBS file using nodes MaterialCraft lacks, say), bring the Console forward
pub fn after_open(app: &mut MaterialCraftApp) {
    let warning_count = app.session.load_warnings.len();
    if warning_count > 0 {
        app.ui.dock.focus(crate::docking::Panel::Console);
        app.toast(format!("{} {warning_count} {}", tr("Opened with"), tr("warnings: see the Console")), false);
    }
}

/// File menu actions that need a file dialog. Returns true when handled
pub fn file_action(app: &mut MaterialCraftApp, ctx: &egui::Context, id: &str) -> bool {
    if matches!(id, "file.save" | "file.saveAs" | "file.saveCopyAs" | "file.importBitmap" | "file.importVector") && !app.session.has_package() {
        return true;
    }

    #[cfg(not(target_arch = "wasm32"))]
    let _ = ctx;
    #[cfg(not(target_arch = "wasm32"))]
    {
        use crate::native_dialogs::{self, Then};
        match id {
            "file.open" => {
                native_dialogs::ask(app, package_dialog(true, false), Then::OpenPackage);
                true
            }
            "file.save" if has_local_path(app) => {
                if let Some(r) = app.run("file.save", json!({})) {
                    let path = r["path"].as_str().unwrap_or_default().to_string();
                    app.ui.add_recent(&path);
                    app.toast(format!("{} {path}", tr("Saved")), false);
                }
                true
            }
            "file.save" | "file.saveAs" | "file.saveCopyAs" => {
                native_dialogs::save_package(app, id == "file.saveCopyAs");
                true
            }
            "file.exportMaterial" => {
                crate::export::open(app, crate::export::ExportTab::Mcmat);
                true
            }
            "file.sendToTextureCraft" => {
                if app.run("file.sendToTextureCraft", json!({})).is_some() {
                    app.toast(tr("Sent to TextureCraft: find it on the shelf under My smart materials"), false);
                }
                true
            }
            "file.importBitmap" => {
                let then = Then::ImportBitmap(native_dialogs::GraphRef::active(app));
                native_dialogs::ask(app, resource_dialog(false), then);
                true
            }
            "file.importVector" => {
                import_resource(app, ctx, true);
                true
            }
            _ => false,
        }
    }
    #[cfg(target_arch = "wasm32")]
    {
        match id {
            "file.open" => crate::web_files::open(ctx),
            "file.importBitmap" => crate::web_files::import_bitmap(app, ctx),
            "file.importVector" => import_resource(app, ctx, true),
            "file.save" => crate::web_files::save(app),
            "file.saveAs" => crate::web_files::save_as(app, false),
            "file.saveCopyAs" => crate::web_files::save_copy(app),
            "file.exportMaterial" => crate::export::open(app, crate::export::ExportTab::Mcmat),
            _ => return false,
        }
        true
    }
}

/// Whether File ▸ Save writes straight to the package's path (a package opened from a URL has none to write to)
#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn has_local_path(app: &MaterialCraftApp) -> bool {
    app.session.path.as_ref().is_some_and(|path| !path.contains("://"))
}

/// Open a package from a path (recent list, command line)
#[cfg(not(target_arch = "wasm32"))]
pub fn open_path(app: &mut MaterialCraftApp, path: &str) {
    open_path_now(app, path);
}

#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn open_path_now(app: &mut MaterialCraftApp, path: &str) {
    if app.run("file.open", json!({"path": path})).is_some() {
        app.ui.add_recent(path);
        app.ui.frame_all = true;
        app.toast(format!("{} {path}", tr("Opened")), false);
        after_open(app);
    }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
#[path = "windows.test.rs"]
mod tests;
