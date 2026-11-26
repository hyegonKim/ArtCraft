/// A row of an Explorer context menu
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum ExplorerRow {
    /// A label, the action id (empty: not available yet) and the shortcut shown beside it
    Item(&'static str, &'static str, Option<&'static str>),
    /// A submenu of (label, action id); an empty id is not available yet
    Sub(&'static str, &'static [(&'static str, &'static str)]),
    Separator,
}
