//! One integration-test executable avoids linking the same dependencies for every feature.
//! Keep snapshot.rs standalone; language_switching.rs needs process isolation for global locale state.

mod clipboard_shortcuts;
mod dialogs;
mod export_window;
mod graph_properties;
mod interface_scale;
mod normal_maps;
mod numeric_input;
mod preview_resolution;
mod resize;
mod resolution;
mod search;
mod slider_round_values;
mod unsaved_changes;
