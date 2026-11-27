//! Window › Type & Tables › Story: the story-wide options.

use serde_json::{Value, json};

use crate::DesignApp;
use crate::widgets::{self, NumField};

/// The Story panel: Optical Margin Alignment for the stories of the text selection or the
/// selected text frames, and the type size its hang amounts are set for. Without text or a text
/// frame selected, the controls are disabled.
pub fn show(app: &mut DesignApp, ui: &mut egui::Ui) {
    let attrs = super::text_attrs(app);
    let story_id = attrs.as_ref().and_then(|a| a["story"].as_u64());
    let options = attrs.map(|a| a["storyOptions"].clone());
    let enabled = options.is_some();
    let options = options.unwrap_or(Value::Null);
    let lang = app.ui.language.clone();
    let tr = |s: &str| crate::i18n::tr(&lang, s).to_string();
    ui.add_enabled_ui(enabled, |ui| {
        ui.horizontal_wrapped(|ui| {
            // Stories that differ show unchecked; a click turns it on for all of them.
            let mut on = options["opticalMargin"].as_bool().unwrap_or(false);
            if widgets::checkbox(ui, &mut on, &tr("Optical Margin Alignment")).changed() {
                let _ = app.run("story.options", json!({"opticalMargin": on}));
            }
            let field = NumField::number("storyOpticalMarginSize", options["opticalMarginSize"].as_f64(), " pt", 2)
                .width(72.0)
                .range(designcraft_doc::OpticalMargin::MIN_SIZE, designcraft_doc::OpticalMargin::MAX_SIZE);
            if let Some(size) = crate::chrome::icon_field(ui, "font-size", &tr("Optical Margin Size"), field) {
                let _ = app.run("story.options", json!({"opticalMarginSize": size}));
            }
        });
        ui.separator();
        ui.label(egui::RichText::new(tr("Editorial Mode")).strong());
        let active_mode = if crate::story_editor::visible(app) { app.ui.editorial_view } else { crate::state::EditorialViewMode::Layout };
        ui.horizontal_wrapped(|ui| {
            if ui.selectable_label(active_mode == crate::state::EditorialViewMode::Layout, tr("Layout View")).clicked() {
                let _ = app.run("view.editorialMode", json!({"mode": "layout"}));
            }
            if ui.selectable_label(active_mode == crate::state::EditorialViewMode::Story, tr("Story View")).clicked() {
                let _ = app.run("view.editorialMode", json!({"mode": "story"}));
            }
            if ui.selectable_label(active_mode == crate::state::EditorialViewMode::Galley, tr("Galley View")).clicked() {
                let _ = app.run("view.editorialMode", json!({"mode": "galley"}));
            }
        });
        if let Some(sid) = story_id
            && let Some(st) = app.session.active()
        {
            let cs = app.session.cache.get(&st.doc, designcraft_doc::StoryId(sid), None);
            if let Some(cf) = designcraft_engine::compute_copyfit(&st.doc, &cs, designcraft_doc::StoryId(sid), None) {
                ui.label(
                    tr("Copyfit Info")
                        + &format!(": {} · {} · {:.1} / {:.1} pt", cf.word_count, cf.actual_lines, cf.frame_depth_used, cf.target_depth),
                );
                let detail = if cf.overset {
                    tr("Overset: +{lines} lines ({words} words)")
                        .replace("{lines}", &cf.overset_lines.to_string())
                        .replace("{words}", &cf.overset_words.to_string())
                } else {
                    tr("Copyfit: −{lines} lines ({words} words)")
                        .replace("{lines}", &cf.underset_lines.to_string())
                        .replace("{words}", &cf.underset_words.to_string())
                };
                ui.small(detail);
            }
        }
        ui.horizontal_wrapped(|ui| {
            if ui.small_button(tr("Import InCopy Story…")).clicked() {
                let _ = app.run("app.importIcml", json!({}));
            }
            if ui.small_button(tr("Export InCopy Story…")).clicked() {
                let p = story_id.map_or_else(|| json!({}), |sid| json!({"story": sid}));
                let _ = app.run("app.exportIcml", p);
            }
        });
    });
}

#[cfg(test)]
mod tests {
    use egui_kittest::{
        Harness,
        kittest::{NodeT, Queryable},
    };

    use super::*;

    fn harness(app: DesignApp) -> Harness<'static, DesignApp> {
        let mut h = Harness::builder().with_size(egui::vec2(260.0, 200.0)).build_ui_state(
            |ui, app: &mut DesignApp| {
                let id = egui::Id::unique("test_fonts");
                if ui.data(|d| d.get_temp::<bool>(id)).is_none() {
                    crate::theme::install_fonts(ui.ctx(), "");
                    ui.data_mut(|d| d.insert_temp(id, true));
                    return;
                }
                show(app, ui);
            },
            app,
        );
        h.run_steps(3);
        h
    }

    fn optical_margin(h: &Harness<'static, DesignApp>, story: u64) -> designcraft_doc::OpticalMargin {
        h.state().session.active().unwrap().doc.story(designcraft_doc::StoryId(story)).unwrap().optical_margin
    }

    fn size_field_text(h: &Harness<'static, DesignApp>) -> Option<String> {
        h.get_all_by_role(egui::accesskit::Role::TextInput).next().unwrap().value()
    }

    #[test]
    fn story_panel_sets_and_shows_the_storys_optical_margin() {
        let mut app = DesignApp::new(designcraft_engine::Session::new(), crate::Services::default());
        app.run("file.new", json!({})).unwrap();
        let story =
            app.run("frame.create", json!({"rect": [72, 72, 300, 200], "content": "text", "text": "“Hung”", "caret": true})).unwrap()["story"]
                .as_u64()
                .unwrap();
        app.run("edit.selectAll", json!({})).unwrap();
        let mut h = harness(app);
        assert_eq!(size_field_text(&h).as_deref(), Some("12 pt"));
        h.get_by_label("Optical Margin Alignment").click();
        h.run_steps(3);
        assert!(optical_margin(&h, story).on);
        assert_eq!(h.get_by_label("Optical Margin Alignment").accesskit_node().toggled(), Some(egui::accesskit::Toggled::True));
        h.get_all_by_role(egui::accesskit::Role::TextInput).next().unwrap().click();
        h.run_steps(2);
        h.event(egui::Event::Key { key: egui::Key::A, physical_key: None, pressed: true, repeat: false, modifiers: egui::Modifiers::COMMAND });
        h.event(egui::Event::Text("18".into()));
        h.key_press(egui::Key::Enter);
        h.run_steps(3);
        assert_eq!(optical_margin(&h, story), designcraft_doc::OpticalMargin { on: true, size: 18.0 });
        // A value set elsewhere shows in the panel.
        h.state_mut().run("story.options", json!({"opticalMarginSize": 30})).unwrap();
        h.run_steps(3);
        assert_eq!(size_field_text(&h).as_deref(), Some("30 pt"));
    }

    #[test]
    fn story_panel_is_disabled_without_text() {
        let mut app = DesignApp::new(designcraft_engine::Session::new(), crate::Services::default());
        app.run("file.new", json!({})).unwrap();
        let mut h = harness(app);
        assert!(h.get_by_label("Optical Margin Alignment").accesskit_node().is_disabled());
        h.get_by_label("Optical Margin Alignment").click();
        h.run_steps(3);
        assert_eq!(h.get_by_label("Optical Margin Alignment").accesskit_node().toggled(), Some(egui::accesskit::Toggled::False));
    }
}
