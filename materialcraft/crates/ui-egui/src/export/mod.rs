//! The Export window: image files from the open graph or a batch of graphs, and the #190 MaterialX and MCMAT options

mod export_format;
mod export_settings;
#[cfg(test)]
mod export_tests;
mod material_export_settings;
mod material_tab;
mod outputs;

pub use export_format::ExportFormat;
pub use export_settings::{BatchExportSettings, ExportSettings, ExportTab, output_key};
pub use material_export_settings::{ExposedControls, MaterialExportSettings, NormalOrientation, TextureDepth};

use egui::{RichText, Ui, vec2};
use egui_phosphor::regular as icon;
use materialcraft_engine::core::Graph;
use serde_json::{Value, json};

use crate::MaterialCraftApp;
use crate::i18n::tr;
use crate::theme::Tokens;

/// Output sizes the size lists offer; 0 keeps the graph's size
const SIZES: [u32; 7] = [0, 256, 512, 1024, 2048, 4096, 8192];

/// File name patterns the Pattern menu offers
const PATTERNS: [&str; 3] = ["$(graph)_$(identifier)", "$(identifier)", "$(graph)_$(usage)"];

/// What the window's buttons asked for this frame
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Action {
    Export,
    SaveSettings,
    Close,
}

/// Open the Export window on `tab`
pub(crate) fn open(app: &mut MaterialCraftApp, tab: ExportTab) {
    app.ui.export_tab = tab;
    app.ui.windows.export = true;
}

/// Draw the window on a copy of the saved settings: Save settings and Export keep it, Close drops it
pub(crate) fn show(app: &mut MaterialCraftApp, ctx: &egui::Context) {
    let mut open = true;
    let mut action = None;
    let mut draft = app.ui.export_draft.take().unwrap_or_else(|| app.ui.export.clone());
    crate::windows::modal(ctx, "export", tr("Export"), &mut open, vec2(640.0, 560.0), |ui| {
        ui.set_min_width(600.0);
        tabs(ui, &mut app.ui.export_tab);
        ui.separator();

        let tab = app.ui.export_tab;
        match tab {
            ExportTab::FromGraph => from_graph(app, ui, &mut draft),
            ExportTab::Batch => batch(app, ui, &mut draft),
            ExportTab::MaterialX | ExportTab::Mcmat => material_tab::show(app, ui, &mut draft, tab),
        }

        ui.add_space(8.0);
        action = footer(ui, tab);
    });

    let close = match action {
        Some(Action::Export) => {
            app.ui.export = draft.clone();
            export(app, &draft)
        }
        Some(Action::SaveSettings) => {
            app.ui.export = draft.clone();
            app.toast(tr("Export settings saved"), false);
            false
        }
        Some(Action::Close) => true,
        None => false,
    };
    if open && !close {
        app.ui.export_draft = Some(draft);
    } else {
        app.ui.windows.export = false;
    }
}

fn tabs(ui: &mut Ui, current: &mut ExportTab) {
    ui.horizontal(|ui| {
        for tab in ExportTab::ALL {
            let button = ui.selectable_label(*current == tab, tr(tab.label()));
            crate::widgets::name(&button, egui::Role::Tab, tab.label());
            if button.clicked() {
                *current = tab;
            }
        }
    });
}

fn from_graph(app: &mut MaterialCraftApp, ui: &mut Ui, settings: &mut ExportSettings) {
    let graph = app.session.graph().ok().filter(|graph| graph.function.is_none()).cloned();
    image_options(app, ui, settings, graph.as_ref());
    ui.add_space(6.0);
    ui.label(RichText::new(tr("Outputs")).strong());
    outputs::tree(ui, settings, graph.as_slice().iter().collect::<Vec<_>>().as_slice());
    all_or_none(ui, settings, graph.as_slice());

    ui.add_space(4.0);
    ui.horizontal(|ui| {
        let automatic = ui.checkbox(&mut settings.automatic, tr("Automatic export when outputs change"));
        crate::widgets::name(&automatic, egui::Role::CheckBox, "Automatic export when outputs change");
        if settings.automatic {
            ui.label(RichText::new(tr("not available yet")).size(11.0).color(Tokens::get(ui.ctx()).text_dim));
        }
    });
}

fn batch(app: &mut MaterialCraftApp, ui: &mut Ui, settings: &mut ExportSettings) {
    let graphs = material_graphs(app);
    image_options(app, ui, settings, graphs.first());
    ui.add_space(6.0);
    ui.label(RichText::new(tr("Outputs")).strong());
    outputs::tree(ui, settings, graphs.iter().collect::<Vec<_>>().as_slice());
    all_or_none(ui, settings, &graphs);

    ui.add_space(4.0);
    let size = &mut settings.batch;
    ui.horizontal(|ui| {
        ui.label(RichText::new(format!("{}:", tr("Output size"))).color(Tokens::get(ui.ctx()).text_dim));
        let (mut width, mut height) = (size.width, size.height);
        size_menu(ui, "Output width", 110.0, &mut width);
        let link = if size.linked { icon::LINK } else { icon::LINK_BREAK };
        if crate::widgets::icon_button(ui, link, tr("Link width and height"), size.linked, 22.0).clicked() {
            size.linked = !size.linked;
        }
        size_menu(ui, "Output height", 110.0, &mut height);

        // The graph's own size applies to both sides, and linked sides follow the one that changed
        if width != size.width {
            height = if size.linked || width == 0 || height == 0 { width } else { height };
        } else if height != size.height {
            width = if size.linked || width == 0 || height == 0 { height } else { width };
        }
        (size.width, size.height) = (width, height);
    });
}

