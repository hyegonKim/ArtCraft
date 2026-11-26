#[cfg(not(target_arch = "wasm32"))]
use crate::user_dir;
use crate::{ControlRequest, Toast, UiState, now_ms};
use crate::{control, cook, docking, fonts, function, graph, i18n, menus, pattern, state, theme, thumbs, toolbar, view2d, view3d, web_files, windows};
use materialcraft_engine::Session;
use serde_json::Value;
use std::collections::VecDeque;

pub struct MaterialCraftApp {
    pub session: Session,
    pub ui: UiState,
    pub control: Option<std::sync::mpsc::Receiver<ControlRequest>>,
    pub toasts: Vec<Toast>,
    /// Background cooking of the open graph
    pub cooker: cook::Cooker,
    /// Node output thumbnails and library previews
    pub thumbs: thumbs::Thumbs,
    /// The graph editor's gesture state
    pub graph: graph::GraphView,
    pub view3d: view3d::View3dState,
    pub view2d: view2d::View2dState,
    /// Embedded graph editors in the main graph panel
    pub pattern_editor: Option<pattern::PatternEditor>,
    pub function_editor: Option<function::FunctionEditor>,
    pub(crate) inspected_graph: Option<materialcraft_engine::GraphTarget>,
    pub(crate) inspected_folder: Option<(u64, String)>,
    pub(crate) folder_property_draft: Option<crate::properties::FolderPropertyDraft>,
    pub(crate) expose_parameter_dialog: Option<crate::properties::ExposeParameterDialog>,
    pub(crate) graph_property_draft: Option<crate::properties::GraphPropertyDraft>,
    pub(crate) graph_parents: Vec<graph::graph_parent::GraphParent>,
    /// Synthetic input from the control channel, fed to egui on the next frames
    pub input_queue: VecDeque<control::Scripted>,
    /// Replies waiting for the next window capture (`ui.screenshot`)
    pub pending_screenshots: Vec<std::sync::mpsc::Sender<Value>>,
    pub fps: f32,
    /// Smoothed CPU time spent building the UI per frame (ms)
    pub ui_ms: f32,
    /// The last status message (bottom left)
    pub status: String,
    pub(crate) pending_load: Option<(String, Vec<u8>)>,
    pub(crate) quit_confirmed: bool,
    last_time: f64,
    #[cfg_attr(target_arch = "wasm32", allow(dead_code))]
    next_autosave: f64,
    styled: Option<(theme::Brightness, theme::Accent)>,
    /// Release checks and the update prompt (desktop only)
    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) updates_rt: crate::updates_ui::Runtime,
    /// Native file dialogs (desktop only)
    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) dialogs: crate::native_dialogs::Dialogs,
    /// The held action waiting for the Save dialog the unsaved-changes prompt opened
    pub(crate) awaiting_save: Option<crate::unsaved::Guarded>,
    /// The window buttons to draw when the app draws the window frame; `None` while the desktop draws it (desktop only)
    #[cfg(not(target_arch = "wasm32"))]
    pub window_layout: Option<craft_window::Layout>,
    /// Keyboard shortcuts and their dispatch
    pub(crate) keymap: craft_keymap::Keymap,
    pub(crate) dispatcher: craft_keymap::egui::Dispatcher,
    /// What Copy or Cut put on the clipboard this frame, for the system clipboard
    pub(crate) system_clipboard: Option<String>,
    /// Fallback fonts for Arabic
    pub fonts: fonts::UiFonts,
    /// The icon font is installed (it takes effect on the frame after `set_fonts`)
    fonts_ready: bool,
    ui_gesture_active: bool,
}

