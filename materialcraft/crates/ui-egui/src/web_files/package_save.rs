use crate::MaterialCraftApp;
use crate::i18n::tr;
use crate::unsaved::Guarded;

/// A browser save request retains its package while other documents are opened or closed
#[derive(Clone, Debug, PartialEq)]
pub struct PackageSave {
    document: u64,
    name: String,
    archive: bool,
    copy: bool,
    before_action: bool,
}

impl PackageSave {
    pub(crate) fn request(app: &mut MaterialCraftApp, copy: bool) {
        // Replacing this dialog would let an unrelated save release the held close action
        if app.awaiting_save.is_some() {
            return;
        }

        let archive = app.session.path.as_deref().is_some_and(materialcraft_engine::file_cmds::is_mcraft);
        app.ui.windows.package_save = Some(Self { document: app.session.document_id, name: app.session.pkg.name.clone(), archive, copy, before_action: false });
    }

    pub(crate) fn save(app: &mut MaterialCraftApp, download: impl FnOnce(&str, &[u8]) -> Result<(), String>) {
        if let Some(path) = app.session.path.clone() {
            Self::download(app, &path, false, download);
        } else {
            Self::request(app, false);
        }
    }

    pub(crate) fn save_then(app: &mut MaterialCraftApp, action: Guarded, download: impl FnOnce(&str, &[u8]) -> Result<(), String>) {
        if let Some(path) = app.session.path.clone() {
            app.awaiting_save = Some(action);
            let saved = Self::download(app, &path, false, download);
            crate::unsaved::finish_save_dialog(app, saved);
        } else {
            Self::request(app, false);
            if let Some(request) = &mut app.ui.windows.package_save {
                request.before_action = true;
                app.awaiting_save = Some(action);
            }
        }
    }

    pub(crate) fn show(app: &mut MaterialCraftApp, ctx: &egui::Context, download: impl FnOnce(&str, &[u8]) -> Result<(), String>) {
        let Some(mut request) = app.ui.windows.package_save.clone() else { return };

        let mut open = true;
        let mut accept = false;
        let mut cancel = false;
        egui::Window::new(tr("Save package")).collapsible(false).resizable(false).open(&mut open).show(ctx, |ui| {
            let label = ui.label(tr("Name"));
            ui.text_edit_singleline(&mut request.name).labelled_by(label.id);
            ui.radio_value(&mut request.archive, false, tr("SBS XML Package (*.sbs)"));
            ui.radio_value(&mut request.archive, true, tr("MaterialCraft Project (*.mcraft)"));
            if !request.archive {
                ui.label(tr("Downloads the SBS and resources together in a ZIP."));
            }
            ui.horizontal(|ui| {
                accept = ui.button(tr("Download")).clicked();
                cancel = ui.button(tr("Cancel")).clicked();
            });
        });
        app.ui.windows.package_save = Some(request);
        let dismissed = !open || cancel || ctx.input(|input| input.key_pressed(egui::Key::Escape));
        if accept || dismissed {
            Self::finish(app, accept && !dismissed, download);
        }
    }

    fn finish(app: &mut MaterialCraftApp, accept: bool, download: impl FnOnce(&str, &[u8]) -> Result<(), String>) {
        let Some(request) = app.ui.windows.package_save.take() else { return };

        let saved = if accept && app.run("package.activate", serde_json::json!({"document": request.document})).is_some() {
            let extension = if request.archive { "mcraft" } else { "sbs" };
            let path = format!("{}.{extension}", materialcraft_engine::formats::safe_file_name(&request.name));
            Self::download(app, &path, request.copy, download)
        } else {
            false
        };

        if request.before_action {
            crate::unsaved::finish_save_dialog(app, saved);
        }
    }

    fn download(app: &mut MaterialCraftApp, path: &str, copy: bool, download: impl FnOnce(&str, &[u8]) -> Result<(), String>) -> bool {
        crate::properties::GraphPropertyDraft::commit(app);
        let archive = materialcraft_engine::file_cmds::is_mcraft(path);
        let name = if archive { path.to_string() } else { format!("{}_sbs.zip", path.strip_suffix(".sbs").unwrap_or(path)) };
        let result = materialcraft_engine::file_cmds::save_files_at(&app.session, path)
            .map_err(|error| error.to_string())
            .and_then(|files| files.zip().map_err(|error| error.to_string()))
            .and_then(|bytes| download(&name, &bytes));

        match result {
            Ok(()) => {
                if !copy {
                    app.session.path = Some(path.to_string());
                    app.session.modified = false;
                }
                app.toast(format!("{} {name}", tr("Downloaded")), false);
                true
            }
            Err(error) => {
                app.toast(format!("{} {name}: {error}", tr("Couldn't save")), true);
                false
            }
        }
    }
}

#[cfg(test)]
#[path = "package_save.test.rs"]
mod tests;
