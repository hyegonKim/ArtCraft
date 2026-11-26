use super::xml::{child_value, xml_escape};
use materialcraft_core::{Attributes, Graph};
use roxmltree::Node;
use std::fmt::Write;

pub(super) fn read(element: Node<'_, '_>) -> Attributes {
    let text = |tag| child_value(element, tag).unwrap_or_default().to_string();
    Attributes {
        label: text("label"),
        category: text("category"),
        description: text("description"),
        author: text("author"),
        author_url: text("authorURL"),
        tags: text("tags"),
        group: text("group"),
        user_data: text("userdata"),
    }
}

pub(super) fn write(output: &mut String, graph: &Graph) {
    let attributes = &graph.attributes;
    output.push_str("<attributes>");
    for (tag, value) in [
        ("category", &attributes.category),
        ("description", &attributes.description),
        ("author", &attributes.author),
        ("tags", &attributes.tags),
        ("label", &attributes.label),
        ("authorURL", &attributes.author_url),
        ("group", &attributes.group),
        ("userdata", &attributes.user_data),
    ] {
        if !value.is_empty() {
            let _ = write!(output, "<{tag} v=\"{}\"/>", xml_escape(value));
        }
    }
    if graph.function.is_none() && graph.physical_size != [0.0; 3] {
        let [x, y, z] = graph.physical_size;
        let _ = write!(output, "<physicalSize v=\"{x} {y} {z}\"/>");
    }
    output.push_str("</attributes>");
}
