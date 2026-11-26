//! The MaterialX and MCMAT tabs: #190 publishing options, kept until the exporter that reads them lands

use egui::{RichText, Ui};

use super::{ExportSettings, ExportTab, ExposedControls, NormalOrientation, TextureDepth, outputs};
use crate::MaterialCraftApp;
use crate::i18n::tr;
use crate::theme::Tokens;

pub(super) fn show(app: &mut MaterialCraftApp, ui: &mut Ui, settings: &mut ExportSettings, tab: ExportTab) {
    let tokens = Tokens::get(ui.ctx());
    let contents = if tab == ExportTab::Mcmat {
        tr("One .mcmat archive: material.mtlx, material.json, resources/ and the optional preview.png")
    } else {
        tr("A folder: material.mtlx, material.json, resources/ and the optional preview.png")
    };
    ui.label(RichText::new(contents).size(11.0).color(tokens.text_dim));
    ui.add_space(4.0);

    super::form(ui, "export-material-options", |ui, width| {
        #[cfg(not(target_arch = "wasm32"))]
        {
            super::form_label(ui, tr("Destination"));
            let path = if tab == ExportTab::Mcmat { &mut settings.material.archive } else { &mut settings.material.folder };
            if super::path_field(ui, path, width) {
                ask_destination(app, tab);
            }
            ui.end_row();
        }

        let material = &mut settings.material;
        super::form_label(ui, tr("Resolution"));
        super::size_menu(ui, "Resolution", width, &mut material.size);
        ui.end_row();

        super::form_label(ui, tr("Textures"));
        super::dropdown(ui, "Textures", width, tr(material.texture_depth.label()), |ui| {
            for depth in TextureDepth::ALL {
                ui.selectable_value(&mut material.texture_depth, depth, tr(depth.label()));
            }
        });
        ui.end_row();

        super::form_label(ui, tr("Normal orientation"));
        super::dropdown(ui, "Normal orientation", width, tr(material.normal_orientation.label()), |ui| {
            for orientation in NormalOrientation::ALL {
                ui.selectable_value(&mut material.normal_orientation, orientation, tr(orientation.label()));
            }
        });
        ui.end_row();

        super::form_label(ui, tr("Displacement scale"));
        let field = ui.add(egui::DragValue::new(&mut material.displacement_scale).speed(0.01).range(0.0..=100.0).max_decimals(3));
        crate::widgets::name(&field, egui::Role::SpinButton, "Displacement scale");
        ui.end_row();

        super::form_label(ui, tr("Exposed parameters"));
        super::dropdown(ui, "Exposed parameters", width, tr(material.controls.label()), |ui| {
            for controls in ExposedControls::ALL {
                ui.selectable_value(&mut material.controls, controls, tr(controls.label()));
            }
        });
        ui.end_row();

        ui.label("");
        let preview = ui.checkbox(&mut material.include_preview, tr("Include preview.png"));
        crate::widgets::name(&preview, egui::Role::CheckBox, "Include preview.png");
        ui.end_row();
    });

    ui.add_space(6.0);
    ui.label(RichText::new(tr("Outputs")).strong());
    let graph = app.session.graph().ok().filter(|graph| graph.function.is_none());
    outputs::tree(ui, settings, graph.as_slice());
    ui.add_space(4.0);
    ui.label(RichText::new(tr("Publishing arrives with the new .mcmat format (#190); these options are kept for it")).size(11.0).color(tokens.text_dim));
}

/// A folder for MaterialX, an archive path for MCMAT
#[cfg(not(target_arch = "wasm32"))]
fn ask_destination(app: &mut MaterialCraftApp, tab: ExportTab) {
    use materialcraft_engine::formats::material::MATERIAL_EXT;

    let dialog = if tab == ExportTab::Mcmat {
        let graph = app.session.graph().map(|graph| graph.name.clone()).unwrap_or_default();
        let file = format!("{}.{MATERIAL_EXT}", materialcraft_engine::formats::safe_file_name(&graph));
        craft_dialogs::FileDialog::save().file_name(file).filter("MaterialCraft material", &[MATERIAL_EXT])
    } else {
        craft_dialogs::FileDialog::folder()
    };
    crate::native_dialogs::ask(app, dialog, crate::native_dialogs::Then::ExportDestination(tab));
}
