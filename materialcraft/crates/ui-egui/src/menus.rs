//! Menu actions and shortcuts share stable command ids

use craft_keymap::{Active, Bindable, Binding, Chord, Keymap, Platform, Scope, Scopes};
use egui::class::HasClasses as _;
use serde_json::Value;

use crate::MaterialCraftApp;
use crate::i18n::tr;
use crate::shell::modality::Modality;
use crate::theme::Tokens;

pub use crate::menu_item::MenuItem;

const fn it(menu: &'static str, label: &'static str, id: &'static str, shortcut: Option<&'static str>) -> MenuItem {
    MenuItem { menu, sub: None, label, id, shortcut }
}

const fn sub(menu: &'static str, sub: &'static str, label: &'static str, id: &'static str, shortcut: Option<&'static str>) -> MenuItem {
    MenuItem { menu, sub: Some(sub), label, id, shortcut }
}

const fn sep(menu: &'static str) -> MenuItem {
    MenuItem { menu, sub: None, label: "-", id: "", shortcut: None }
}

pub const TOP: [&str; 5] = ["File", "Edit", "Tools", "Windows", "Help"];

pub(crate) const EXPLORER_NEW: &[(&str, &str)] = &[("Graph", "ui.newGraph"), ("Function Graph", "ui.newFunctionGraph"), ("Folder", "folder.create")];
pub(crate) const EXPLORER_IMPORT: &[(&str, &str)] = &[("Bitmap", "file.importBitmap"), ("Vector graphics", "file.importVector")];
pub(crate) const EXPLORER_RESOURCE: &[(&str, &str)] = &[("Add to graph", "graph.resource.add")];
pub(crate) use crate::explorer::explorer_row::ExplorerRow;
use ExplorerRow::{Item, Separator, Sub};

/// Linking keeps a resource outside the package; MaterialCraft only embeds resources so far
pub(crate) const EXPLORER_LINK: &[(&str, &str)] = &[("Bitmap", ""), ("Vector graphics", "")];
pub(crate) const EXPLORER_SEND_TO: &[(&str, &str)] = &[("TextureCraft", "file.sendToTextureCraft")];
/// Explorer actions that wrap engine commands or the operating system: computing every output,
/// showing the package file and opening the packages it depends on
pub(crate) const EXPLORER_COMPUTE: &str = "ui.explorer.computeOutputs";
pub(crate) const EXPLORER_SHOW_IN_FOLDER: &str = "ui.explorer.showInFolder";
pub(crate) const EXPLORER_OPEN_DEPENDENCIES: &str = "ui.explorer.openDependencies";
pub(crate) const EXPLORER_VIEW_OUTPUTS: &str = "ui.explorer.viewOutputs";
pub(crate) const EXPLORER_EXPORT_OUTPUTS: &str = "ui.explorer.exportOutputs";
pub(crate) const EXPLORER_EXPORT_MATERIAL: &str = "ui.explorer.exportMaterial";
pub(crate) const EXPLORER_PACKAGE: &[ExplorerRow] = &[
    Sub("New", EXPLORER_NEW),
    Sub("Import", EXPLORER_IMPORT),
    Sub("Link", EXPLORER_LINK),
    Separator,
    Item("Reload", "file.reload", None),
    Item("Save", "file.save", Some("Ctrl+S")),
    Item("Save as...", "file.saveAs", Some("Ctrl+Shift+S")),
    Item("Save a copy as...", "file.saveCopyAs", None),
    Separator,
    Item("Export material...", "file.exportMaterial", None),
    Item("Export with dependencies...", "", None),
    Sub("Send to", EXPLORER_SEND_TO),
    Separator,
    Item("Copy", "explorer.copy", None),
    Item("Paste", "explorer.paste", None),
    Item("Close package", "package.close", None),
    Separator,
    Item("Compute outputs", EXPLORER_COMPUTE, None),
    Item("Show in folder...", EXPLORER_SHOW_IN_FOLDER, None),
    Item("Dependency manager...", "", None),
    Item("Open dependencies", EXPLORER_OPEN_DEPENDENCIES, None),
];
pub(crate) const EXPLORER_FOLDER: &[ExplorerRow] = &[
    Item("Copy", "explorer.copy", None),
    Item("Paste", "explorer.paste", None),
    Item("Remove", "folder.remove", Some("Delete")),
    Item("Rename", "folder.rename", Some("F2")),
];
pub(crate) const EXPLORER_GRAPH: &[ExplorerRow] = &[
    Item("Open", "graph.open", Some("Enter")),
    Item("Copy", "explorer.copy", None),
    Item("Paste", "explorer.paste", None),
    Item("Remove", "graph.delete", Some("Delete")),
    Item("Rename", "graph.rename", Some("F2")),
    Separator,
    Item("View outputs in 3D view", EXPLORER_VIEW_OUTPUTS, None),
    Item("Compute outputs", EXPLORER_COMPUTE, None),
    Item("Export outputs...", EXPLORER_EXPORT_OUTPUTS, None),
    Item("Export material...", EXPLORER_EXPORT_MATERIAL, None),
];