impl MaterialCraftApp {
    pub fn new(cc: &eframe::CreationContext<'_>, session: Session) -> MaterialCraftApp {
        let ui = cc.storage.and_then(|s| eframe::get_value::<UiState>(s, "ui")).unwrap_or_else(|| {
            let mut ui = UiState::default();
            ui.prefs.language = i18n::os_language().into();
            ui
        });
        theme::install(&cc.egui_ctx);
        let mut app = MaterialCraftApp::headless(session, ui);
        i18n::set_language(&app.ui.prefs.language);
        app.fonts_ready = true;
        app.ui.windows.home = app.ui.prefs.show_home;
        #[cfg(not(target_arch = "wasm32"))]
        {
            app.dialogs = crate::native_dialogs::attached(&cc.egui_ctx);
        }
        app
    }

    /// An app without eframe (offscreen snapshots and tests): fonts are installed on the first frame, which draws nothing
    pub fn headless(session: Session, mut ui: UiState) -> MaterialCraftApp {
        ui.windows = state::Windows::default();
        ui.frame_all = true;
        let next_autosave = ui.prefs.autosave_minutes as f64 * 60.0;
        let mut app = MaterialCraftApp {
            session,
            ui,
            control: None,
            toasts: Vec::new(),
            cooker: cook::Cooker::default(),
            thumbs: thumbs::Thumbs::default(),
            graph: graph::GraphView::default(),
            view3d: view3d::View3dState::default(),
            view2d: view2d::View2dState::default(),
            function_editor: None,
            inspected_graph: None,
            inspected_folder: None,
            folder_property_draft: None,
            graph_property_draft: None,
            expose_parameter_dialog: None,
            graph_parents: Vec::new(),
            pattern_editor: None,
            input_queue: VecDeque::new(),
            pending_screenshots: Vec::new(),
            fps: 0.0,
            ui_ms: 0.0,
            status: String::new(),
            pending_load: None,
            quit_confirmed: false,
            last_time: 0.0,
            next_autosave,
            styled: None,
            #[cfg(not(target_arch = "wasm32"))]
            updates_rt: crate::updates_ui::Runtime::default(),
            #[cfg(not(target_arch = "wasm32"))]
            dialogs: crate::native_dialogs::unattached(),
            awaiting_save: None,
            #[cfg(not(target_arch = "wasm32"))]
            window_layout: None,
            keymap: menus::keymap(craft_keymap::Platform::current()),
            // The desktop sends Copy, Cut and Paste events in place of their key presses and the web sends both: the option lets them run their commands
            dispatcher: craft_keymap::egui::Dispatcher::new().with_repeat(true).with_clipboard_events(true),
            system_clipboard: None,
            fonts: fonts::UiFonts::default(),
            fonts_ready: false,
            ui_gesture_active: false,
        };
        i18n::set_language(&app.ui.prefs.language);
        app.apply_prefs();
        app
    }

    /// Push application preferences into the session
    pub fn apply_prefs(&mut self) {
        self.ui.prefs.history = self.ui.prefs.history.sanitized();
        if self.session.history_prefs != self.ui.prefs.history {
            let _ = materialcraft_engine::execute(&mut self.session, "session.history", &serde_json::json!(self.ui.prefs.history));
        }

        if self.session.camera != self.ui.prefs.camera.sanitized() {
            self.session.set_camera_prefs(self.ui.prefs.camera);
        }
        let p = if self.ui.prefs.preview_size == 0 { None } else { Some(self.ui.prefs.preview_size.clamp(64, 8192)) };
        if self.session.preview_size != p {
            let _ = materialcraft_engine::execute(&mut self.session, "view.previewSize", &serde_json::json!({"size": p.unwrap_or(0)}));
        }
    }

