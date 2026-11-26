use egui::{Button, Event, Key, Ui};

use crate::MaterialCraftApp;
use crate::i18n::tr;
use crate::shell::modality::Modality;

pub(crate) fn show(app: &MaterialCraftApp, ui: &mut Ui, entries: impl IntoIterator<Item = (&'static str, &'static str, &'static str)>) -> Option<&'static str> {
    let blocked = Modality::capture(app, ui.ctx()).is_blocked();
    let filter_id = ui.make_persistent_id("add-node-filter");
    let frame = ui.ctx().cumulative_frame_nr();
    let previous = ui.ctx().data_mut(|data| data.get_temp::<(u64, Option<char>)>(filter_id));
    let mut filter = previous.filter(|(last_frame, _)| frame <= last_frame.saturating_add(1)).and_then(|(_, letter)| letter);

    if !blocked {
        ui.input(|input| {
            for event in &input.events {
                let Event::Key { key, pressed: true, modifiers, .. } = event else { continue };
                if modifiers.ctrl || modifiers.alt || modifiers.command {
                    continue;
                }

                if *key == Key::Backspace {
                    filter = None;
                } else if key.name().len() == 1 {
                    filter = key.name().chars().next().map(|letter| letter.to_ascii_lowercase());
                }
            }
        });
    }

    let mut selected = None;
    let enter = filter.is_some() && ui.input(|input| input.key_pressed(Key::Enter));
    ui.set_min_width(220.0);
    let height = (ui.ctx().content_rect().height() - 16.0).max(100.0);
    ui.set_max_height(height);
    egui::ScrollArea::vertical().max_height(height).show(ui, |ui| {
        let mut previous_category = None;
        for (id, label, category) in entries {
            if previous_category != Some(category) {
                if previous_category.is_some() {
                    ui.separator();
                }
                previous_category = Some(category);
            }

            let label = tr(label);
            let enabled = !blocked && filter.is_none_or(|letter| label.chars().next().is_some_and(|first| first.eq_ignore_ascii_case(&letter)));
            if ui.add_enabled(enabled, Button::new(label)).clicked() || (enter && enabled && selected.is_none()) {
                selected = Some(id);
            }
        }
    });

    if selected.is_some() {
        ui.ctx().data_mut(|data| data.remove::<(u64, Option<char>)>(filter_id));
        ui.close();
    } else {
        ui.ctx().data_mut(|data| data.insert_temp(filter_id, (frame, filter)));
    }
    selected
}

#[cfg(test)]
mod tests {
    use egui_kittest::{
        Harness,
        kittest::{By, NodeT, Queryable},
    };

    fn app() -> crate::MaterialCraftApp {
        crate::MaterialCraftApp::headless(materialcraft_engine::sample_session(), crate::UiState::default())
    }

    #[test]
    fn entries_are_disabled_behind_a_prompt() {
        let mut app = app();
        app.ui.windows.unsaved = Some(crate::unsaved::Guarded::Quit);
        let mut harness = Harness::builder().build_ui(move |ui| {
            super::show(&app, ui, [("svg", "SVG", "Atomic nodes")]);
        });
        harness.run_steps(3);
        assert!(harness.get(By::new().role(egui::accesskit::Role::Button).label("SVG")).accesskit_node().is_disabled());
    }

    #[test]
    fn ordered_entries_keep_the_catalog_sequence() {
        let app = app();
        let mut harness = Harness::builder().build_ui(move |ui| {
            super::show(&app, ui, [("second", "Second", "Native"), ("first", "First", "Native"), ("third", "Third", "Native")]);
        });
        harness.run_steps(3);
        let rows: Vec<_> = ["Second", "First", "Third"]
            .iter()
            .map(|label| harness.get(By::new().role(egui::accesskit::Role::Button).label(label)).rect().center().y)
            .collect();

        assert!(rows.windows(2).all(|pair| pair[0] < pair[1]), "catalog order must survive rendering: {rows:?}");
    }

    #[test]
    fn letters_typed_behind_a_prompt_do_not_become_the_filter() {
        let mut harness = Harness::builder().build_ui_state(
            |ui, app: &mut crate::MaterialCraftApp| {
                super::show(app, ui, [("svg", "SVG", "Atomic nodes"), ("grayscale", "Grayscale conversion", "Atomic nodes")]);
            },
            app(),
        );
        harness.run_steps(3);
        harness.state_mut().ui.windows.unsaved = Some(crate::unsaved::Guarded::Quit);
        harness.run_steps(3);
        harness.key_press(egui::Key::S);
        harness.run_steps(3);
        harness.state_mut().ui.windows.unsaved = None;
        harness.run_steps(3);
        assert!(!harness.get(By::new().role(egui::accesskit::Role::Button).label("Grayscale conversion")).accesskit_node().is_disabled());
    }

    #[test]
    fn letters_disable_other_initials_without_removing_entries() {
        let app = app();
        let mut harness = Harness::builder().build_ui_state(
            move |ui, selected: &mut Option<&'static str>| {
                if let Some(id) = super::show(
                    &app,
                    ui,
                    [("svg", "SVG", "Atomic nodes"), ("sharpen", "Sharpen", "Atomic nodes"), ("grayscale", "Grayscale conversion", "Atomic nodes")],
                ) {
                    *selected = Some(id);
                }
            },
            None,
        );
        harness.run_steps(3);
        harness.key_press(egui::Key::S);
        harness.run_steps(3);

        let button = |label| By::new().role(egui::accesskit::Role::Button).label(label);
        assert!(!harness.get(button("SVG")).accesskit_node().is_disabled());
        assert!(!harness.get(button("Sharpen")).accesskit_node().is_disabled());
        assert!(harness.get(button("Grayscale conversion")).accesskit_node().is_disabled());

        harness.key_press(egui::Key::G);
        harness.run_steps(3);
        assert!(harness.get(button("SVG")).accesskit_node().is_disabled());
        assert!(!harness.get(button("Grayscale conversion")).accesskit_node().is_disabled());

        harness.key_press(egui::Key::Backspace);
        harness.run_steps(3);
        for label in ["SVG", "Sharpen", "Grayscale conversion"] {
            assert!(!harness.get(button(label)).accesskit_node().is_disabled());
        }

        harness.key_press(egui::Key::S);
        harness.key_press(egui::Key::Enter);
        harness.step();
        assert_eq!(*harness.state(), Some("svg"));
    }
}
