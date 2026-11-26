//! Whether input is blocked: the shortcuts, the canvas keys, the floating panels, file drops and
//! `ui.menu.invoke` all ask [`Modality::capture`]
//!
//! Home and the Shortcuts list leave the app usable behind them, so they do not block

use crate::MaterialCraftApp;

/// The commands that show a file dialog: behind an open one they bring it forward
#[cfg(not(target_arch = "wasm32"))]
const SHOW_A_FILE_DIALOG: &[&str] = &["file.open", "file.saveAs", "file.saveCopyAs", "file.importBitmap", "file.importVector"];

/// What is holding input back
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Blocker {
    /// The unsaved-changes prompt, or the Save dialog it opened
    UnsavedPrompt,
    /// A native file dialog on screen or queued
    #[cfg(not(target_arch = "wasm32"))]
    FileDialog,
    /// The update prompt or the result window of a check
    #[cfg(not(target_arch = "wasm32"))]
    Update,
    /// One of the app's own dialogs
    Dialog,
    /// A window shown through egui's modal layer
    Modal,
}

impl Blocker {
    /// The English name the control channel reports
    pub(crate) fn name(self) -> &'static str {
        match self {
            Blocker::UnsavedPrompt => "the unsaved-changes prompt",
            #[cfg(not(target_arch = "wasm32"))]
            Blocker::FileDialog => "a file dialog",
            #[cfg(not(target_arch = "wasm32"))]
            Blocker::Update => "an update window",
            Blocker::Dialog => "a dialog",
            Blocker::Modal => "a modal window",
        }
    }

    /// The menu commands that still run behind this blocker
    fn allowed_commands(self) -> &'static [&'static str] {
        match self {
            #[cfg(not(target_arch = "wasm32"))]
            Blocker::FileDialog => SHOW_A_FILE_DIALOG,
            _ => &[],
        }
    }
}

/// The answer for one moment
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Modality {
    Free,
    Blocked(Blocker),
}

impl Modality {
    /// Reads the app's own state before egui's modal layer, which reports the previous frame. A
    /// file dialog is reported only when nothing else blocks input
    pub(crate) fn capture(app: &MaterialCraftApp, ctx: &egui::Context) -> Modality {
        match open_blocker(app) {
            Some(blocker) => Modality::Blocked(blocker),
            None if ctx.memory(|memory| memory.top_modal_layer()).is_some() => Modality::Blocked(Blocker::Modal),
            #[cfg(not(target_arch = "wasm32"))]
            None if app.dialogs.is_open() || app.dialogs.queued() > 0 => Modality::Blocked(Blocker::FileDialog),
            None => Modality::Free,
        }
    }

    pub(crate) fn is_blocked(self) -> bool {
        self != Modality::Free
    }

    /// Whether the menu command `id` can run now
    pub(crate) fn allows(self, id: &str) -> bool {
        match self {
            Modality::Free => true,
            Modality::Blocked(blocker) => blocker.allowed_commands().contains(&id),
        }
    }
}

fn open_blocker(app: &MaterialCraftApp) -> Option<Blocker> {
    let windows = &app.ui.windows;
    if windows.unsaved.is_some() || app.awaiting_save.is_some() {
        return Some(Blocker::UnsavedPrompt);
    }
    #[cfg(not(target_arch = "wasm32"))]
    if crate::updates_ui::blocks_input(app) {
        return Some(Blocker::Update);
    }
    #[cfg(any(test, target_arch = "wasm32"))]
    if windows.package_save.is_some() {
        return Some(Blocker::Dialog);
    }
    (app.expose_parameter_dialog.is_some() || windows.new_graph || windows.export || windows.preferences || windows.about || app.ui.explorer.rename.is_some())
        .then_some(Blocker::Dialog)
}

#[cfg(test)]
#[path = "modality.test.rs"]
mod tests;
