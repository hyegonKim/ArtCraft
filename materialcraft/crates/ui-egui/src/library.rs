//! The library: a category tree on the left (Favorites, then the library's categories and
//! sub-categories), the matching entries as preview tiles (or a list) on the right, with search and
//! filters. Drag an entry into the graph, or double-click it to add it. Entries the open graph
//! can't take, and library nodes MaterialCraft doesn't have yet, are dimmed.

use std::sync::Arc;

use egui::{Color32, Rect, RichText, Sense, Ui, vec2};
use egui_phosphor::regular as icon;
use materialcraft_engine::node_library::CATEGORIES;
use materialcraft_engine::{LibraryCanvas, LibraryEntry, LibraryItem};

use crate::MaterialCraftApp;
use crate::function::library::FunctionLibraryDrag;
mod library_target;
use crate::graph::{LibraryDrag, header_color};
use crate::i18n::tr;
use crate::theme::Tokens;
pub(crate) use library_target::LibraryTarget;

/// The category key of the Favorites row
pub(crate) const FAVORITES: &str = "@favorites";
/// Function entries that need more nodes than MaterialCraft has are shown in this color
const WARNING: Color32 = Color32::from_rgb(255, 90, 180);

/// Choose the icon for a known category or fall back to a folder.
fn category_icon(category: &str) -> &'static str {
    match category {
        FAVORITES => icon::STAR,
        "Graph items" => icon::FLOW_ARROW,
        "Atomic nodes" => icon::CUBE,
        "FxMap nodes" => icon::SPARKLE,
        "Function nodes" => icon::FUNCTION,
        "Texture generators" => icon::DICE_FIVE,
        "Texture generators/Noises" => icon::CLOUD,
        "Texture generators/Patterns" => icon::CHECKERBOARD,
        "Filters" => icon::FADERS,
        "Filters/Adjustments" => icon::CIRCLE_HALF_TILT,
        "Filters/Blending" | "Material filters/Blending" => icon::INTERSECT,
        "Filters/Blurs" => icon::DROP,
        "Filters/Channels" => icon::STACK,
        "Filters/Effects" => icon::MAGIC_WAND,
        "Filters/Normal map" => icon::COMPASS,
        "Filters/Tiling" => icon::SQUARES_FOUR,
        "Filters/Transforms" | "Material filters/Transforms" => icon::BOUNDING_BOX,
        "Material filters" => icon::SPHERE,
        "Material filters/1-click" => icon::CURSOR_CLICK,
        "Material filters/Effects" => icon::SPARKLE,
        "Material filters/PBR utilities" => icon::WRENCH,
        "Material filters/Scan processing" => icon::SCAN,
        "Mesh-based generators" => icon::POLYGON,
        "Mesh-based generators/Mask generators" => icon::CUBE_TRANSPARENT,
        "Mesh-based generators/Weathering" => icon::CLOUD_RAIN,
        "Mesh-based generators/Utilities" => icon::TOOLBOX,
        "Spline & Path tools" | "Spline & Path tools/Spline tools" => icon::BEZIER_CURVE,
        "Spline & Path tools/Path tools" => icon::PATH,
        "MCF functions" => icon::MATH_OPERATIONS,
        "Functions" => icon::WAVE_SINE,
        "3D view" => icon::GLOBE,
        "3D view/HDRI environments" => icon::GLOBE_HEMISPHERE_WEST,
        "3D view/HDRI tools" => icon::SUN,
        "PBR materials" => icon::CIRCLE_HALF,
        "Values" => icon::HASH,
        _ => match category.split('/').next().unwrap_or("") {
            "Function nodes" | "MCF functions" | "Values" => icon::ARROW_SQUARE_OUT,
            "Functions" => icon::WAVE_SINE,
            "PBR materials" => icon::CIRCLE,
            _ => icon::FOLDER_SIMPLE,
        },
    }
}

