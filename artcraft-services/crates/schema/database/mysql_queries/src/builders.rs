
/// This is simply a semantic / literate way of saying
/// an Optional should be required. There should be a
/// better way to actually enforce this, though.
pub (crate) type RequiredOption<T> = Option<T>;
