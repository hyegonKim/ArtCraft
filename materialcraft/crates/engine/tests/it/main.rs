//! One integration-test executable avoids linking the same dependencies for every feature.
//! Keep feature tests in their own modules; the standalone session.rs target remains available.

mod annotations;
mod atomics;
mod delete_relink;
mod embedded_edit;
mod explorer;
mod export_names;
mod exposed_inputs;
mod function_graph;
mod function_instances;
mod function_library;
mod function_math;
mod function_output;
mod graph_alignment;
mod graph_edit;
mod graph_interaction;
mod graph_properties;
mod linked_instances;
mod linked_resources;
mod material_preview;
mod node_library;
mod normal_maps;
mod open_documents;
mod optimization_perf;
mod parameter_functions;
mod pattern_graph;
mod perf;
mod reset_drivers;
mod sbs_functions;
mod subgraph_seed;
mod system_variables;
mod view_settings;
mod wire_swap_types;