/// The library's entries for the open session, rebuilt when its packages change
fn entries(app: &MaterialCraftApp, ctx: &egui::Context) -> Arc<Vec<LibraryEntry>> {
    type Cached = (Vec<(String, usize)>, Arc<Vec<LibraryEntry>>);
    let mut key: Vec<(String, usize)> = vec![(String::new(), Arc::as_ptr(&app.session.library) as usize)];
    key.extend(
        app.session.pkg.dependencies.iter().filter(|(path, _)| path.starts_with("sbs://")).map(|(path, package)| (path.clone(), Arc::as_ptr(package) as usize)),
    );
    let id = egui::Id::unique("library-entries");
    if let Some((cached, list)) = ctx.data(|data| data.get_temp::<Arc<Cached>>(id)).as_deref()
        && *cached == key
    {
        return list.clone();
    }

    let list = Arc::new(app.session.library_entries());
    ctx.data_mut(|data| data.insert_temp(id, Arc::new((key, list.clone()))));
    list
}

/// Draw the shared library tree and search results for the active editor.
pub fn show(app: &mut MaterialCraftApp, ui: &mut Ui) {
    let tokens = Tokens::get(ui.ctx());
    let target = LibraryTarget::of(app);
    let all = entries(app, ui.ctx());
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 2.0;
        ui.label(RichText::new(icon::MAGNIFYING_GLASS).color(tokens.text_dim));
        let hint = match target.canvas {
            LibraryCanvas::Function => tr("Search functions"),
            LibraryCanvas::Pattern => tr("Search FX-Map nodes"),
            LibraryCanvas::Any | LibraryCanvas::Material => tr("Search"),
        };
        let width = (ui.available_width() - 5.0 * 24.0).max(40.0);
        let response = ui.add(egui::TextEdit::singleline(&mut app.ui.library_search).hint_text(hint).desired_width(width));
        crate::widgets::name(&response, egui::Role::SearchInput, "Search the library");
        filter_menu(app, ui);
        let selected = app.ui.library_selected.clone();
        let favorite = selected.as_ref().is_some_and(|id| app.ui.library_favorites.contains(id));
        let tooltip = if favorite { tr("Remove from Favorites") } else { tr("Add to Favorites") };
        let star = ui.add_enabled_ui(selected.is_some(), |ui| crate::widgets::icon_button(ui, icon::STAR, tooltip, favorite, 22.0)).inner;
        if star.clicked()
            && let Some(id) = selected
        {
            toggle_favorite(app, &id);
        }
        if crate::widgets::icon_button(ui, icon::ARROWS_CLOCKWISE, tr("Reload library"), false, 22.0).clicked() {
            app.run("library.reload", serde_json::json!({}));
        }
        if crate::widgets::icon_button(ui, icon::SQUARES_FOUR, tr("Icons"), !app.ui.prefs.library_list, 22.0).clicked() {
            app.ui.prefs.library_list = false;
        }
        if crate::widgets::icon_button(ui, icon::LIST, tr("List"), app.ui.prefs.library_list, 22.0).clicked() {
            app.ui.prefs.library_list = true;
        }
    });
    ui.add_space(2.0);
    let available_size = ui.available_size();

    // The tree takes what the tiles leave: one column of tiles in a narrow panel
    let tree_width = (available_size.x * 0.6).clamp(120.0, 230.0).min(available_size.x - 92.0).max(80.0);
    let top_down = egui::Layout::top_down(egui::Align::Min);
    ui.horizontal_top(|ui| {
        ui.allocate_ui_with_layout(vec2(tree_width, available_size.y), top_down, |ui| {
            egui::ScrollArea::vertical().id_salt("lib-tree").auto_shrink([false, false]).show(ui, |ui| category_tree(app, ui, &all));
        });
        ui.separator();
        ui.allocate_ui_with_layout(vec2(ui.available_width(), available_size.y), top_down, |ui| {
            egui::ScrollArea::vertical().id_salt("lib-items").auto_shrink([false, false]).show(ui, |ui| items(app, ui, &all, &target));
        });
    });
}

