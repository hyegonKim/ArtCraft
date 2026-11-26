use crate::menus;

/// What the right-clicked item allows right now
#[derive(Clone, Copy, Debug, Default)]
pub(super) struct ExplorerMenuState {
    /// The Explorer clipboard holds something to paste
    pub paste: bool,
    /// The package has a file on disk (reload, show in folder)
    pub saved: bool,
    /// The package links other package files
    pub dependencies: bool,
    /// The graph (or, for a package, one of its graphs) has image outputs
    pub outputs: bool,
}

impl ExplorerMenuState {
    /// Check the selected item supports this context-menu command.
    pub(super) fn enabled(&self, command: &str) -> bool {
        match command {
            "" => false,
            "explorer.paste" => self.paste,
            "file.reload" | menus::EXPLORER_SHOW_IN_FOLDER => self.saved,
            menus::EXPLORER_OPEN_DEPENDENCIES => self.dependencies,
            menus::EXPLORER_COMPUTE | menus::EXPLORER_VIEW_OUTPUTS | menus::EXPLORER_EXPORT_OUTPUTS | menus::EXPLORER_EXPORT_MATERIAL => self.outputs,
            _ => true,
        }
    }
}