/// The menu catalog (the target menu tree with our ids)
pub const CATALOG: &[MenuItem] = &[
    sub("File", "New", "Graph...", "ui.newGraph", Some("Ctrl+N")),
    sub("File", "New", "Function Graph...", "ui.newFunctionGraph", None),
    sub("File", "New", "Empty package", "file.newPackage", None),
    it("File", "Open...", "file.open", Some("Ctrl+O")),
    sep("File"),
    it("File", "Save", "file.save", Some("Ctrl+S")),
    it("File", "Save as...", "file.saveAs", Some("Ctrl+Shift+S")),
    it("File", "Close all", "ui.closeAll", None),
    sep("File"),
    it("File", "Import bitmap...", "file.importBitmap", None),
    it("File", "Import vector graphics...", "file.importVector", None),
    it("File", "Export outputs...", "ui.export", Some("Ctrl+Shift+E")),
    it("File", "Export material...", "file.exportMaterial", None),
    it("File", "Send to TextureCraft", "file.sendToTextureCraft", None),
    it("File", "Reload resources", "ui.reloadResources", None),
    sep("File"),
    it("File", "Exit", "ui.quit", Some("Ctrl+Q")),
    it("Edit", "Undo", "edit.undo", Some("Ctrl+Z")),
    it("Edit", "Redo", "edit.redo", Some("Ctrl+Y")),
    sep("Edit"),
    it("Edit", "Copy", "edit.copy", Some("Ctrl+C")),
    it("Edit", "Cut", "edit.cut", Some("Ctrl+X")),
    it("Edit", "Paste", "edit.paste", Some("Ctrl+V")),
    it("Edit", "Duplicate", "edit.duplicate", Some("Ctrl+D")),
    it("Edit", "Delete", "edit.delete", Some("Delete")),
    it("Edit", "Delete and relink", "edit.deleteRelink", Some("Backspace")),
    it("Edit", "Select all", "edit.selectAll", Some("Ctrl+A")),
    it("Edit", "Edit node graph", "graph.node.edit", Some("Ctrl+E")),
    it("Edit", "Parent graph", "ui.graph.parent", Some("Alt+ArrowLeft")),
    sep("Edit"),
    it("Edit", "Preferences...", "ui.preferences", None),
    it("Tools", "Compute graph", "graph.cook", None),
    it("Tools", "Snap to grid", "ui.graph.snapToGrid", None),
    it("Tools", "Compute nodes thumbnails", "ui.graph.computeThumbnails", None),
    it("Tools", "Clear nodes thumbnails", "ui.graph.clearThumbnails", Some("F5")),
    it("Tools", "Previous pin", "ui.graph.previousPin", Some("Shift+F2")),
    it("Tools", "Next pin", "ui.graph.nextPin", Some("F2")),
    it("Tools", "View outputs in 3D view", "ui.graph.viewOutputs", None),
    it("Tools", "Reset and view outputs in 3D view", "ui.graph.resetViewOutputs", None),
    it("Tools", "Clear cache", "ui.clearCache", None),
    sub("Tools", "Preview size", "256 px", "ui.preview.256", None),
    sub("Tools", "Preview size", "512 px", "ui.preview.512", None),
    sub("Tools", "Preview size", "1024 px", "ui.preview.1024", None),
    sub("Tools", "Preview size", "2048 px", "ui.preview.2048", None),
    sub("Tools", "Preview size", "Full size", "ui.preview.0", None),
    it("Tools", "Create graph from selection", "graph.createFromSelection", None),
    it("Windows", "Explorer", "ui.view.explorer", None),
    it("Windows", "Graph", "ui.view.graph", None),
    it("Windows", "Properties", "ui.view.properties", None),
    it("Windows", "Console", "ui.view.console", None),
    it("Windows", "3D View", "ui.view.3d", None),
    it("Windows", "2D View", "ui.view.2d", None),
    it("Windows", "Library", "ui.view.library", None),
    it("Windows", "History", "ui.view.history", None),
    sep("Windows"),
    it("Windows", "Reset layout", "ui.resetLayout", None),
    it("Help", "Home screen...", "ui.home", None),
    it("Help", "Documentation...", "ui.help.docs", None),
    it("Help", "Shortcuts list...", "ui.help.shortcuts", None),
    sep("Help"),
    it("Help", "Join the community (Discord)...", "ui.help.discord", None),
    it("Help", "ArtCraft website...", "ui.help.website", None),
    it("Help", "Report a bug...", "ui.help.bug", None),
    sep("Help"),
    #[cfg(not(target_arch = "wasm32"))]
    it("Help", "Check for updates...", "ui.help.checkForUpdates", None),
    it("Help", "About MaterialCraft...", "ui.about", None),
];

fn is_ui(id: &str) -> bool {
    id.starts_with("ui.")
}

/// Whether an item can run now
pub fn enabled(app: &MaterialCraftApp, id: &str) -> bool {
    if let Some(enabled) = crate::history::enabled(app, id) {
        return enabled;
    }

    if let Some(enabled) = crate::graph::editor_actions::enabled(app, id) {
        return enabled;
    }

    if let Some(enabled) = crate::graph::actions::enabled(app, id) {
        return enabled;
    }

    if matches!(id, "edit.paste" | "edit.pasteWithoutLinks") {
        return app.session.graph().is_ok() && app.session.clipboard.as_ref().is_some_and(|clip| !clip.nodes.is_empty());
    }

    if is_ui(id) {
        return true;
    }
    materialcraft_engine::find(id).is_some_and(|c| (c.enabled)(&app.session).is_ok())
}

/// Whether an id is implemented at all (parity report)
pub fn implemented(id: &str) -> bool {
    is_ui(id) || materialcraft_engine::find(id).is_some()
}

