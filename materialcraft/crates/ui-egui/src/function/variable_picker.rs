use egui::{TextEdit, Ui};
use materialcraft_engine::FunctionVariable;

use crate::i18n::tr;

pub(super) fn show(ui: &mut Ui, id: u32, name: &mut String, variables: &[FunctionVariable]) -> bool {
    let mut changed = false;
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 0.0;
        let width = (ui.available_width() - ui.spacing().interact_size.y).max(40.0);
        let field = ui.add(TextEdit::singleline(name).id_salt(("function_variable", id)).hint_text(tr("Variable name")).desired_width(width));
        crate::widgets::name(&field, egui::Role::TextInput, tr("Variable name"));
        changed |= field.changed();

        let button = ui.button(egui_phosphor::regular::CARET_DOWN);
        crate::widgets::name(&button, egui::Role::Button, tr("Choose variable"));
        let bounds = field.rect.union(button.rect);
        egui::Popup::menu(&button).anchor(bounds).width(bounds.width()).show(|ui| {
            ui.set_min_width(ui.available_width());
            egui::ScrollArea::vertical().max_height(240.0).auto_shrink([false, true]).show(ui, |ui| {
                if variables.is_empty() {
                    ui.label(tr("No suggested variables"));
                }

                for variable in variables {
                    if ui.selectable_label(*name == variable.name, &variable.name).clicked() {
                        changed |= *name != variable.name;
                        name.clone_from(&variable.name);
                        ui.close();
                    }
                }
            });
        });
        button.on_hover_text(tr("Choose variable"));
    });
    changed
}
