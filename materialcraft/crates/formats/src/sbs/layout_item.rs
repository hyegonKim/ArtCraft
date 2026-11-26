/// One node placed in a layout frame
#[derive(Clone, Debug, PartialEq)]
pub enum LayoutItem {
    /// An atomic SBS filter by its SBS name, with the MaterialCraft node it reads as (`None`
    /// when MaterialCraft has no counterpart yet)
    Atomic { filter: String, node: Option<&'static str> },
    /// A graph input (`input_color`, `input_grayscale`, `input_value`) or `output`
    Node(&'static str),
    /// An instance of graph `graph` from the library package `package`
    Instance { package: String, graph: String, outputs: Vec<String> },
    /// A native function operation, e.g. `add`
    Operation(String),
    /// An instance of function graph `graph` from the library package `package`
    FunctionInstance { package: String, graph: String },
    /// A bitmap showing one of the layout package's own resources, e.g. an environment image
    Bitmap { resource: String },
}