    /// Run an engine command, reporting errors as toasts. Returns the result on success
    pub fn run(&mut self, id: &str, mut params: Value) -> Option<Value> {
        if id.starts_with("graph.annotation.")
            || id.starts_with("graph.frame.")
            || id.starts_with("graph.comment.")
            || id.starts_with("graph.pin.")
            || id == "library.add"
        {
            if params.is_null() {
                params = serde_json::json!({});
            }
            if let (Some(parameters), Some(target)) = (params.as_object_mut(), self.graph.annotation_target.parameters().as_object()) {
                for (name, value) in target {
                    parameters.entry(name).or_insert_with(|| value.clone());
                }
            }
        }
        if id.starts_with("file.")
            || id.starts_with("package.")
            || matches!(id, "graph.open" | "graph.delete" | "graph.duplicate" | "folder.remove" | "explorer.paste" | "edit.undo" | "edit.redo")
        {
            crate::properties::GraphPropertyDraft::commit(self);
        }

        if matches!(id, "edit.undo" | "edit.redo") {
            if params.is_null() {
                params = serde_json::json!({});
            }
            if let Some(parameters) = params.as_object_mut() {
                parameters.entry("document").or_insert_with(|| Value::from(crate::history::document(self)));
            }
        }

        if matches!(
            id,
            "graph.node.add"
                | "graph.instance.add"
                | "graph.node.move"
                | "graph.frame.move"
                | "graph.wire.insertDot"
                | "edit.paste"
                | "edit.pasteWithoutLinks"
                | "edit.duplicate"
                | "edit.duplicateWithoutLinks"
        ) {
            if params.is_null() {
                params = serde_json::json!({});
            }

            if let Some(parameters) = params.as_object_mut() {
                parameters.entry("snapToGrid").or_insert(Value::Bool(self.ui.prefs.snap_to_grid));
            }
        }

        match cook::execute_guarded(&mut self.session, id, &params) {
            Ok(v) => {
                // The desktop sends no Paste event while the system clipboard is empty, so Copy and Cut fill it
                if matches!(id, "edit.copy" | "edit.cut")
                    && let Some(clip) = v.get("clip")
                {
                    self.system_clipboard = Some(clip.to_string());
                }
                if matches!(id, "graph.pin.add" | "library.add")
                    && let Some(materialcraft_engine::AnnotationSelection::Pin(pin)) = self.session.annotation_selection
                {
                    self.graph.pin_description_focus = Some(pin);
                    self.inspected_folder = None;
                    self.inspected_graph = None;
                    self.ui.dock.focus(docking::Panel::Properties);
                }
                if id == "view.previewSize" {
                    self.ui.prefs.preview_size = self.session.preview_size.unwrap_or(0);
                }
                if id == "session.history" {
                    self.ui.prefs.history = self.session.history_prefs;
                }
                self.close_subgraph_after(id);
                if id == "graph.node.function.edit"
                    && let Ok(target) = serde_json::from_value::<materialcraft_engine::FunctionGraphTarget>(v["target"].clone())
                {
                    let target = match target {
                        materialcraft_engine::FunctionGraphTarget::Base { node, parameter } => Some((node, function::FunctionTarget::Base(parameter))),
                        materialcraft_engine::FunctionGraphTarget::Dynamic { node, parameter } => Some((node, function::FunctionTarget::Dynamic(parameter))),
                        _ => None,
                    };
                    if let Some((node, target)) = target {
                        crate::graph::subgraph_navigation::open_function(self, node, target);
                    }
                }
                if matches!(id, "view.3d.assign" | "view.3d.outputs") {
                    self.ui.dock.focus(docking::Panel::View3d);
                }
                Some(v)
            }
            Err(e) => {
                self.toast(e.to_string(), true);
                None
            }
        }
    }

    pub(crate) fn close_subgraph_after(&mut self, command: &str) {
        if command.starts_with("file.new")
            || command.starts_with("package.")
            || matches!(command, "file.open" | "file.reload" | "graph.open" | "graph.delete" | "graph.duplicate" | "folder.remove" | "explorer.paste")
        {
            self.graph.inspected_node = None;
            self.inspected_graph = None;
            self.graph_property_draft = None;
            self.inspected_folder = None;
            self.folder_property_draft = None;
            self.graph_parents.clear();
            self.pattern_editor = None;
            self.function_editor = None;
            self.graph.selected_wires.clear();
            self.graph.cancel_gesture();
        }
        self.reconcile_editors();
        if (materialcraft_engine::selection_commands::supports(command) || matches!(command, "edit.undo" | "edit.redo"))
            && let Some(editor) = &mut self.function_editor
            && matches!(editor.target, function::FunctionTarget::Graph)
        {
            editor.selection = self.session.selection.iter().filter_map(|id| u32::try_from(id.0).ok()).collect();
        }
    }