/// Destination, format, bit depth, file name pattern and its preview
fn image_options(app: &mut MaterialCraftApp, ui: &mut Ui, settings: &mut ExportSettings, preview_graph: Option<&Graph>) {
    let preview = preview_graph.and_then(|graph| preview_file(settings, graph)).unwrap_or_default();
    form(ui, "export-image-options", |ui, width| {
        #[cfg(not(target_arch = "wasm32"))]
        {
            form_label(ui, tr("Destination"));
            if path_field(ui, &mut settings.destination, width) {
                let tab = app.ui.export_tab;
                crate::native_dialogs::ask(app, craft_dialogs::FileDialog::folder(), crate::native_dialogs::Then::ExportDestination(tab));
            }
            ui.end_row();
        }
        #[cfg(target_arch = "wasm32")]
        let _ = &app;

        form_label(ui, tr("Format"));
        let selected = ExportFormat::find(&settings.format).map_or_else(|| settings.format.clone(), ExportFormat::title);
        dropdown(ui, "Format", width, selected, |ui| {
            for format in ExportFormat::all() {
                let title = if format.available { format.title() } else { format!("{} - {}", format.title(), tr("not available yet")) };
                ui.selectable_value(&mut settings.format, format.extension.to_string(), title);
            }
        });
        ui.end_row();

        form_label(ui, tr("Bit depth"));
        let depth_label = match settings.depth {
            8 => "8 bits",
            16 => "16 bits",
            32 => "32 bits float",
            _ => "Output's format",
        };
        dropdown(ui, "Bit depth", width, tr(depth_label), |ui| {
            for (depth, label) in [(0, "Output's format"), (8, "8 bits"), (16, "16 bits"), (32, "32 bits float")] {
                ui.selectable_value(&mut settings.depth, depth, tr(label));
            }
        });
        ui.end_row();

        form_label(ui, tr("Pattern"));
        ui.horizontal(|ui| {
            let field = ui.add(egui::TextEdit::singleline(&mut settings.pattern).desired_width(width - 34.0));
            crate::widgets::name(&field, egui::Role::TextInput, "Pattern");
            ui.menu_button(icon::CARET_DOWN, |ui| {
                for pattern in PATTERNS {
                    if ui.button(pattern).clicked() {
                        settings.pattern = pattern.to_string();
                        ui.close();
                    }
                }
            });
        });
        ui.end_row();

        form_label(ui, tr("Preview"));
        ui.label(RichText::new(preview).italics());
        ui.end_row();
    });
}

/// Labelled rows whose fields line up and share the window's width
pub(super) fn form(ui: &mut Ui, id: &str, rows: impl FnOnce(&mut Ui, f32)) {
    const LABEL_WIDTH: f32 = 130.0;

    let field_width = (ui.available_width() - LABEL_WIDTH - 10.0).max(120.0);
    egui::Grid::new(id).num_columns(2).min_col_width(LABEL_WIDTH).spacing([10.0, 6.0]).show(ui, |ui| rows(ui, field_width));
}

/// A row label, right-aligned against its field
pub(super) fn form_label(ui: &mut Ui, text: &str) {
    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| ui.label(RichText::new(format!("{text}:")).color(Tokens::get(ui.ctx()).text_dim)));
}

/// A dropdown `width` wide whose list shows every choice, named `name` for screen readers and tests
pub(super) fn dropdown(ui: &mut Ui, name: &str, width: f32, selected: impl Into<egui::WidgetText>, choices: impl FnOnce(&mut Ui)) {
    let combo = egui::ComboBox::from_id_salt(("export-dropdown", name)).width(width).height(400.0).selected_text(selected).show_ui(ui, choices);
    crate::widgets::name(&combo.response, egui::Role::ComboBox, name);
}

/// A path field with a button that asks for the path; true when the button was clicked
#[cfg(not(target_arch = "wasm32"))]
pub(super) fn path_field(ui: &mut Ui, path: &mut String, width: f32) -> bool {
    ui.horizontal(|ui| {
        let field = ui.add(egui::TextEdit::singleline(path).desired_width(width - 34.0));
        crate::widgets::name(&field, egui::Role::TextInput, "Destination");
        crate::widgets::icon_button(ui, icon::FOLDER_OPEN, tr("Choose..."), false, 22.0).clicked()
    })
    .inner
}

