//! Entry point for opening separately installed Craft Suite desktop tools.

use std::path::PathBuf;

use egui::Ui;

use crate::pages::common::Env;
use crate::pages::suite_registry::{self, SUITE_APPS};
use crate::theme;
use crate::ui::widgets;

pub fn ui(ui: &mut Ui, env: &mut Env<'_>) {
  let current_exe = match std::env::current_exe() {
    Ok(path) => path,
    Err(err) => {
      ui.label(format!("Couldn't locate Craft Suite apps: {err}"));
      return;
    },
  };
  let override_dir = std::env::var_os("ARTCRAFT_SUITE_BIN_DIR").map(PathBuf::from);
  egui::ScrollArea::vertical().id_salt("craft_suite_apps").show(ui, |ui| {
    ui.add_space(20.0);
    ui.label(widgets::heading("Craft Suite", 28.0));
    ui.label("Open creative tools in a separate window.");
    ui.add_space(16.0);
    for app in SUITE_APPS {
      let available = suite_registry::resolve_binary(app.id, override_dir.as_deref(), &current_exe);
      egui::Frame::new().fill(theme::CONTROLS).inner_margin(egui::Margin::same(12)).corner_radius(theme::RADIUS).show(ui, |ui| {
        ui.set_min_width(ui.available_width());
        ui.horizontal(|ui| {
          ui.vertical(|ui| {
            ui.strong(app.name);
            ui.label(egui::RichText::new(app.domain).color(theme::MUTED));
          });
          ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let response = ui.add_enabled(available.is_ok(), egui::Button::new("Open"));
            if response.clicked() {
              match suite_registry::launch(app.id, override_dir.as_deref(), &current_exe) {
                Ok(()) => env.toasts.success(format!("Starting {}", app.name)),
                Err(err) => env.toasts.error(err),
              }
            }
            if let Err(reason) = available {
              response.on_hover_text(&reason);
              ui.label(egui::RichText::new("Unavailable").color(theme::MUTED)).on_hover_text(reason);
            }
          });
        });
      });
      ui.add_space(6.0);
    }
  });
}