    pub(crate) fn reconcile_editors(&mut self) {
        if self
            .inspected_folder
            .as_ref()
            .is_some_and(|(document, path)| !self.session.package(*document).is_ok_and(|package| package.folder_paths().contains(path)))
        {
            self.inspected_folder = None;
            self.folder_property_draft = None;
        }
        if self.inspected_graph.is_some_and(|target| target.get(&self.session).is_err()) {
            self.inspected_graph = None;
        }
        if self.function_editor.as_ref().is_some_and(|editor| editor.current(self).is_none()) {
            self.function_editor = None;
        }
        if self.pattern_editor.as_ref().is_some_and(|editor| editor.current(self).is_none()) {
            self.pattern_editor = None;
        }
    }

    pub fn toast(&mut self, text: impl Into<String>, error: bool) {
        let text = text.into();
        self.status = text.clone();
        self.session.log(text.clone());
        self.toasts.push(Toast { text, error, until: self.last_time + if error { 6.0 } else { 3.0 } });
    }

    /// Every `prefs.autosave_minutes`, write a modified package to the autosave folder
    #[cfg(not(target_arch = "wasm32"))]
    fn autosave(&mut self, now: f64) {
        let period = self.ui.prefs.autosave_minutes as f64 * 60.0;
        if period <= 0.0 || !self.session.modified {
            self.next_autosave = now + period;
            return;
        }

        if now < self.next_autosave {
            return;
        }

        self.next_autosave = now + period;
        let dir = user_dir("autosave");
        let path = dir.join(format!("{}.mcraft", materialcraft_engine::formats::safe_file_name(&self.session.pkg.name)));
        let pkg = self.session.pkg.clone();
        let spawned = std::thread::Builder::new().name("materialcraft-autosave".into()).spawn(move || {
            let result = std::fs::create_dir_all(&dir)
                .map_err(|e| e.to_string())
                .and_then(|_| {
                    materialcraft_engine::formats::project::ProjectFiles::build(&pkg, "autosave.mcraft", true)
                        .and_then(|files| files.zip())
                        .map_err(|e| e.to_string())
                })
                .and_then(|bytes| materialcraft_engine::formats::write_atomic(&path, &bytes).map_err(|e| e.to_string()));
            match result {
                Ok(()) => log::info!("autosaved {}", path.display()),
                Err(e) => log::warn!("autosave to {} failed: {e}", path.display()),
            }
        });
        if let Err(e) = spawned {
            log::warn!("could not start autosave: {e}");
        }
    }
}

impl eframe::App for MaterialCraftApp {
    fn auto_save_interval(&self) -> std::time::Duration {
        std::time::Duration::from_secs(if cfg!(target_arch = "wasm32") { 5 } else { 30 })
    }

    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, "ui", &self.ui);
    }

    fn raw_input_hook(&mut self, _ctx: &egui::Context, raw: &mut egui::RawInput) {
        control::feed_input(self, raw);
    }

    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        #[cfg(not(target_arch = "wasm32"))]
        self.step_then(ctx, craft_quit::on_close_request);
        #[cfg(target_arch = "wasm32")]
        self.step(ctx);
    }

    fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        self.draw(ui);
        #[cfg(not(target_arch = "wasm32"))]
        self.dialogs.show_pending(Some(&*frame));
        #[cfg(target_arch = "wasm32")]
        let _ = frame;
    }
}

impl MaterialCraftApp {
    /// [`Self::step`], then the platform's close handling, which sees a close `step` canceled
    #[cfg(not(target_arch = "wasm32"))]
    fn step_then(&mut self, ctx: &egui::Context, close: impl FnOnce(&egui::Context) -> bool) {
        self.step(ctx);
        close(ctx);
    }

