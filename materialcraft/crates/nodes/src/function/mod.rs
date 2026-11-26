//! Native function primitives, compilation and bounded evaluation

mod arithmetic;
mod catalog;
mod compile;
mod evaluation_context;
mod evaluation_state;
mod evaluation_step;
mod function_call;
mod function_definition;
mod function_scope;
mod inferred_type;
mod instruction;
mod node_interface;
mod node_types;
mod operation;
mod output;
mod processor;
mod program;
mod resolve_instances;
mod system_variable;
pub mod value_type;
mod vector;
mod warnings;

pub use catalog::{FUNCTIONS, default_value, definition};
pub use compile::compile;
pub use evaluation_context::EvaluationContext;
pub use evaluation_state::EvaluationState;
pub use function_definition::FunctionDefinition;
pub use function_scope::FunctionScope;
pub use node_interface::{graph_ports, input_identifiers, node_label};
pub use node_types::node_types;
pub use output::{can_be_output, output_node};
pub(crate) use processor::pixel_processor;
pub use program::Program;
pub use resolve_instances::resolve_instances;
pub use system_variable::SystemVariable;
pub use warnings::warnings;

#[cfg(test)]
mod tests;

#[cfg(test)]
#[path = "system_variable.test.rs"]
mod system_variable_tests;