/// The file name the first checked output of `graph` is written to
fn preview_file(settings: &ExportSettings, graph: &Graph) -> Option<String> {
    let output = outputs::listed(graph).into_iter().find(|output| settings.includes(&graph.name, &output.identifier))?;
    let name = materialcraft_engine::formats::output_name(&settings.pattern, &graph.name, &output.identifier, output.usage);
    Some(format!("{name}.{}", settings.format))
}

fn all_or_none(ui: &mut Ui, settings: &mut ExportSettings, graphs: &[Graph]) {
    ui.horizontal(|ui| {
        for (label, include) in [("All", true), ("None", false)] {
            if ui.add(egui::Button::new(tr(label)).min_size(vec2(100.0, 0.0))).clicked() {
                for graph in graphs {
                    settings.include_all(graph, include);
                }
            }
        }
    });
}

/// A list of output sizes; 0 is the graph's size
pub(super) fn size_menu(ui: &mut Ui, name: &str, width: f32, pixels: &mut u32) {
    let label = |size: u32| if size == 0 { tr("Graph's size").to_string() } else { size.to_string() };
    dropdown(ui, name, width, label(*pixels), |ui| {
        for size in SIZES {
            ui.selectable_value(pixels, size, label(size));
        }
    });
}

fn footer(ui: &mut Ui, tab: ExportTab) -> Option<Action> {
    let export_label = match tab {
        ExportTab::FromGraph => "Export outputs",
        ExportTab::Batch => "Start export",
        ExportTab::MaterialX | ExportTab::Mcmat => "Export",
    };
    let width = (ui.available_width() - 16.0) / 3.0;
    let mut action = None;
    ui.horizontal(|ui| {
        if ui.add(crate::widgets::primary_button(ui, tr(export_label)).min_size(vec2(width, 26.0))).clicked() {
            action = Some(Action::Export);
        }
        if ui.add(egui::Button::new(tr("Save settings")).min_size(vec2(width, 26.0))).clicked() {
            action = Some(Action::SaveSettings);
        }
        if ui.add(egui::Button::new(tr("Close")).min_size(vec2(width, 26.0))).clicked() {
            action = Some(Action::Close);
        }
    });
    action
}

/// The package's material graphs, in package order
fn material_graphs(app: &MaterialCraftApp) -> Vec<Graph> {
    app.session.pkg.graphs.iter().filter(|graph| graph.function.is_none()).cloned().collect()
}

/// Export what the current tab describes; true when the window can close
fn export(app: &mut MaterialCraftApp, settings: &ExportSettings) -> bool {
    match app.ui.export_tab {
        ExportTab::MaterialX => {
            app.toast(tr("MaterialX export isn't available yet (#190)"), true);
            false
        }
        ExportTab::Mcmat => {
            app.toast(tr(".mcmat export isn't available yet (#190)"), true);
            false
        }
        ExportTab::FromGraph => {
            let graph = app.session.graph;
            export_images(app, settings, &[graph], Value::Null)
        }
        ExportTab::Batch => {
            let graphs: Vec<usize> = app.session.pkg.graphs.iter().enumerate().filter(|(_, graph)| graph.function.is_none()).map(|(index, _)| index).collect();
            let size = &settings.batch;
            let size = if size.width == 0 || size.height == 0 { Value::Null } else { json!([size.width, size.height]) };
            export_images(app, settings, &graphs, size)
        }
    }
}

/// Run `file.export` for each of `graphs` with a checked output; true when every export succeeded
fn export_images(app: &mut MaterialCraftApp, settings: &ExportSettings, graphs: &[usize], size: Value) -> bool {
    let format = ExportFormat::find(&settings.format);
    if let Some(format) = format.filter(|format| !format.available) {
        app.toast(crate::i18n::trf("{format} export isn't available yet", &[("format", &format.title())]), true);
        return false;
    }
    if settings.destination.trim().is_empty() && !cfg!(target_arch = "wasm32") {
        app.toast(tr("Choose a folder to export to"), true);
        return false;
    }

    let depth = if settings.depth == 0 { Value::Null } else { json!(settings.depth) };
    let requests: Vec<Value> = graphs
        .iter()
        .filter_map(|&index| {
            let (outputs, color_spaces) = settings.selection(app.session.pkg.graphs.get(index)?)?;
            Some(json!({"dir": settings.destination, "format": settings.format, "pattern": settings.pattern, "depth": depth, "size": size, "graph": index, "outputs": outputs, "colorSpaces": color_spaces}))
        })
        .collect();
    if requests.is_empty() {
        app.toast(tr("Check at least one output to export"), true);
        return false;
    }

    #[cfg(target_arch = "wasm32")]
    {
        for request in &requests {
            crate::web_files::export(app, request);
        }
        true
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let mut files = 0;
        for request in requests {
            let Some(result) = app.run("file.export", request) else { return false };
            files += result["files"].as_array().map_or(0, Vec::len);
        }
        app.toast(format!("{} {files} {}", tr("Exported"), tr("files")), false);
        true
    }
}