    /// Per-frame logic before drawing: styles, the control channel, cooking, shortcuts
    pub fn step(&mut self, ctx: &egui::Context) {
        if !self.fonts_ready {
            theme::install(ctx);
            ctx.request_repaint();
            return;
        }
        i18n::set_language(&self.ui.prefs.language);
        self.fonts.update(ctx, &self.ui.prefs.language, self.ui.windows.preferences);
        let now = ctx.input(|i| i.time);
        let dt = (now - self.last_time) as f32;
        if dt > 0.0 {
            self.fps = self.fps * 0.9 + (1.0 / dt) * 0.1;
        }
        self.last_time = now;
        let style = (self.ui.prefs.brightness, self.ui.prefs.accent);
        if self.styled != Some(style) {
            theme::apply(ctx, style.0, style.1);
            self.styled = Some(style);
        }
        let scale = self.ui.prefs.ui_scale.clamp(0.5, 3.0);
        if (ctx.zoom_factor() - scale).abs() > 1e-3 && !ctx.input(|input| input.pointer.any_down()) {
            ctx.set_zoom_factor(scale);
        }
        if ctx.input(|input| input.pointer.any_pressed()) {
            self.session.finish_edit();
        }
        crate::unsaved::handle_close(self, ctx);
        control::poll(self, ctx);
        web_files::poll(self);
        #[cfg(not(target_arch = "wasm32"))]
        crate::native_dialogs::poll(self);
        if !crate::shell::modality::Modality::capture(self, ctx).is_blocked() {
            for f in ctx.input(|i| i.raw.dropped_files.clone()) {
                web_files::dropped(self, ctx, &f);
            }
        }
        self.apply_prefs();
        cook::drive(self, ctx);
        #[cfg(not(target_arch = "wasm32"))]
        self.autosave(now);
        #[cfg(not(target_arch = "wasm32"))]
        crate::updates_ui::tick(self, ctx);
        self.reconcile_editors();
        menus::shortcuts(self, ctx);
        self.publish_clipboard(ctx);
        self.toasts.retain(|t| t.until > now);
        if !self.input_queue.is_empty() {
            ctx.request_repaint();
        }
    }

    /// Hand what Copy or Cut put on the clipboard to the system clipboard
    fn publish_clipboard(&mut self, ctx: &egui::Context) {
        if let Some(text) = self.system_clipboard.take() {
            ctx.copy_text(text);
        }
    }

    /// Draw the whole window into `ui`
    pub fn draw(&mut self, ui: &mut egui::Ui) {
        crate::annotation::activate(self, crate::annotation::target(self));
        if !self.fonts_ready {
            self.fonts_ready = true;
            return;
        }
        let ctx = ui.ctx().clone();
        let t0 = now_ms();
        // Tab can focus a menu or toolbar button while drawing; preserve keyboard ownership
        self.graph.keyboard_was_focused = ctx.egui_wants_keyboard_input();
        menus::menu_bar(self, ui);
        toolbar::main_toolbar(self, ui);
        toolbar::status_bar(self, ui);
        self.ui.explorer.hovered = false;
        egui::CentralPanel::default().frame(egui::Frame::NONE.fill(theme::Tokens::get(&ctx).chrome)).show(ui, |ui| docking::show(self, ui));
        windows::show(self, &ctx);
        #[cfg(not(target_arch = "wasm32"))]
        if self.window_layout.is_some() {
            crate::window_controls::resize_zones(ui);
        }
        let gesture_active = ctx.input(|input| input.pointer.any_down()) || ctx.egui_wants_keyboard_input();
        if !gesture_active && (self.ui_gesture_active || ctx.input(|input| input.pointer.any_released())) {
            self.session.finish_edit();
        }
        self.ui_gesture_active = gesture_active;
        self.publish_clipboard(&ctx);
        crate::toast::show(self, &ctx);
        control::request_screenshots(self, &ctx);
        let dt = (now_ms() - t0) as f32;
        self.ui_ms = if self.ui_ms == 0.0 { dt } else { self.ui_ms * 0.9 + dt * 0.1 };
    }
}

#[cfg(test)]
#[path = "material_craft_app.test.rs"]
mod tests;