/// Invoke a menu item (menus, shortcuts, the control channel's `ui.menu.invoke`), unless a dialog or prompt blocks it
pub fn invoke(app: &mut MaterialCraftApp, ctx: &egui::Context, id: &str) {
    if !Modality::capture(app, ctx).allows(id) {
        return;
    }

    if crate::graph::editor_actions::invoke(app, id) {
        return;
    }

    if crate::graph::actions::invoke(app, id) {
        return;
    }

    if let Some(panel) = crate::docking::view_panel(id) {
        app.ui.dock.toggle(panel);
        return;
    }
    if let Some(size) = id.strip_prefix("ui.preview.").and_then(|s| s.parse::<u32>().ok()) {
        app.ui.prefs.preview_size = size;
        app.apply_prefs();
        return;
    }
    match id {
        "ui.newGraph" | "ui.newFunctionGraph" => {
            app.ui.new_graph_kind =
                if id == "ui.newFunctionGraph" { materialcraft_engine::core::GraphKind::Function } else { materialcraft_engine::core::GraphKind::Material };
            app.ui.windows.new_graph = true;
        }
        "ui.export" => crate::export::open(app, crate::export::ExportTab::FromGraph),
        "ui.quit" => ctx.send_viewport_cmd(egui::ViewportCommand::Close),
        "ui.preferences" => app.ui.windows.preferences = true,
        "ui.view.graph" => {}
        "ui.resetLayout" => app.ui.dock = crate::docking::Dock::default(),
        "ui.closeAll" => crate::unsaved::guard(app, crate::unsaved::Guarded::Menu { id: id.to_string() }),
        "file.newPackage" => {
            app.run("file.newPackage", serde_json::json!({}));
        }
        "ui.reloadResources" | "ui.clearCache" => {
            if let Ok(mut c) = app.session.cache.lock() {
                c.clear();
            }
            app.session.dirty = true;
        }
        "ui.home" => app.ui.windows.home = true,
        "ui.help.shortcuts" => app.ui.windows.shortcuts = true,
        "ui.about" => app.ui.windows.about = true,
        #[cfg(not(target_arch = "wasm32"))]
        "ui.help.checkForUpdates" => crate::updates_ui::check(app),
        "ui.help.discord" => ctx.open_url(egui::OpenUrl::new_tab("https://discord.gg/artcraft")),
        "ui.help.website" => ctx.open_url(egui::OpenUrl::new_tab("https://getartcraft.com/apps/materialcraft")),
        "ui.help.docs" => ctx.open_url(egui::OpenUrl::new_tab("https://github.com/storytold/materialcraft#readme")),
        "ui.help.bug" => ctx.open_url(egui::OpenUrl::new_tab("https://github.com/storytold/materialcraft/issues")),
        _ => {
            if crate::windows::file_action(app, ctx, id) {
                return;
            }
            app.run(id, Value::Null);
        }
    }
}

/// Parse "Ctrl+Shift+E" style shortcuts; `Ctrl` in these texts is Command on macOS
pub fn parse_shortcut(s: &str) -> Option<Chord> {
    Chord::parse(&s.replace("Ctrl", "Cmd")).ok()
}

/// The text a shortcut shows on this platform: "Cmd+Shift+E" on macOS, where the interface fonts lack the ⇧ and ⌘ glyphs, and "Ctrl+Shift+E" elsewhere
pub fn shortcut_label(ctx: &egui::Context, text: &str) -> String {
    let platform = craft_keymap::egui::platform(ctx);
    parse_shortcut(text).map_or_else(|| text.to_string(), |chord| if platform.is_mac() { chord.to_string() } else { chord.display(platform) })
}

/// Every shortcut in effect: the catalog's, then the engine commands' not listed there
pub(crate) fn shortcut_table() -> Vec<(&'static str, &'static str)> {
    let mut table: Vec<(&'static str, &'static str)> = CATALOG.iter().filter_map(|i| Some((i.id, i.shortcut?))).collect();
    for c in materialcraft_engine::command_specs() {
        if table.iter().any(|(id, _)| *id == c.id) {
            continue;
        }
        if let Some(shortcut) = c.shortcut {
            table.push((c.id, shortcut));
        }
    }
    table
}

/// The keymap the table's shortcuts dispatch through
pub(crate) fn keymap(platform: Platform) -> Keymap {
    let catalog = shortcut_table()
        .into_iter()
        .filter_map(|(id, text)| Some(Bindable::new(id, id, Scope::app()).with_default(Binding::new(parse_shortcut(text)?))))
        .collect();
    Keymap::new(catalog, Scopes::new(), platform)
}

/// The shortcut text shown for `id` (the catalog's, else the engine command's)
pub fn shortcut_of(id: &str) -> Option<&'static str> {
    CATALOG.iter().find(|i| i.id == id).and_then(|i| i.shortcut).or_else(|| materialcraft_engine::find(id).and_then(|c| c.shortcut))
}

