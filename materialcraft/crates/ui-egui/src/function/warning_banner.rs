use crate::{i18n::tr, theme::Tokens};
use egui::Ui;
use materialcraft_engine::{
    FunctionGraphTarget,
    core::{FunctionGraph, Graph},
};

pub(super) fn show(ui: &mut Ui, document: &Graph, target: &FunctionGraphTarget, graph: &FunctionGraph) {
    let messages = materialcraft_engine::function_inspection::warnings_for(document, target, graph);
    let expected = materialcraft_engine::function_output::expected_type(document, target);
    if messages.is_empty() && expected.is_none() {
        return;
    }

    let tokens = Tokens::get(ui.ctx());
    egui::Frame::NONE.fill(tokens.panel).inner_margin(egui::Margin::symmetric(8, 4)).show(ui, |ui| {
        ui.set_width(ui.available_width());
        if let Some(expected) = expected {
            ui.colored_label(tokens.text_dim, format!("{}: {}", tr("Expected output"), expected.label()));
        }
        for message in messages {
            ui.colored_label(tokens.warning, format!("{}  {message}", egui_phosphor::regular::WARNING))
                .on_hover_text(tr("Choose an output of the expected type and connect it to a value source, such as a constant or variable input"));
        }
    });
}
