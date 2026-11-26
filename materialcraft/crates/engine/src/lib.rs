#![forbid(unsafe_code)]
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

#[cfg(test)]
extern crate self as materialcraft_engine;

mod document_path;
mod evaluation_environment_commands;
mod explorer_clipboard;
mod explorer_clipboard_commands;
mod folder_commands;
mod folder_path;
mod folder_property_commands;
pub use explorer_clipboard::ExplorerClipboard;
mod function_library;
mod function_library_commands;
pub mod node_library;
mod node_library_commands;
pub use node_library::{LibraryCanvas, LibraryEntry, LibraryItem, NodeLibrary};
mod function_variable;
mod function_variable_commands;
mod function_variables;
pub use function_variable::FunctionVariable;
mod document_history;
mod document_history_commands;
mod graph_instance_commands;
mod graph_references;
mod linked_files;
mod linked_packages;
mod linked_resource;
mod material_preview;
mod material_preview_commands;
mod material_source;
mod node_info_commands;
mod open_document;
mod package_commands;
mod package_file_commands;
mod preview_cook;
mod resource_commands;
pub use material_preview::MaterialPreview;
pub use material_source::MaterialSource;
pub use open_document::OpenDocument;
pub use preview_cook::PreviewCook;

mod alignment_commands;
mod embedded_clipboard;
mod graph_validation;
pub mod selection_commands;
mod selection_target;
pub use embedded_clipboard::EmbeddedClipboard;
pub use selection_target::SelectionTarget;
pub mod commands;
pub mod edit_cmds;
pub mod file_cmds;
pub mod graph_cmds;
mod graph_property_commands;
mod graph_target;
mod history_prefs;
pub use graph_target::GraphTarget;
pub use history_prefs::HistoryPrefs;
pub mod graph_edit;
pub mod graph_grid;
pub mod inspect;
pub mod material_cmds;
pub mod node_cmds;
pub mod samples;
pub mod session;
mod value_input_commands;
pub mod view_cmds;
pub mod wire_cmds;
pub mod wire_swap;

pub use commands::{CommandSpec, command_specs, execute, find};
pub use session::{CameraPrefs, Session, View2d, View3d, WARNING_PREFIX};
mod clip;
mod color_space;
mod connection_commands;
mod export_request;
mod export_size;
pub use clip::Clip;
pub use color_space::ColorSpace;
pub use export_request::ExportRequest;
pub use export_size::ExportSize;

pub use materialcraft_core as core;
pub use materialcraft_eval as eval;
pub use materialcraft_formats as formats;
pub use materialcraft_image as image;
pub use materialcraft_nodes as nodes;

mod engine_error;
pub use engine_error::EngineError;

pub type Result<T> = std::result::Result<T, EngineError>;

/// Version string shown in About, `--version` and window titles
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

mod startup;
pub use startup::{new_session, sample_session};

mod atomic_sample;
mod atomic_sample_commands;
pub use atomic_sample::atomic_sample;

mod material_options;
pub use material_options::MaterialOptions;

mod exported;
pub use exported::Exported;

mod annotation_commands;
mod annotation_selection;
mod annotation_target;
pub use annotation_target::AnnotationTarget;
mod comment_commands;
mod frame_commands;
mod frame_contents;
mod function_commands;
mod node_graph_commands;
mod pattern_graph_commands;
mod pin_commands;
mod snapshot;
pub use annotation_selection::AnnotationSelection;
pub use frame_contents::FrameContents;

mod embedded_edit_commands;
mod embedded_wire_commands;
mod function_edit;
mod function_graph_target;
mod function_info_commands;
pub mod function_inspection;
pub mod function_output;
mod parameter_function_commands;
mod parameter_target;
mod pattern_edit;
pub use function_edit::FunctionEdit;
pub use function_graph_target::FunctionGraphTarget;
pub use pattern_edit::PatternEdit;

mod exposed_input_commands;
