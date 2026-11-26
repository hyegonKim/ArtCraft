use super::{
    constant::SELF_DEPENDENCY,
    sbs_writer::SbsWriter,
    xml::{child_element, child_value, children_named, xml_escape},
};
use materialcraft_core::{GraphInstance, GraphPort, Value};
use roxmltree::Node as XmlNode;
use std::fmt::Write;

pub(super) fn read(element: XmlNode<'_, '_>, value: Option<&Value>, type_code: u32) -> Result<GraphInstance, String> {
    let Some(Value::Text(reference)) = value else { return Err("function instance has no source reference".into()) };
    let (package, graph) = source_reference(element, reference)?;

    let inputs: Vec<_> = child_element(element, "materialcraftInterface")
        .into_iter()
        .flat_map(|interface| children_named(interface, "input"))
        .take(4097)
        .map(|port| GraphPort {
            identifier: child_value(port, "identifier").unwrap_or("").into(),
            type_code: child_value(port, "type").and_then(|value| value.parse().ok()).unwrap_or(0),
        })
        .collect();
    if inputs.len() > 4096 {
        return Err("Function instance exceeds the 4096-input limit".into());
    }
    let names: std::collections::BTreeSet<_> = inputs.iter().map(|port| &port.identifier).collect();
    if names.len() != inputs.len() {
        return Err("Duplicate Function input identifiers".into());
    }
    Ok(GraphInstance {
        label: String::new(),
        defaults: Default::default(),
        package,
        graph,
        inputs,
        outputs: vec![GraphPort { identifier: "output".into(), type_code }],
    })
}

fn source_reference(element: XmlNode<'_, '_>, reference: &str) -> Result<(String, String), String> {
    if let Some(reference) = reference.strip_prefix("pkg:///") {
        let (graph, query) = reference.split_once('?').unwrap_or((reference, ""));
        let dependency = query.split('&').find_map(|part| part.strip_prefix("dependency="));
        let package = dependency
            .map(|uid| {
                child_element(element.document().root_element(), "dependencies")
                    .into_iter()
                    .flat_map(|dependencies| children_named(dependencies, "dependency"))
                    .find(|dependency| child_value(*dependency, "uid") == Some(uid))
                    .and_then(|dependency| child_value(dependency, "filename"))
                    .map(str::to_string)
                    .unwrap_or_else(|| format!("missing://dependency/{uid}"))
            })
            .unwrap_or_default();

        return Ok((if package == SELF_DEPENDENCY { String::new() } else { package }, graph.into()));
    }

    if let Some((package, graph)) = reference.split_once(".sbs/") {
        return Ok((format!("{package}.sbs"), graph.into()));
    }

    Err("invalid function source reference".into())
}

pub(super) fn write(instance: &GraphInstance, writer: &mut SbsWriter) -> String {
    let reference = if instance.package.is_empty() {
        format!("pkg:///{}", instance.graph)
    } else {
        format!("pkg:///{}?dependency={}", instance.graph, writer.external_package_uid(&instance.package))
    };
    let mut xml = format!(
        "<funcDatas><funcData><name v=\"instance\"/><constantValue><constantValueString v=\"{}\"/></constantValue></funcData></funcDatas>",
        xml_escape(&reference)
    );
    // Cache the editable interface so a missing source cannot erase unconnected pins
    xml.push_str("<materialcraftInterface>");
    for port in &instance.inputs {
        let _ = write!(xml, "<input><identifier v=\"{}\"/><type v=\"{}\"/></input>", xml_escape(&port.identifier), port.type_code);
    }
    xml.push_str("</materialcraftInterface>");
    xml
}
