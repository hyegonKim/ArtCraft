use crate::{MaterialCraftApp, i18n::tr};
use egui::Ui;
use serde_json::json;

/// The SBS pin label is edited as its Description property.
pub(super) fn show(app: &mut MaterialCraftApp, ui: &mut Ui, id: u64) {
    let Some(mut name) =
        app.session.annotation_target.items(&app.session).ok().and_then(|items| items.pins.into_iter().find(|pin| pin.id == id)).map(|pin| pin.name)
    else {
        return;
    };

    crate::properties::section(ui, "pin-parameters", tr("Parameters"), |ui| {
        ui.label(tr("Description"));
        let description = ui.add(egui::TextEdit::multiline(&mut name).desired_rows(4).desired_width(f32::INFINITY));
        crate::widgets::name(&description, egui::Role::TextInput, "Navigation Pin description");
        if app.graph.pin_description_focus == Some(id) {
            description.request_focus();
            app.graph.pin_description_focus = None;
        }
        if description.changed() {
            app.run("graph.pin.edit", json!({"id": id, "description": name}));
        }
        if description.lost_focus() {
            app.session.finish_edit();
        }
    });
}
