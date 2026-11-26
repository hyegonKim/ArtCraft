use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::{Receiver, channel};

use egui_kittest::kittest::Queryable as _;

use super::*;
use crate::menus;
use crate::state::UiState;
use materialcraft_engine::sample_session;

const COMMAND: &str = "ui.help.checkForUpdates";

/// A pins file that does not exist: the tests do not read the launcher's real choices
fn no_pins() -> std::path::PathBuf {
    std::env::temp_dir().join("materialcraft-updates-test-missing").join("pins.json")
}

fn app() -> MaterialCraftApp {
    let mut app = MaterialCraftApp::headless(sample_session(), UiState::default());
    app.updates_rt.updater = Updater::new(&crate::UPDATE_APP, "0.1.0").with_pins_path(no_pins());
    app
}

/// An app whose checks answer "nothing newer" instead of asking GitHub; the receiver hears each
/// check start, and the counter says how many did
fn app_with_source() -> (MaterialCraftApp, Receiver<()>, Arc<AtomicUsize>) {
    let mut app = app();
    let (tx, rx) = channel();
    let calls = Arc::new(AtomicUsize::new(0));
    let counted = calls.clone();
    let source: craft_update::updater::CheckSource = Arc::new(move || {
        counted.fetch_add(1, Ordering::SeqCst);
        let _ = tx.send(());
        Ok(None)
    });
    app.updates_rt.updater = Updater::new(&crate::UPDATE_APP, "0.1.0").with_pins_path(no_pins()).with_source(source);
    (app, rx, calls)
}

fn release() -> craft_update::Release {
    craft_update::Release {
        tag: "v99.0.0".into(),
        version: craft_update::parse_version("99.0.0"),
        name: "MaterialCraft 99.0.0".into(),
        html_url: "https://github.com/storytold/materialcraft/releases/tag/v99.0.0".into(),
        ..Default::default()
    }
}

pub(super) fn open_prompt(app: &mut MaterialCraftApp) {
    app.updates_rt.updater = Updater::new(&crate::UPDATE_APP, "0.1.0").with_pins_path(no_pins());
    app.updates_rt.updater.offer(release());
}

#[test]
fn the_update_app_names_this_repository() {
    assert_eq!(crate::UPDATE_APP.id, "materialcraft");
    assert_eq!(crate::UPDATE_APP.name, "MaterialCraft");
    assert_eq!(crate::UPDATE_APP.repo, "storytold/materialcraft");
    assert_eq!(crate::UPDATE_APP.bundle_id(), "ai.storyteller.materialcraft");
    assert!(crate::UPDATE_APP.icon_png.is_some());
}

#[test]
fn the_command_is_in_the_help_menu() {
    let item = menus::CATALOG.iter().find(|item| item.id == COMMAND).unwrap();
    assert_eq!(item.menu, "Help");
    assert_eq!(item.label, "Check for updates...");
    assert!(menus::implemented(COMMAND));
    assert!(menus::enabled(&app(), COMMAND));
}

#[test]
fn a_new_app_is_idle_and_does_not_check_at_start() {
    let rt = Runtime::default();
    assert!(!rt.startup_check);
    assert!(!rt.check_requested);
    assert!(!rt.updater.prompt_open());
    assert_eq!(rt.updater.status(), &Status::Idle);
}

/// One whole frame of the app; the first one only installs the fonts
fn frame(app: &mut MaterialCraftApp, ctx: &egui::Context) {
    let mut output = ctx.run_ui(egui::RawInput::default(), |ui| {
        app.step(ui.ctx());
        app.draw(ui);
    });
    output.textures_delta.clear();
}

#[test]
fn the_menu_command_starts_a_check_on_the_next_frame() {
    let (mut app, checks, _) = app_with_source();
    let ctx = egui::Context::default();
    frame(&mut app, &ctx);
    menus::invoke(&mut app, &ctx, COMMAND);
    assert!(app.updates_rt.check_requested);
    assert_eq!(app.updates_rt.updater.status(), &Status::Idle, "the check waits for the frame");

    frame(&mut app, &ctx);
    assert!(!app.updates_rt.check_requested);
    assert_ne!(app.updates_rt.updater.status(), &Status::Idle);
    // Dropping the app ends the wait with an error if no check was started
    drop(app);
    checks.recv().unwrap();
}