/// Expose availability and canvas filters without changing the selected category.
fn filter_menu(app: &mut MaterialCraftApp, ui: &mut Ui) {
    let filtered = !app.ui.library_show_missing || !app.ui.library_show_other;
    let response = crate::widgets::icon_button(ui, icon::FUNNEL, tr("Filter"), filtered, 22.0);
    egui::Popup::menu(&response).show(|ui| {
        ui.set_min_width(240.0);
        ui.checkbox(&mut app.ui.library_show_missing, tr("Show nodes not in MaterialCraft yet"));
        ui.checkbox(&mut app.ui.library_show_other, tr("Show nodes for other graph types"));
    });
}

/// Toggle an entry while bounding the saved favorites list.
fn toggle_favorite(app: &mut MaterialCraftApp, id: &str) {
    if let Some(index) = app.ui.library_favorites.iter().position(|favorite| favorite == id) {
        app.ui.library_favorites.remove(index);
    } else if app.ui.library_favorites.len() < 512 {
        app.ui.library_favorites.push(id.to_string());
    }
}

/// One tree row: an optional disclosure triangle, the category icon and its name
fn tree_row(app: &mut MaterialCraftApp, ui: &mut Ui, label: &str, category: &str, depth: usize, open: Option<bool>, count: usize) {
    let tokens = Tokens::get(ui.ctx());
    let selected = app.ui.library_category == category;
    let (rect, response) = ui.allocate_exact_size(vec2(ui.available_width(), 20.0), Sense::click());
    crate::widgets::name(&response, egui::Role::TreeItem, label);
    let response = response.on_hover_text(crate::i18n::trf("{name}: {count} entries", &[("name", &label), ("count", &count)]));
    if selected {
        ui.painter().rect_filled(rect, 4.0, tokens.row_selected);
    } else if response.hovered() {
        ui.painter().rect_filled(rect, 4.0, tokens.row_hover);
    }
    let indent = 4.0 + depth as f32 * 14.0;
    let chevron = Rect::from_min_size(rect.min + vec2(indent, 0.0), vec2(14.0, rect.height()));
    if let Some(open) = open {
        let symbol = if open { icon::CARET_DOWN } else { icon::CARET_RIGHT };
        ui.painter().text(chevron.center(), egui::Align2::CENTER_CENTER, symbol, egui::FontId::proportional(11.0), tokens.text_dim);
    }
    let x = chevron.right() + 2.0;
    let color = if selected { tokens.text_strong } else { tokens.text };
    ui.painter().text(egui::pos2(x + 7.0, rect.center().y), egui::Align2::CENTER_CENTER, category_icon(category), egui::FontId::proportional(13.0), color);
    let font = if depth == 0 { 12.0 } else { 11.5 };
    let galley = crate::widgets::elided(ui.painter(), label.to_string(), font, color, rect.right() - x - 22.0);
    ui.painter().galley(egui::pos2(x + 18.0, rect.center().y - galley.size().y * 0.5), galley, color);
    if response.clicked() {
        let on_chevron = response.interact_pointer_pos().is_some_and(|pointer| chevron.contains(pointer));
        if let Some(open) = open
            && (on_chevron || !open || selected)
        {
            set_open(app, category, !open);
        }
        if !on_chevron {
            app.ui.library_category = category.to_string();
        }
    }
}

/// Persist the expansion state of one category.
fn set_open(app: &mut MaterialCraftApp, category: &str, open: bool) {
    if open {
        app.ui.library_expanded.insert(category.to_string());
    } else {
        app.ui.library_expanded.remove(category);
    }
}

/// How many entries a category holds, its sub-categories included
fn count(entries: &[LibraryEntry], category: &str) -> usize {
    entries.iter().filter(|entry| entry.category == category || entry.category.strip_prefix(category).is_some_and(|rest| rest.starts_with('/'))).count()
}

