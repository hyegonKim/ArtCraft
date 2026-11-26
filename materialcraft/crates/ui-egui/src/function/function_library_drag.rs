/// A library function being dragged onto a Function canvas
#[derive(Clone, Debug)]
pub(crate) enum FunctionLibraryDrag {
    Operation(String),
    Instance { package: String, graph: String },
}