#[test]
fn the_check_at_start_runs_only_when_turned_on() {
    let (mut app, checks, calls) = app_with_source();
    let ctx = egui::Context::default();
    tick(&mut app, &ctx);
    assert_eq!(calls.load(Ordering::SeqCst), 0);

    app.set_startup_update_check(true);
    tick(&mut app, &ctx);
    assert_eq!(app.updates_rt.updater.status(), &Status::Idle, "the check at start is quiet");
    // Dropping the app ends the wait with an error if no check was started
    drop(app);
    checks.recv().unwrap();
    assert_eq!(calls.load(Ordering::SeqCst), 1);
}

#[test]
fn result_window_shows_every_status_but_idle() {
    let shown = [
        Status::Checking,
        Status::UpToDate,
        Status::Failed("no network".into()),
        Status::Installing,
        Status::Ready,
        Status::Managed,
        Status::Updated("0.2.0".into()),
        Status::InstallFailed("disk full".into()),
    ];
    for status in &shown {
        assert!(result_window_shows(status, false), "{status:?}");
        assert!(!result_window_shows(status, true), "{status:?} over the prompt");
    }
    assert!(!result_window_shows(&Status::Idle, false));
}

#[test]
fn an_offered_release_opens_the_prompt_over_the_window() {
    let mut app = app();
    app.updates_rt.updater.offer(release());
    assert!(blocks_input(&app));

    let mut harness = egui_kittest::Harness::builder().build_ui_state(|ui, app: &mut MaterialCraftApp| crate::windows::show(app, ui.ctx()), app);
    harness.run_steps(3);
    let _ = harness.get_by_label("LATER");
    assert!(harness.state().updates_rt.updater.prompt_open());
}

#[test]
fn a_requested_check_shows_its_progress() {
    let mut app = app();
    // The check stays running until this sender goes away at the end of the test
    let (release, wait) = channel::<()>();
    let wait = std::sync::Mutex::new(wait);
    let source: craft_update::updater::CheckSource = Arc::new(move || {
        if let Ok(wait) = wait.lock() {
            let _ = wait.recv();
        }
        Ok(None)
    });
    app.updates_rt.updater = Updater::new(&crate::UPDATE_APP, "0.1.0").with_pins_path(no_pins()).with_source(source);
    check(&mut app);
    tick(&mut app, &egui::Context::default());

    let mut harness = egui_kittest::Harness::builder().build_ui_state(|ui, app: &mut MaterialCraftApp| crate::windows::show(app, ui.ctx()), app);
    harness.run_steps(3);
    let _ = harness.get_by_label("Checking for updates…");
    drop(release);
}

/// Ctrl+Shift+E opens the export window when nothing holds the keyboard
fn press_export(app: &mut MaterialCraftApp) {
    let ctx = egui::Context::default();
    let modifiers = egui::Modifiers::COMMAND | egui::Modifiers::SHIFT;
    let input = egui::RawInput {
        events: vec![egui::Event::Key { key: egui::Key::E, physical_key: None, pressed: true, repeat: false, modifiers }],
        ..Default::default()
    };
    let mut output = ctx.run_ui(input, |ui| menus::shortcuts(app, ui.ctx()));
    output.textures_delta.clear();
}

#[test]
fn the_prompt_and_the_modal_results_hold_the_keyboard() {
    assert!(holds_keyboard(&Status::Idle, true));
    assert!(holds_keyboard(&Status::Checking, false));
    assert!(holds_keyboard(&Status::UpToDate, false));
    assert!(holds_keyboard(&Status::Failed("no network".into()), false));
    assert!(!holds_keyboard(&Status::Idle, false));
    assert!(!holds_keyboard(&Status::Updated("0.2.0".into()), false));
}

#[test]
fn shortcuts_wait_while_the_prompt_is_open() {
    let mut idle = app();
    press_export(&mut idle);
    assert!(idle.ui.windows.export, "the shortcut works without the prompt");

    let mut prompted = app();
    prompted.updates_rt.updater.offer(release());
    press_export(&mut prompted);
    assert!(!prompted.ui.windows.export);
}