/// Draw menu bars with aligned labels and a separate gutter for checkmarks.
pub fn menu_bar(app: &mut MaterialCraftApp, ui: &mut egui::Ui) {
    let tokens = Tokens::get(ui.ctx());
    egui::Panel::top("menu_bar").exact_size(24.0).frame(egui::Frame::NONE.fill(tokens.chrome).inner_margin(egui::Margin::symmetric(6, 0))).show(ui, |ui| {
        #[cfg(not(target_arch = "wasm32"))]
        let window_layout = app.window_layout.clone();
        #[cfg(not(target_arch = "wasm32"))]
        if window_layout.is_some() {
            craft_window::title_bar::empty_area(ui, ui.max_rect());
        }

        egui::MenuBar::new().ui(ui, |ui| {
            let context = ui.ctx().clone();
            #[cfg(not(target_arch = "wasm32"))]
            if let Some(layout) = &window_layout {
                crate::window_controls::buttons(ui, &context, &layout.left);
            }
            for top in TOP {
                ui.menu_button(tr(top), |ui| {
                    ui.set_min_width(220.0);
                    let mut current_submenu: Option<&str> = None;
                    let items: Vec<MenuItem> = CATALOG.iter().filter(|item| item.menu == top).copied().collect();
                    for item in &items {
                        if let Some(submenu) = item.sub {
                            if current_submenu == Some(submenu) {
                                continue;
                            }

                            current_submenu = Some(submenu);
                            ui.menu_button(menu_label(tr(submenu)), |ui| {
                                for entry in items.iter().filter(|entry| entry.sub == Some(submenu)) {
                                    menu_item(app, &context, ui, entry);
                                }
                            });
                            continue;
                        }

                        if item.label == "-" {
                            ui.separator();
                            continue;
                        }

                        menu_item(app, &context, ui, item);
                    }
                });
            }
            #[cfg(not(target_arch = "wasm32"))]
            if let Some(layout) = window_layout.as_ref().filter(|layout| !layout.right.is_empty()) {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let mut right = layout.right.clone();
                    right.reverse();
                    crate::window_controls::buttons(ui, &context, &right);
                });
            }
        });
    });
}

/// Reserve the same checkmark gutter for commands, toggles and submenus.
fn menu_label(label: &str) -> (egui::Atom<'_>, &str) {
    (egui::Atom::custom(egui::IdSalt::new("menu-checkmark"), egui::vec2(12.0, 12.0)), label)
}

/// Draw a command with an optional checkmark and dispatch it when activated.
fn menu_item(app: &mut MaterialCraftApp, context: &egui::Context, ui: &mut egui::Ui, item: &MenuItem) {
    let modality = Modality::capture(app, context);
    let is_enabled = modality.allows(item.id) && enabled(app, item.id);
    let mut button = egui::Button::new(menu_label(tr(item.label)));
    if let Some(shortcut) = item.shortcut {
        button = button.shortcut_text(shortcut_label(context, shortcut));
    }
    let checked = match item.id {
        "ui.view.graph" => Some(true),
        "ui.graph.snapToGrid" => Some(app.ui.prefs.snap_to_grid),
        id if id.starts_with("ui.preview.") => {
            id.strip_prefix("ui.preview.").and_then(|size| size.parse::<u32>().ok()).map(|size| size == app.ui.prefs.preview_size)
        }
        id => crate::docking::view_panel(id).map(|panel| app.ui.dock.is_open(panel)),
    };
    if let Some(checked) = checked {
        // Keep the accessible toggle state without styling the entire row as selected
        button = button.selected(checked);
        button.remove_class(egui::Button::CLASS_SELECTED);
    }

    let response = ui.add_enabled(is_enabled, |ui: &mut egui::Ui| {
        let response = button.atom_ui(ui);
        if checked == Some(true)
            && let Some(rect) = response.rect(egui::IdSalt::new("menu-checkmark"))
        {
            let center = rect.center();
            let color = ui.style().interact(&response).fg_stroke.color;
            ui.painter().add(egui::Shape::line(
                vec![center + egui::vec2(-4.0, 0.0), center + egui::vec2(-1.0, 3.0), center + egui::vec2(4.0, -3.0)],
                egui::Stroke::new(1.5, color),
            ));
        }
        response.response
    });
    if response.clicked() {
        invoke(app, context, item.id);
        ui.close();
    }

    #[cfg(not(target_arch = "wasm32"))]
    if item.id == "file.open" {
        ui.add_enabled_ui(!modality.is_blocked() && !app.ui.recent.is_empty(), |ui| {
            ui.menu_button(menu_label(tr("Recent packages")), |ui| {
                for path in app.ui.recent.clone() {
                    if ui.button(menu_label(&path)).clicked() {
                        crate::windows::open_path(app, &path);
                        ui.close();
                    }
                }
                ui.separator();
                if ui.button(menu_label(tr("Clear recent packages"))).clicked() {
                    app.ui.recent.clear();
                    ui.close();
                }
            });
        });
    }
}

/// Dispatch shortcuts unless a widget, popup or dialog owns the keyboard
pub fn shortcuts(app: &mut MaterialCraftApp, ctx: &egui::Context) {
    if ctx.egui_wants_keyboard_input() || egui::Popup::is_any_open(ctx) {
        return;
    }
    // The desktop sends Copy, Cut and Paste events in place of their key presses
    if !ctx
        .input(|i| i.events.iter().any(|e| matches!(e, egui::Event::Key { pressed: true, .. } | egui::Event::Copy | egui::Event::Cut | egui::Event::Paste(_))))
    {
        return;
    }

    let modality = Modality::capture(app, ctx);
    if !modality.is_blocked() && crate::explorer::shortcuts(app, ctx) {
        return;
    }
    if !modality.is_blocked() {
        crate::graph::wire_actions::shortcuts(app, ctx);
    }

    let platform = craft_keymap::egui::platform(ctx);
    if app.keymap.platform() != platform {
        app.keymap = keymap(platform);
    }
    // Selected label text copies through egui's Copy and Cut events, which a hit would remove
    let label_selected = label_text_selected(ctx);
    // A clipboard chord with nothing to copy or paste leaves its event to egui instead of reporting an error
    let unavailable_clipboard_commands: Vec<&str> = ["edit.copy", "edit.cut", "edit.paste"].into_iter().filter(|id| !enabled(app, id)).collect();
    let hits = ctx.input_mut(|i| {
        app.dispatcher.dispatch_filtered(i, &app.keymap, &Active::NONE, |hit| {
            let id = hit.command.id.as_str();
            modality.allows(id) && !(label_selected && matches!(id, "edit.copy" | "edit.cut")) && !unavailable_clipboard_commands.contains(&id)
        })
    });
    for hit in hits {
        if crate::graph::editor_actions::shortcut_enabled(app, &hit.command.id) {
            invoke(app, ctx, &hit.command.id);
        }
    }
}