/// Draw known categories first, followed by categories supplied by packages.
fn category_tree(app: &mut MaterialCraftApp, ui: &mut Ui, entries: &[LibraryEntry]) {
    ui.spacing_mut().item_spacing.y = 1.0;
    let favorites = app.ui.library_favorites.len();
    tree_row(app, ui, tr("Favorites"), FAVORITES, 0, None, favorites);
    for (top, subs) in CATEGORIES {
        let open = (!subs.is_empty()).then(|| app.ui.library_expanded.contains(*top));
        tree_row(app, ui, tr(top), top, 0, open, count(entries, top));
        if open == Some(true) {
            for sub in *subs {
                let category = format!("{top}/{sub}");
                tree_row(app, ui, tr(sub), &category, 1, None, count(entries, &category));
            }
        }
    }

    // Packages can name categories the tree doesn't have; they go last
    let mut extra: Vec<&str> = Vec::new();
    for entry in entries {
        let top = entry.category.split('/').next().unwrap_or("");
        if !top.is_empty() && !CATEGORIES.iter().any(|(name, _)| *name == top) && !extra.contains(&top) {
            extra.push(top);
        }
    }
    for top in extra {
        tree_row(app, ui, tr(top), top, 0, None, count(entries, top));
    }
}

/// Matching entries ordered by their displayed names in tiles and list views.
pub(crate) fn visible<'a>(app: &MaterialCraftApp, entries: &'a [LibraryEntry], target: &LibraryTarget) -> Vec<&'a LibraryEntry> {
    let search = app.ui.library_search.trim().to_lowercase();
    let category = app.ui.library_category.as_str();
    let mut visible: Vec<_> = entries
        .iter()
        .filter(|entry| app.ui.library_show_missing || entry.missing.is_none())
        .filter(|entry| app.ui.library_show_other || entry.canvas == LibraryCanvas::Any || entry.canvas == target.canvas)
        .filter(|entry| {
            if !search.is_empty() {
                return tr(&entry.label).to_lowercase().contains(&search) || entry.id.to_lowercase().contains(&search);
            }

            match category {
                "" => true,
                FAVORITES => app.ui.library_favorites.contains(&entry.id),
                category => entry.category == category || entry.category.strip_prefix(category).is_some_and(|rest| rest.starts_with('/')),
            }
        })
        .collect();
    visible.sort_by_cached_key(|entry| (tr(&entry.label).to_lowercase(), entry.id.as_str()));
    visible
}

