/// The caller determines which context-dependent variables a function can read.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum FunctionScope {
    #[default]
    Node,
    PixelProcessor,
    FxMap,
}