fn label_text_selected(ctx: &egui::Context) -> bool {
    ctx.with_plugin::<egui::text_selection::LabelSelectionState, _>(|state| state.has_selection()).unwrap_or(false)
}

/// Menu items for `ui.menu.list`
pub fn list(app: &MaterialCraftApp, ctx: &egui::Context) -> Value {
    let modality = Modality::capture(app, ctx);
    Value::Array(
        CATALOG
            .iter()
            .filter(|i| i.label != "-")
            .map(|i| {
                serde_json::json!({"menu": i.menu, "submenu": i.sub, "label": i.label, "id": i.id, "shortcut": i.shortcut,
                    "implemented": implemented(i.id), "enabled": modality.allows(i.id) && enabled(app, i.id)})
            })
            .collect(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_shortcuts_parse_and_ids_are_unique() {
        let mut seen = std::collections::BTreeSet::new();
        for i in CATALOG.iter().filter(|i| i.label != "-") {
            assert!(seen.insert(i.id), "duplicate menu id {}", i.id);
            if let Some(sc) = i.shortcut {
                assert!(parse_shortcut(sc).is_some(), "{sc}");
            }
        }
        // Almost every item is live
        let live = CATALOG.iter().filter(|i| i.label != "-" && implemented(i.id)).count();
        assert!(live + 1 >= CATALOG.iter().filter(|i| i.label != "-").count());
    }

    #[test]
    fn every_shortcut_is_a_chord_and_the_catalog_wins_a_shared_chord() {
        for platform in [Platform::Mac, Platform::Windows, Platform::Linux] {
            let map = keymap(platform);
            let mut owners = std::collections::BTreeMap::new();
            for (id, text) in shortcut_table() {
                let chord = parse_shortcut(text).unwrap_or_else(|| panic!("{id}: {text}"));
                owners.entry(chord.to_string()).or_insert((id, chord));
            }
            for (id, chord) in owners.into_values() {
                assert_eq!(map.lookup(&chord, false, &Active::NONE), craft_keymap::Lookup::Run(id.into()), "{chord}");
            }
        }
    }

    #[test]
    fn ctrl_in_a_shortcut_text_is_command_on_macos() {
        let chord = parse_shortcut("Ctrl+Shift+E").unwrap();
        assert_eq!(chord.display(Platform::Mac), "⇧⌘E");
        assert_eq!(chord.display(Platform::Windows), "Ctrl+Shift+E");
    }

    fn dispatched(key: egui::Key, modifiers: egui::Modifiers, app: &mut MaterialCraftApp) {
        let ctx = egui::Context::default();
        let input =
            egui::RawInput { events: vec![egui::Event::Key { key, physical_key: None, pressed: true, repeat: false, modifiers }], ..Default::default() };
        ctx.run_ui(input, |ui| shortcuts(app, ui.ctx())).textures_delta.clear();
    }

    fn app() -> MaterialCraftApp {
        MaterialCraftApp::headless(materialcraft_engine::sample_session(), crate::UiState::default())
    }

    #[test]
    fn a_shortcut_needs_exactly_its_modifiers() {
        let mut exact = app();
        dispatched(egui::Key::N, egui::Modifiers::COMMAND, &mut exact);
        assert!(exact.ui.windows.new_graph, "Ctrl+N is bound to New graph");

        let mut extra = app();
        dispatched(egui::Key::N, egui::Modifiers::COMMAND | egui::Modifiers::SHIFT, &mut extra);
        assert!(!extra.ui.windows.new_graph, "Ctrl+Shift+N is not bound");
    }

    #[test]
    fn a_longer_chord_runs_its_own_command() {
        let mut app = app();
        dispatched(egui::Key::E, egui::Modifiers::COMMAND | egui::Modifiers::SHIFT, &mut app);
        assert!(app.ui.windows.export, "Ctrl+Shift+E opens Export outputs");
    }

    #[test]
    fn a_scripted_ctrl_key_runs_its_shortcut() {
        let mut app = app();
        let ctx = egui::Context::default();
        let frames = crate::control::parse_events(&[serde_json::json!({"type": "key", "key": "N", "modifiers": ["ctrl"]})]).unwrap();
        for frame in frames {
            let input = egui::RawInput { events: frame.events, ..Default::default() };
            ctx.run_ui(input, |ui| shortcuts(&mut app, ui.ctx())).textures_delta.clear();
        }
        assert!(app.ui.windows.new_graph);
    }

    #[test]
    fn a_held_key_repeats_its_shortcut() {
        let mut app = app();
        let ctx = egui::Context::default();
        // egui marks the second press of a key that was never released as a repeat
        for _ in 0..2 {
            app.ui.windows.new_graph = false;
            let input = egui::RawInput {
                events: vec![egui::Event::Key { key: egui::Key::N, physical_key: None, pressed: true, repeat: false, modifiers: egui::Modifiers::COMMAND }],
                ..Default::default()
            };
            ctx.run_ui(input, |ui| shortcuts(&mut app, ui.ctx())).textures_delta.clear();
            assert!(app.ui.windows.new_graph, "a held key runs the shortcut again");
        }
    }

    #[test]
    fn shortcuts_wait_while_a_text_field_has_the_keyboard() {
        let mut app = app();
        let ctx = egui::Context::default();
        let field = egui::Id::unique("field");
        ctx.run_ui(egui::RawInput::default(), |ui| ui.memory_mut(|m| m.request_focus(field))).textures_delta.clear();
        let input = egui::RawInput {
            events: vec![egui::Event::Key { key: egui::Key::N, physical_key: None, pressed: true, repeat: false, modifiers: egui::Modifiers::COMMAND }],
            ..Default::default()
        };
        ctx.run_ui(input, |ui| {
            let mut text = String::new();
            ui.add(egui::TextEdit::singleline(&mut text).id(field));
            shortcuts(&mut app, ui.ctx());
        })
        .textures_delta
        .clear();
        assert!(!app.ui.windows.new_graph);
    }

    #[test]
    fn delete_waits_behind_the_unsaved_prompt() {
        let mut app = app();
        app.run("edit.selectAll", serde_json::json!({}));
        let nodes = app.session.graph().unwrap().nodes.len();
        assert!(nodes > 0);

        app.ui.windows.unsaved = Some(crate::unsaved::Guarded::Quit);
        dispatched(egui::Key::Delete, egui::Modifiers::NONE, &mut app);
        assert_eq!(app.session.graph().unwrap().nodes.len(), nodes, "Delete does not edit the package behind the prompt");

        app.ui.windows.unsaved = None;
        dispatched(egui::Key::Delete, egui::Modifiers::NONE, &mut app);
        assert_eq!(app.session.graph().unwrap().nodes.len(), 0, "Delete works once the prompt is answered");
    }

    #[test]
    fn wire_keys_wait_behind_the_unsaved_prompt() {
        let mut app = app();
        let wire = app.session.graph().unwrap().connections.first().map(|wire| (wire.to, wire.to_port.clone())).unwrap();
        let wires = app.session.graph().unwrap().connections.len();
        app.graph.selected_wires = vec![wire];

        app.ui.windows.unsaved = Some(crate::unsaved::Guarded::Quit);
        dispatched(egui::Key::Delete, egui::Modifiers::NONE, &mut app);
        assert_eq!(app.session.graph().unwrap().connections.len(), wires);

        app.ui.windows.unsaved = None;
        dispatched(egui::Key::Delete, egui::Modifiers::NONE, &mut app);
        assert_eq!(app.session.graph().unwrap().connections.len(), wires - 1, "Delete works once the prompt is answered");
    }

    /// Whether a press is still in egui's input after `shortcuts`, where a shortcut that ran removes it
    fn press_left_in_input(app: &mut MaterialCraftApp, key: egui::Key, modifiers: egui::Modifiers) -> bool {
        let ctx = egui::Context::default();
        let input =
            egui::RawInput { events: vec![egui::Event::Key { key, physical_key: None, pressed: true, repeat: false, modifiers }], ..Default::default() };
        let mut left = false;
        ctx.run_ui(input, |ui| {
            shortcuts(app, ui.ctx());
            left = ui.input(|i| i.events.iter().any(|e| matches!(e, egui::Event::Key { key: pressed, .. } if *pressed == key)));
        })
        .textures_delta
        .clear();
        left
    }

    #[test]
    fn behind_a_file_dialog_only_the_commands_that_show_one_run() {
        let mut app = app();
        app.dialogs.ask(craft_dialogs::FileDialog::open(), crate::native_dialogs::Then::OpenPackage);

        assert!(!press_left_in_input(&mut app, egui::Key::O, egui::Modifiers::COMMAND), "Ctrl+O reaches the open dialog");
        assert!(press_left_in_input(&mut app, egui::Key::N, egui::Modifiers::COMMAND), "Ctrl+N waits");
        assert!(!app.ui.windows.new_graph);
        assert_eq!(app.dialogs.queued(), 1, "the dialog comes forward and no second one is asked for");
    }

    #[test]
    fn invoke_waits_behind_a_file_dialog_except_for_the_commands_that_show_one() {
        let ctx = egui::Context::default();
        let mut app = app();
        app.dialogs.ask(craft_dialogs::FileDialog::open(), crate::native_dialogs::Then::OpenPackage);
        invoke(&mut app, &ctx, "ui.export");
        assert!(!app.ui.windows.export);
        invoke(&mut app, &ctx, "file.open");
        assert_eq!(app.dialogs.queued(), 1, "the dialog comes forward and no second one is asked for");
    }

    #[test]
    fn invoke_runs_nothing_behind_a_prompt() {
        let mut app = app();
        let ctx = egui::Context::default();
        app.ui.windows.unsaved = Some(crate::unsaved::Guarded::Quit);
        invoke(&mut app, &ctx, "ui.export");
        assert!(!app.ui.windows.export);

        app.ui.windows.unsaved = None;
        invoke(&mut app, &ctx, "ui.export");
        assert!(app.ui.windows.export);
    }

    /// Explorer shortcuts affect the hovered selection and leave graph shortcuts elsewhere.
    #[test]
    fn explorer_keys_act_on_its_selection_only_while_hovered() {
        use crate::explorer::ExplorerTarget;
        let mut app = app();
        app.run("file.new", serde_json::json!({"template": "empty", "name": "Second"})).unwrap();
        let graphs = app.session.pkg.graphs.len();
        let document = app.session.document_id;
        app.ui.explorer.selected = Some(ExplorerTarget::Graph { document, index: 0 });

        // Away from the Explorer, F2 and Delete keep their graph shortcuts
        dispatched(egui::Key::F2, egui::Modifiers::NONE, &mut app);
        assert!(app.ui.explorer.rename.is_none());

        app.ui.explorer.hovered = true;
        dispatched(egui::Key::Enter, egui::Modifiers::NONE, &mut app);
        assert_eq!(app.session.graph, 0);
        dispatched(egui::Key::F2, egui::Modifiers::NONE, &mut app);
        assert!(matches!(app.ui.explorer.rename, Some((ExplorerTarget::Graph { index: 0, .. }, _))));
        app.ui.explorer.rename = None;
        dispatched(egui::Key::Delete, egui::Modifiers::NONE, &mut app);
        assert_eq!(app.session.pkg.graphs.len(), graphs - 1);
        dispatched(egui::Key::Z, egui::Modifiers::COMMAND, &mut app);
        assert_eq!(app.session.pkg.graphs.len(), graphs, "removing a graph undoes");
    }

    /// Explorer menu actions resolve through their documented UI or engine handler.
    #[test]
    fn explorer_menus_name_only_actions_that_exist() {
        for rows in [EXPLORER_PACKAGE, EXPLORER_FOLDER, EXPLORER_GRAPH] {
            for row in rows {
                let ids: Vec<&str> = match row {
                    ExplorerRow::Item(_, id, _) => vec![id],
                    ExplorerRow::Sub(_, entries) => entries.iter().map(|(_, id)| *id).collect(),
                    ExplorerRow::Separator => Vec::new(),
                };
                for id in ids.into_iter().filter(|id| !id.is_empty()) {
                    assert!(implemented(id) || id.starts_with("ui.explorer.") || id.starts_with("explorer."), "{id}");
                }
                if let ExplorerRow::Item(label, _, Some(shortcut)) = row {
                    assert!(parse_shortcut(shortcut).is_some(), "{label}: {shortcut}");
                }
            }
        }
    }

    #[test]
    fn the_shortcuts_list_leaves_shortcuts_running() {
        let mut app = app();
        app.ui.windows.shortcuts = true;
        dispatched(egui::Key::N, egui::Modifiers::COMMAND, &mut app);
        assert!(app.ui.windows.new_graph);
    }

    #[test]
    fn menu_bar_items_gray_out_behind_a_prompt() {
        use egui_kittest::kittest::{By, NodeT, Queryable};

        let mut app = app();
        app.ui.recent.push("recent.sbs".into());
        let mut harness = egui_kittest::Harness::builder().build_ui_state(|ui, app: &mut MaterialCraftApp| menu_bar(app, ui), app);
        harness.run_steps(2);
        harness.get_by_label("File").click();
        harness.run_steps(3);
        let item = |label| By::new().role(egui::accesskit::Role::Button).label(label);
        assert!(!harness.get(item("Close all")).accesskit_node().is_disabled());
        assert!(!harness.get(item("Recent packages")).accesskit_node().is_disabled());

        harness.state_mut().ui.windows.unsaved = Some(crate::unsaved::Guarded::Quit);
        harness.run_steps(3);
        assert!(harness.get(item("Close all")).accesskit_node().is_disabled());
        assert!(harness.get(item("Recent packages")).accesskit_node().is_disabled());
    }

    #[test]
    fn open_stays_enabled_behind_a_file_dialog() {
        use egui_kittest::kittest::{By, NodeT, Queryable};

        let mut app = app();
        app.dialogs.ask(craft_dialogs::FileDialog::open(), crate::native_dialogs::Then::OpenPackage);
        let mut harness = egui_kittest::Harness::builder().build_ui_state(|ui, app: &mut MaterialCraftApp| menu_bar(app, ui), app);
        harness.run_steps(2);
        harness.get_by_label("File").click();
        harness.run_steps(3);
        let item = |label| By::new().role(egui::accesskit::Role::Button).label_contains(label);
        assert!(!harness.get(item("Open...")).accesskit_node().is_disabled());
        assert!(harness.get(item("Close all")).accesskit_node().is_disabled());
    }

    fn press_c(events_after: Vec<egui::Event>) -> egui::RawInput {
        let mut events = vec![
            egui::Event::ModifiersChanged(egui::Modifiers::COMMAND),
            egui::Event::Key { key: egui::Key::C, physical_key: None, pressed: true, repeat: false, modifiers: egui::Modifiers::COMMAND },
        ];
        events.extend(events_after);
        egui::RawInput { events, ..Default::default() }
    }

    /// Run a frame with `input` around a selectable label, and report whether egui's Copy event is still there after `shortcuts`
    fn copy_event_kept(app: &mut MaterialCraftApp, ctx: &egui::Context, input: egui::RawInput) -> bool {
        let mut kept = false;
        ctx.run_ui(input, |ui| {
            shortcuts(app, ui.ctx());
            kept = ui.input(|i| i.events.contains(&egui::Event::Copy));
            ui.label("selectable text");
        })
        .textures_delta
        .clear();
        kept
    }

    fn select_label_text(ctx: &egui::Context) {
        let frame = |events: Vec<egui::Event>| {
            let mut rect = egui::Rect::NOTHING;
            ctx.run_ui(egui::RawInput { events, ..Default::default() }, |ui| rect = ui.label("selectable text").rect).textures_delta.clear();
            rect
        };
        let rect = frame(Vec::new());
        let (from, to) = (rect.left_center() + egui::vec2(1.0, 0.0), rect.right_center() - egui::vec2(1.0, 0.0));
        let button = |pos, pressed| egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed, modifiers: egui::Modifiers::NONE };
        frame(vec![egui::Event::PointerMoved(from)]);
        frame(vec![button(from, true)]);
        frame(vec![egui::Event::PointerMoved(to)]);
        frame(vec![button(to, false)]);
        frame(Vec::new());
    }

    #[test]
    fn the_desktops_copy_event_copies_the_selected_nodes() {
        let mut app = app();
        app.run("edit.selectAll", serde_json::json!({}));
        let ctx = egui::Context::default();

        let kept = copy_event_kept(&mut app, &ctx, egui::RawInput { events: vec![egui::Event::Copy], ..Default::default() });

        assert!(!kept, "edit.copy takes the event");
        assert!(app.session.clipboard.is_some());
    }

    #[test]
    fn selected_label_text_keeps_the_desktops_copy_event() {
        let mut app = app();
        app.run("edit.selectAll", serde_json::json!({}));
        let ctx = egui::Context::default();
        select_label_text(&ctx);

        let kept = copy_event_kept(&mut app, &ctx, egui::RawInput { events: vec![egui::Event::Copy], ..Default::default() });

        assert!(kept, "egui copies the label's text from this event");
        assert!(app.session.clipboard.is_none(), "the event does not also copy the selected nodes");
    }

    #[test]
    fn clipboard_events_with_nothing_to_copy_or_paste_stay_with_egui() {
        let mut app = app();
        app.run("edit.selectNone", serde_json::json!({}));
        app.session.clipboard = None;
        let toasts = app.toasts.len();
        let events = vec![egui::Event::Copy, egui::Event::Cut, egui::Event::Paste("text from another app".into())];
        let ctx = egui::Context::default();

        let mut left = Vec::new();
        ctx.run_ui(egui::RawInput { events: events.clone(), ..Default::default() }, |ui| {
            shortcuts(&mut app, ui.ctx());
            left = ui.input(|i| i.events.clone());
        })
        .textures_delta
        .clear();

        assert_eq!(left, events);
        assert_eq!(app.toasts.len(), toasts, "no error toast for a chord that runs nothing");
    }

    #[test]
    fn a_web_copy_press_runs_edit_copy() {
        let mut app = app();
        app.dispatcher = craft_keymap::egui::Dispatcher::new().with_repeat(true).with_clipboard_events(true);
        app.run("edit.selectAll", serde_json::json!({}));
        let ctx = egui::Context::default();
        copy_event_kept(&mut app, &ctx, press_c(vec![egui::Event::Copy]));
        assert!(app.session.clipboard.is_some(), "the web build's press runs edit.copy");
    }

    #[test]
    fn selected_label_text_keeps_its_copy_on_the_web() {
        let mut app = app();
        app.dispatcher = craft_keymap::egui::Dispatcher::new().with_repeat(true).with_clipboard_events(true);
        app.run("edit.selectAll", serde_json::json!({}));
        let ctx = egui::Context::default();
        select_label_text(&ctx);
        assert!(label_text_selected(&ctx));
        assert!(copy_event_kept(&mut app, &ctx, press_c(vec![egui::Event::Copy])), "egui copies the label's text from this event");
        assert!(app.session.clipboard.is_none(), "the press does not also copy the selected nodes");
    }

    #[test]
    fn the_interface_fonts_draw_every_shortcut_label() {
        for os in [egui::os::OperatingSystem::Mac, egui::os::OperatingSystem::Windows, egui::os::OperatingSystem::Nix] {
            let ctx = egui::Context::default();
            ctx.set_os(os);
            crate::theme::install(&ctx);
            ctx.run_ui(egui::RawInput::default(), |_| {}).textures_delta.clear();
            for (id, text) in shortcut_table() {
                let label = shortcut_label(&ctx, text);
                for family in [egui::FontFamily::Proportional, egui::FontFamily::Monospace] {
                    let font = egui::FontId::new(13.0, family.clone());
                    assert!(ctx.fonts_mut(|fonts| fonts.has_glyphs(&font, &label)), "{id}: {label} in {family:?}");
                }
            }
        }
    }

    #[test]
    fn macos_labels_name_the_command_key() {
        let ctx = egui::Context::default();
        ctx.set_os(egui::os::OperatingSystem::Mac);
        assert_eq!(shortcut_label(&ctx, "Ctrl+Shift+E"), "Cmd+Shift+E");
        ctx.set_os(egui::os::OperatingSystem::Windows);
        assert_eq!(shortcut_label(&ctx, "Ctrl+Shift+E"), "Ctrl+Shift+E");
    }
    /// A hidden Explorer must not delete its selection using stale hover state.
    #[test]
    fn review_hidden_explorer_ignores_delete() {
        let mut app = app();
        let graphs = app.session.pkg.graphs.len();
        app.ui.explorer.selected = Some(crate::explorer::ExplorerTarget::Graph { document: app.session.document_id, index: 0 });
        app.ui.explorer.hovered = true;
        app.ui.dock.close(crate::docking::Panel::Explorer);

        dispatched(egui::Key::Delete, egui::Modifiers::NONE, &mut app);

        assert_eq!(app.session.pkg.graphs.len(), graphs);
    }
}
