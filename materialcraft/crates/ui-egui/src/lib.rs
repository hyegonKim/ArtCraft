#![forbid(unsafe_code)]
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

mod annotation;
pub mod control;
pub mod cook;
pub mod docking;
pub mod explorer;
pub mod export;
pub mod fonts;
pub mod function;
pub mod graph;
mod graph_drag_payload;
mod history;
pub mod i18n;
pub mod levels_editor;
pub mod library;
mod material_preview_menu;
mod menu_item;
pub mod menus;
mod preview_resolution;
#[path = "property/mod.rs"]
pub mod properties;
mod shell;
pub mod state;
pub mod theme;
pub mod thumbs;
pub mod toolbar;
pub mod unsaved;
mod value_input_editor;
pub mod view2d;
pub mod view3d;
pub mod web_files;
pub mod widgets;
pub mod windows;

mod material_craft_app;
#[cfg(not(target_arch = "wasm32"))]
mod native_dialogs;
pub mod pattern;
mod toast;
#[cfg(not(target_arch = "wasm32"))]
pub mod updates_ui;
mod user_directory;
mod value_editor;
#[cfg(not(target_arch = "wasm32"))]
mod window_controls;

/// This app as the updater knows it: its release repository, and the icon and accent color the
/// update prompt shows
#[cfg(not(target_arch = "wasm32"))]
pub const UPDATE_APP: craft_update::app::App = craft_update::app::App {
    icon_png: Some(include_bytes!("../../../assets/app-icon/hicolor/128x128/apps/ai.storyteller.materialcraft.png")),
    accent: Some([0x17, 0x8a, 0x9c]),
    ..craft_update::app::App::new("materialcraft", "MaterialCraft", "Procedural material design", "storytold/materialcraft")
};

pub use control::ControlRequest;
pub use material_craft_app::MaterialCraftApp;
pub use materialcraft_engine::eval::now_ms;
pub use state::UiState;
pub use toast::Toast;
#[cfg(not(target_arch = "wasm32"))]
pub use user_directory::user_dir;
