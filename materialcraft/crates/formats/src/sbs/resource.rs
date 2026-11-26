use super::{embedded::encode_embedded, sbs_writer::SbsWriter, xml::xml_escape};
use crate::Result;
use materialcraft_core::Resource;
use std::fmt::Write;

pub(super) fn write(content: &mut String, resource: &Resource, writer: &mut SbsWriter) -> Result<()> {
    let svg = resource.file.to_ascii_lowercase().ends_with(".svg");
    let ext = resource.file.rsplit_once('.').map_or("png", |(_, e)| e);
    let source = if !resource.source.is_empty() && resource.bytes.is_empty() {
        format!("<externalcopy><filename v=\"{}\"/></externalcopy>", xml_escape(&resource.source))
    } else {
        format!("<binembedded><datalength v=\"{}\"/><strdata v=\"{}\"/></binembedded>", resource.bytes.len(), encode_embedded(resource.bytes.as_slice())?)
    };
    let _ = write!(
        content,
        "<resource><identifier v=\"{}\"/><uid v=\"{}\"/><type v=\"{}\"/><format v=\"{}\"/><filepath v=\"\"/><source>{source}</source></resource>",
        xml_escape(&resource.name),
        writer.next_uid(),
        if svg { "svg" } else { "bitmap" },
        xml_escape(ext)
    );

    Ok(())
}