/// Draw matching entries and dispatch compatible additions or favorite changes.
fn items(app: &mut MaterialCraftApp, ui: &mut Ui, entries: &[LibraryEntry], target: &LibraryTarget) {
    let tokens = Tokens::get(ui.ctx());
    let ctx = ui.ctx().clone();
    let list = visible(app, entries, target);
    if list.is_empty() {
        let text = match app.ui.library_category.as_str() {
            FAVORITES if app.ui.library_search.trim().is_empty() => tr("Select an entry and click the star to add it to Favorites"),
            category if app.ui.library_search.trim().is_empty() && !category.is_empty() => tr("Nothing in this category yet"),
            _ => tr("Nothing matches"),
        };
        ui.label(RichText::new(text).color(tokens.text_dim));
        return;
    }

    let as_list = app.ui.prefs.library_list;
    let tile = vec2(78.0, 100.0);
    let columns = if as_list { 1 } else { ((ui.available_width() + 4.0) / (tile.x + 4.0)).floor().max(1.0) as usize };
    let mut add = None;
    let mut favorite = None;
    for chunk in list.chunks(columns) {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing = vec2(4.0, 4.0);
            for entry in chunk {
                let size = if as_list { vec2(ui.available_width(), 26.0) } else { tile };
                let (rect, response) = ui.allocate_exact_size(size, Sense::click_and_drag());
                crate::widgets::name(&response, egui::Role::ListItem, &entry.label);
                let takes = target.takes(entry);
                let selected = app.ui.library_selected.as_deref() == Some(entry.id.as_str());
                if selected {
                    ui.painter().rect_filled(rect, 6.0, tokens.row_selected);
                } else if response.hovered() {
                    ui.painter().rect_filled(rect, 6.0, tokens.row_hover);
                }
                let fade = if takes { 1.0 } else { 0.4 };
                let image = if as_list {
                    Rect::from_min_size(rect.min + vec2(2.0, 2.0), vec2(22.0, 22.0))
                } else {
                    Rect::from_min_size(rect.min + vec2(5.0, 4.0), vec2(68.0, 68.0))
                };
                preview(app, ui, &ctx, entry, image, fade);
                let color = if entry.warning.is_some() { WARNING } else { tokens.text };
                let color = color.gamma_multiply(if takes { 1.0 } else { 0.55 });
                if as_list {
                    let galley = crate::widgets::elided(ui.painter(), entry.label.clone(), 11.5, color, rect.width() - 34.0);
                    ui.painter().galley(rect.left_center() + vec2(30.0, -galley.size().y * 0.5), galley, color);
                } else {
                    let galley = ui.painter().layout(entry.label.clone(), egui::FontId::proportional(10.5), color, tile.x - 4.0);
                    let pos = egui::pos2(rect.center().x - galley.size().x * 0.5, image.bottom() + 3.0);
                    ui.painter().with_clip_rect(rect).galley(pos, galley, color);
                }
                if app.ui.library_favorites.contains(&entry.id) {
                    ui.painter().text(
                        rect.right_top() + vec2(-8.0, 9.0),
                        egui::Align2::CENTER_CENTER,
                        icon::STAR,
                        egui::FontId::proportional(10.0),
                        tokens.accent,
                    );
                }

                let mut tip = format!("{}\n{}", entry.label, tr(&entry.category));
                if !entry.description.is_empty() {
                    tip.push_str(&format!("\n\n{}", entry.description));
                }
                if let Some(reason) = target.refusal(entry).or_else(|| entry.warning.clone()) {
                    tip.push_str(&format!("\n\n{reason}"));
                }
                let response = response.on_hover_text(tip);
                if response.clicked() || response.secondary_clicked() {
                    app.ui.library_selected = Some(entry.id.clone());
                }
                if takes && response.drag_started() {
                    drag(&ctx, entry);
                }
                if takes && response.double_clicked() {
                    add = Some((*entry).clone());
                }
                response.context_menu(|ui| {
                    if ui.add_enabled(takes, egui::Button::new(tr("Add to graph"))).clicked() {
                        add = Some((*entry).clone());
                        ui.close();
                    }
                    let label = if app.ui.library_favorites.contains(&entry.id) { tr("Remove from Favorites") } else { tr("Add to Favorites") };
                    if ui.button(label).clicked() {
                        favorite = Some(entry.id.clone());
                        ui.close();
                    }
                });
            }
        });
    }
    if let Some(id) = favorite {
        toggle_favorite(app, &id);
    }
    if let Some(entry) = add {
        add_entry(app, &entry);
    }
}

/// The entry's thumbnail: a cooked preview for native nodes, its category's icon otherwise
fn preview(app: &mut MaterialCraftApp, ui: &Ui, ctx: &egui::Context, entry: &LibraryEntry, rect: Rect, fade: f32) {
    let painter = ui.painter();
    let (texture, color) = match &entry.item {
        LibraryItem::Node { def } if entry.missing.is_none() && def != "dot" => (app.thumbs.library(ctx, def), header_color(def)),
        _ => (None, category_color(&entry.category)),
    };
    match texture {
        Some(texture) => {
            painter.image(texture, rect, Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(1.0, 1.0)), Color32::WHITE.gamma_multiply(fade));
        }
        None => {
            painter.rect_filled(rect, 4.0, color.gamma_multiply(0.7 * fade));
            let symbol = match &entry.item {
                LibraryItem::Comment => icon::CHAT_TEXT,
                LibraryItem::Frame => icon::FRAME_CORNERS,
                LibraryItem::Pin => icon::PUSH_PIN,
                LibraryItem::Node { def } if def == "dot" => icon::DOT_OUTLINE,
                _ => category_icon(&entry.category),
            };
            painter.text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                symbol,
                egui::FontId::proportional(rect.height() * 0.42),
                Color32::WHITE.gamma_multiply(fade),
            );
        }
    }
    let strip = Rect::from_min_size(rect.left_bottom() - vec2(0.0, 3.0), vec2(rect.width(), 3.0));
    painter.rect_filled(strip, 0.0, color.gamma_multiply(fade));
    if entry.missing.is_some() && rect.height() > 30.0 {
        let badge = Rect::from_center_size(rect.right_top() + vec2(-9.0, 9.0), vec2(14.0, 14.0));
        painter.circle_filled(badge.center(), 7.0, Color32::from_black_alpha(160));
        painter.text(badge.center(), egui::Align2::CENTER_CENTER, icon::CLOCK, egui::FontId::proportional(10.0), Color32::WHITE);
    }
}

