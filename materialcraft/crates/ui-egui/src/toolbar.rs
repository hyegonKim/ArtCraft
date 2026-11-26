//! The main toolbar under the menu bar and the status bar at the bottom of the window.

use egui::{RichText, Ui};
use egui_phosphor::regular as icon;

use crate::MaterialCraftApp;
use crate::i18n::tr;
use crate::theme::Tokens;

pub fn main_toolbar(app: &mut MaterialCraftApp, ui: &mut Ui) {
    let tk = Tokens::get(ui.ctx());
    let ctx = ui.ctx().clone();
    egui::Panel::top("main_toolbar").exact_size(30.0).frame(egui::Frame::NONE.fill(tk.chrome).inner_margin(egui::Margin::symmetric(6, 2))).show(ui, |ui| {
        ui.horizontal_centered(|ui| {
            ui.spacing_mut().item_spacing.x = 2.0;
            let (u, r) = (crate::menus::enabled(app, "edit.undo"), crate::menus::enabled(app, "edit.redo"));
            let modality = crate::shell::modality::Modality::capture(app, &ctx);
            let mut b = |ui: &mut Ui, ic: &str, tip: &str, id: &str, on: bool| {
                let r = ui.add_enabled_ui(on && modality.allows(id), |ui| crate::widgets::icon_button(ui, ic, tip, false, 24.0)).inner;
                if r.clicked() {
                    crate::menus::invoke(app, &ctx, id);
                }
            };
            b(ui, icon::FILE_PLUS, tr("New graph... (Ctrl+N)"), "ui.newGraph", true);
            b(ui, icon::FOLDER_OPEN, tr("Open package... (Ctrl+O)"), "file.open", true);
            b(ui, icon::FLOPPY_DISK, tr("Save (Ctrl+S)"), "file.save", true);
            ui.add_space(6.0);
            b(ui, icon::ARROW_U_UP_LEFT, tr("Undo (Ctrl+Z)"), "edit.undo", u);
            b(ui, icon::ARROW_U_UP_RIGHT, tr("Redo (Ctrl+Y)"), "edit.redo", r);
            ui.add_space(6.0);
            b(ui, icon::EXPORT, tr("Export outputs... (Ctrl+Shift+E)"), "ui.export", true);
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(RichText::new("MaterialCraft").strong().color(tk.accent));
            });
        });
    });
}

pub fn status_bar(app: &mut MaterialCraftApp, ui: &mut Ui) {
    let tk = Tokens::get(ui.ctx());
    egui::Panel::bottom("status_bar").exact_size(22.0).frame(egui::Frame::NONE.fill(tk.chrome).inner_margin(egui::Margin::symmetric(8, 0))).show(ui, |ui| {
        ui.horizontal_centered(|ui| {
            let busy = app.cooker.busy();
            if busy {
                ui.add(egui::Spinner::new().size(12.0));
                ui.label(RichText::new(tr("Computing...")).size(11.0).color(tk.text_dim));
            } else if !app.status.is_empty() {
                ui.label(RichText::new(&app.status).size(11.0).color(tk.text_dim));
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let cache = app.session.cache.lock().map(|c| c.bytes()).unwrap_or(0);
                let threads = std::thread::available_parallelism().map_or(1, |n| n.get());
                let preview = app.session.preview_size.map_or_else(|| tr("full size").to_string(), |p| format!("{p} px"));
                let text = format!(
                    "{}: CPU ({threads} {})  |  {}: {:.0} ms  |  {}: {preview}  |  {}: {} MB  |  {} {}",
                    tr("Engine"),
                    tr("threads"),
                    tr("Last compute"),
                    app.session.cooked.ms,
                    tr("Preview"),
                    tr("Cache"),
                    cache >> 20,
                    tr("Version"),
                    materialcraft_engine::VERSION,
                );
                ui.label(RichText::new(text).size(11.0).color(tk.text_dim));
            });
        });
    });
}

#[cfg(test)]
mod tests {
    use egui_kittest::kittest::{By, NodeT, Queryable};

    use crate::{MaterialCraftApp, state::UiState};

    fn harness(block: impl FnOnce(&mut MaterialCraftApp)) -> egui_kittest::Harness<'static, MaterialCraftApp> {
        let mut app = MaterialCraftApp::headless(materialcraft_engine::sample_session(), UiState::default());
        block(&mut app);
        let mut fonts_installed = false;
        let mut harness = egui_kittest::Harness::builder().with_size(egui::vec2(800.0, 100.0)).build_ui_state(
            move |ui, app: &mut MaterialCraftApp| {
                if std::mem::replace(&mut fonts_installed, true) {
                    super::main_toolbar(app, ui);
                } else {
                    crate::theme::install(ui.ctx());
                }
            },
            app,
        );
        harness.run_steps(3);
        harness
    }

    // Each button's position in the toolbar. The labels change with the UI language.
    const NEW_GRAPH: usize = 0;
    const OPEN: usize = 1;
    const SAVE: usize = 2;
    const EXPORT: usize = 5;

    fn off(harness: &egui_kittest::Harness<'static, MaterialCraftApp>, button: usize) -> bool {
        harness.get_all(By::new().role(egui::accesskit::Role::Button)).nth(button).expect("the toolbar has this button").accesskit_node().is_disabled()
    }

    #[test]
    fn toolbar_buttons_match_the_commands_a_blocker_allows() {
        let free = harness(|_| {});
        assert!(!off(&free, NEW_GRAPH) && !off(&free, OPEN) && !off(&free, SAVE) && !off(&free, EXPORT));

        let dialog = harness(|app| app.ui.windows.about = true);
        assert!(off(&dialog, NEW_GRAPH) && off(&dialog, OPEN) && off(&dialog, SAVE) && off(&dialog, EXPORT));

        let file_dialog = harness(|app| app.dialogs.ask(craft_dialogs::FileDialog::open(), crate::native_dialogs::Then::OpenPackage));
        assert!(!off(&file_dialog, OPEN), "a file dialog leaves Open enabled, which brings it forward");
        assert!(off(&file_dialog, SAVE));
    }
}
