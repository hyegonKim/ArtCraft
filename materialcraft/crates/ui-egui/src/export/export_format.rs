/// A file format the Export window offers
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ExportFormat {
    pub extension: &'static str,
    pub label: &'static str,
    /// MaterialCraft can write it today
    pub available: bool,
}

/// Formats listed before MaterialCraft can write them, so their exporter can be connected later
const PLANNED: &[(&str, &str)] = &[("dds", "DirectDraw Surface")];

impl ExportFormat {
    /// The engine's export formats, then the planned ones
    pub fn all() -> impl Iterator<Item = ExportFormat> {
        let available = materialcraft_engine::formats::EXPORT_FORMATS.iter().map(|&(extension, label)| ExportFormat { extension, label, available: true });
        let planned = PLANNED.iter().map(|&(extension, label)| ExportFormat { extension, label, available: false });
        available.chain(planned)
    }

    /// The format with this extension, if the window offers it
    pub fn find(extension: &str) -> Option<ExportFormat> {
        ExportFormat::all().find(|format| format.extension == extension)
    }

    /// "PNG (*.png)"
    pub fn title(self) -> String {
        format!("{} (*.{})", self.label, self.extension)
    }
}