/// Choose the preview accent for an entry category.
fn category_color(category: &str) -> Color32 {
    match category.split('/').next().unwrap_or("") {
        "Graph items" | "Atomic nodes" | "FxMap nodes" => Color32::from_rgb(0x5c, 0x62, 0x6e),
        "Function nodes" | "Functions" | "MCF functions" | "Values" => Color32::from_rgb(0x4a, 0x5a, 0x8a),
        "Texture generators" if category.ends_with("Patterns") => Color32::from_rgb(0xb3, 0x5a, 0x2c),
        "Texture generators" => Color32::from_rgb(0xa8, 0x3c, 0x34),
        "Filters" => Color32::from_rgb(0x3b, 0x7f, 0x4c),
        "Material filters" | "PBR materials" => Color32::from_rgb(0x3a, 0x66, 0xb0),
        "Mesh-based generators" => Color32::from_rgb(0x7a, 0x4f, 0x9e),
        "Spline & Path tools" => Color32::from_rgb(0x2f, 0x86, 0x8a),
        "3D view" => Color32::from_rgb(0x8a, 0x76, 0x2f),
        _ => Color32::from_rgb(0x5c, 0x62, 0x6e),
    }
}

/// Create the payload accepted by the destination canvas.
fn drag(ctx: &egui::Context, entry: &LibraryEntry) {
    match &entry.item {
        LibraryItem::Operation { operation } => egui::DragAndDrop::set_payload(ctx, FunctionLibraryDrag::Operation(operation.clone())),
        LibraryItem::FunctionInstance { package, graph } => {
            egui::DragAndDrop::set_payload(ctx, FunctionLibraryDrag::Instance { package: package.clone(), graph: graph.clone() })
        }
        LibraryItem::Pattern { .. } => {}
        _ => egui::DragAndDrop::set_payload(ctx, LibraryDrag(entry.id.clone())),
    }
}

/// Add `entry` to the open graph: native nodes next to the selection, the rest in the view
pub(crate) fn add_entry(app: &mut MaterialCraftApp, entry: &LibraryEntry) {
    if entry.canvas == LibraryCanvas::Any {
        let target = crate::annotation::target(app);
        crate::annotation::activate(app, target);
        let camera = crate::annotation::camera(app);
        let at = [camera.pan[0] + app.graph.view.width() * 0.5 / camera.zoom, camera.pan[1] + app.graph.view.height() * 0.5 / camera.zoom];
        if let Some(result) = app.run("library.add", serde_json::json!({"entry": entry.id, "at": at, "snapToGrid": app.ui.prefs.snap_to_grid})) {
            let id = result["result"]["id"].as_u64().and_then(|id| u32::try_from(id).ok());
            let selection = if app.session.annotation_selection.is_some() { Vec::new() } else { id.into_iter().collect() };
            if let Some(editor) = &mut app.function_editor {
                editor.selection = selection.clone();
                editor.selected_wires.clear();
            }
            if let Some(editor) = &mut app.pattern_editor {
                editor.selection = selection;
                editor.selected_wires.clear();
            }
        }
        return;
    }
    match &entry.item {
        LibraryItem::Operation { operation } => crate::function::library::add(app, &FunctionLibraryDrag::Operation(operation.clone())),
        LibraryItem::FunctionInstance { package, graph } => {
            crate::function::library::add(app, &FunctionLibraryDrag::Instance { package: package.clone(), graph: graph.clone() })
        }
        LibraryItem::Pattern { kind } => crate::pattern::library::add(app, *kind),
        LibraryItem::Node { def } => crate::graph::add_node_near_selection(app, def),
        _ => crate::graph::add_entry_in_view(app, &entry.id),
    }
}

#[cfg(test)]
#[path = "library.test.rs"]
mod tests;
