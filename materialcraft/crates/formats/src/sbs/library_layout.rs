//! Library layouts. An SBS package can lay out a node library: its graphs group nodes in titled
//! frames ("Filters - Blurs"), and a parameter function groups function nodes the same way. This
//! reads the frames and the nodes inside each one; it does not cook anything. The engine decides
//! which frame titles are library categories.

use super::{
    LayoutFrame, LayoutItem,
    constant::MAX_XML_NODES,
    filter_map::FILTERS,
    xml::{child_element, child_value, children_named, parse_floats},
};
use crate::{FormatError, Result};
use roxmltree::Node as XmlNode;
use std::collections::HashMap;

/// Bounds for hostile layouts: frames and placed nodes kept per file
const MAX_FRAMES: usize = 4096;
const MAX_ITEMS: usize = 65_536;

/// The frames of every graph and parameter function in `bytes`, in document order
pub fn read_layout(bytes: &[u8], uri: &str) -> Result<Vec<LayoutFrame>> {
    let text = std::str::from_utf8(bytes).map_err(|e| FormatError::BadPackage(format!("the SBS file is not UTF-8 text: {e}")))?;
    let text = text.trim_start_matches('\u{feff}');
    let options = roxmltree::ParsingOptions { allow_dtd: false, nodes_limit: MAX_XML_NODES };
    let doc = roxmltree::Document::parse_with_options(text, options).map_err(|e| FormatError::BadPackage(format!("the SBS file is not valid XML: {e}")))?;
    let root = doc.root_element();
    if root.tag_name().name() != "package" {
        return Err(FormatError::BadPackage("not an SBS package (the root element is not <package>)".into()));
    }

    let mut dependencies = HashMap::new();
    for dependency in child_element(root, "dependencies").into_iter().flat_map(|d| children_named(d, "dependency")) {
        if let (Some(uid), Some(file)) = (child_value(dependency, "uid"), child_value(dependency, "filename")) {
            let file = if file == "?himself" { uri.to_string() } else { super::library::resolve(uri, file).map_err(FormatError::BadPackage)? };
            dependencies.insert(uid, file);
        }
    }

    let mut frames = Vec::new();
    let mut items = 0;
    for scope in root.descendants().filter(|node| node.is_element()) {
        let function = match scope.tag_name().name() {
            "graph" if child_element(scope, "compNodes").is_some() => false,
            _ if child_element(scope, "paramNodes").is_some() => true,
            _ => continue,
        };
        let mut scope_frames = scope_frames(scope);
        if scope_frames.is_empty() {
            continue;
        }

        let nodes: Vec<(XmlNode, [f32; 2])> = if function {
            child_element(scope, "paramNodes").into_iter().flat_map(|n| children_named(n, "paramNode")).filter_map(|n| Some((n, position(n)?))).collect()
        } else {
            child_element(scope, "compNodes").into_iter().flat_map(|n| children_named(n, "compNode")).filter_map(|n| Some((n, position(n)?))).collect()
        };
        let mut placed: Vec<(usize, [f32; 2], LayoutItem)> = Vec::new();
        for (node, at) in nodes {
            if items >= MAX_ITEMS {
                break;
            }

            // The smallest frame holding the node wins, so nested frames keep their own nodes
            let Some(frame) = scope_frames
                .iter()
                .enumerate()
                .filter(|(_, (_, rect))| at[0] >= rect[0] && at[1] >= rect[1] && at[0] <= rect[0] + rect[2] && at[1] <= rect[1] + rect[3])
                .min_by(|a, b| (a.1.1[2] * a.1.1[3]).total_cmp(&(b.1.1[2] * b.1.1[3])))
                .map(|(index, _)| index)
            else {
                continue;
            };

            let item = if function { function_item(node, &dependencies) } else { graph_item(scope, node, &dependencies) };
            if let Some(item) = item {
                placed.push((frame, at, item));
                items += 1;
            }
        }
        placed.sort_by(|a, b| a.0.cmp(&b.0).then(a.1[1].total_cmp(&b.1[1])).then(a.1[0].total_cmp(&b.1[0])));
        let mut grouped: Vec<Vec<LayoutItem>> = vec![Vec::new(); scope_frames.len()];
        for (frame, _, item) in placed {
            if let Some(list) = grouped.get_mut(frame) {
                list.push(item);
            }
        }
        for ((title, _), items) in scope_frames.drain(..).zip(grouped) {
            if frames.len() >= MAX_FRAMES {
                break;
            }

            frames.push(LayoutFrame { title, function, items });
        }
    }
    Ok(frames)
}

/// The titled frames of a graph or function: (title, [x, y, width, height])
fn scope_frames(scope: XmlNode) -> Vec<(String, [f32; 4])> {
    child_element(scope, "GUIObjects")
        .into_iter()
        .flat_map(|objects| children_named(objects, "GUIObject"))
        .filter(|object| child_value(*object, "type") == Some("COMMENT"))
        .filter_map(|object| {
            let title = child_value(object, "title")?.trim();
            let layout = child_element(object, "GUILayout")?;
            let at = parse_floats(child_value(layout, "gpos")?);
            let size = parse_floats(child_value(layout, "size")?);
            let rect = [*at.first()?, *at.get(1)?, *size.first()?, *size.get(1)?];
            (!title.is_empty() && rect[2] > 0.0 && rect[3] > 0.0).then(|| (title.chars().take(256).collect(), rect))
        })
        .take(MAX_FRAMES)
        .collect()
}

/// Read a node position when its layout has both coordinates.
fn position(node: XmlNode) -> Option<[f32; 2]> {
    let at = parse_floats(child_value(child_element(node, "GUILayout")?, "gpos")?);
    Some([*at.first()?, *at.get(1)?])
}

/// `pkg:///Graph/Path?dependency=123` as (graph path, package file)
fn reference(path: &str, dependencies: &HashMap<&str, String>) -> Option<(String, String)> {
    let path = path.strip_prefix("pkg:///")?;
    let (graph, query) = path.split_once('?').unwrap_or((path, ""));
    let uid = query.split('&').find_map(|pair| pair.strip_prefix("dependency="))?;
    let package = dependencies.get(uid)?.clone();
    let graph = graph.trim_matches('/');
    (!graph.is_empty()).then(|| (graph.chars().take(1024).collect(), package))
}

/// Translate a graph node into a library placement without cooking it.
fn graph_item(graph: XmlNode, node: XmlNode, dependencies: &HashMap<&str, String>) -> Option<LayoutItem> {
    let implementation = child_element(node, "compImplementation")?.children().find(|c| c.is_element())?;
    match implementation.tag_name().name() {
        "compFilter" => {
            let filter = child_value(implementation, "filter")?;
            if filter == "bitmap"
                && let Some(resource) = string_parameter(implementation, "bitmapresourcepath")
            {
                let resource = resource.strip_prefix("pkg:///").unwrap_or(resource);
                let resource = resource.split_once('?').map_or(resource, |(path, _)| path);
                return Some(LayoutItem::Bitmap { resource: resource.chars().take(1024).collect() });
            }

            Some(LayoutItem::Atomic { filter: filter.chars().take(128).collect(), node: FILTERS.iter().find(|f| f.sbs == filter).map(|f| f.def) })
        }
        "compInstance" => {
            let (graph, package) = reference(child_value(implementation, "path")?, dependencies)?;
            let outputs = child_element(implementation, "outputBridgings")
                .into_iter()
                .flat_map(|b| children_named(b, "outputBridging"))
                .filter_map(|b| child_value(b, "identifier"))
                .take(64)
                .map(str::to_string)
                .collect();
            Some(LayoutItem::Instance { package, graph, outputs })
        }
        "compInputBridge" => {
            let entry = child_value(implementation, "entry")?;
            let input = child_element(graph, "paraminputs")
                .into_iter()
                .flat_map(|inputs| children_named(inputs, "paraminput"))
                .find(|input| child_value(*input, "uid") == Some(entry));
            Some(LayoutItem::Node(match input.and_then(|input| child_value(input, "type")) {
                Some("1") | None => "input_color",
                Some("2") => "input_grayscale",
                Some(_) => "input_value",
            }))
        }
        "compOutputBridge" => Some(LayoutItem::Node("output")),
        _ => None,
    }
}

/// Translate a native operation or reusable function reference into a placement.
fn function_item(node: XmlNode, dependencies: &HashMap<&str, String>) -> Option<LayoutItem> {
    let operation = child_value(node, "function")?;
    if operation != "instance" {
        return Some(LayoutItem::Operation(operation.chars().take(128).collect()));
    }

    let path = child_element(node, "funcDatas")
        .into_iter()
        .flat_map(|datas| children_named(datas, "funcData"))
        .find(|data| child_value(*data, "name") == Some("instance"))
        .and_then(|data| child_element(data, "constantValue"))
        .and_then(|value| child_value(value, "constantValueString"))?;
    let (graph, package) = reference(path, dependencies)?;
    Some(LayoutItem::FunctionInstance { package, graph })
}

/// Read a named static string parameter from an atomic filter.
fn string_parameter<'a>(filter: XmlNode<'a, '_>, name: &str) -> Option<&'a str> {
    child_element(filter, "parameters")
        .into_iter()
        .flat_map(|parameters| children_named(parameters, "parameter"))
        .find(|parameter| child_value(*parameter, "name") == Some(name))
        .and_then(|parameter| child_element(parameter, "paramValue"))
        .and_then(|value| child_value(value, "constantValueString"))
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    const LAYOUT: &str = r#"<?xml version="1.0"?><package><dependencies>
        <dependency><filename v="?himself"/><uid v="1"/></dependency>
        <dependency><filename v="sbs://noise.sbs"/><uid v="2"/></dependency>
        <dependency><filename v="sbs://functions.sbs"/><uid v="3"/></dependency></dependencies>
        <content><graph><identifier v="Graph"/>
        <paraminputs><paraminput><identifier v="mask"/><uid v="40"/><type v="2"/></paraminput></paraminputs>
        <compNodes>
          <compNode><uid v="10"/><GUILayout><gpos v="20 90 0"/></GUILayout><compImplementation><compInstance><path v="pkg:///perlin_noise?dependency=2"/><outputBridgings><outputBridging><uid v="11"/><identifier v="output"/></outputBridging></outputBridgings></compInstance></compImplementation></compNode>
          <compNode><uid v="12"/><GUILayout><gpos v="20 40 0"/></GUILayout><compImplementation><compFilter><filter v="blur"/></compFilter></compImplementation></compNode>
          <compNode><uid v="13"/><GUILayout><gpos v="20 60 0"/></GUILayout><compImplementation><compInputBridge><entry v="40"/></compInputBridge></compImplementation></compNode>
          <compNode><uid v="14"/><GUILayout><gpos v="500 60 0"/></GUILayout><compImplementation><compFilter><filter v="bitmap"/><parameters><parameter><name v="bitmapresourcepath"/><paramValue><constantValueString v="pkg:///Resources/studio?dependency=1"/></paramValue></parameter></parameters></compFilter></compImplementation></compNode>
          <compNode><uid v="15"/><GUILayout><gpos v="900 900 0"/></GUILayout><compImplementation><compOutputBridge/></compImplementation></compNode>
          <compNode><uid v="16"/><GUILayout><gpos v="30 70 0"/></GUILayout><compImplementation><compFilter><filter v="blend"/><parameters><parameter><name v="opacitymult"/><paramValue><dynamicValue>
            <paramNodes>
              <paramNode><uid v="1"/><GUILayout><gpos v="0 10 0"/></GUILayout><function v="add"/></paramNode>
              <paramNode><uid v="2"/><GUILayout><gpos v="300 10 0"/></GUILayout><function v="instance"/><funcDatas><funcData><name v="instance"/><constantValue><constantValueString v="pkg:///Functions/Math/Pi?dependency=3"/></constantValue></funcData></funcDatas></paramNode>
            </paramNodes>
            <GUIObjects>
              <GUIObject><type v="COMMENT"/><GUILayout><gpos v="-10 0 0"/><size v="100 100"/></GUILayout><title v="Operator"/></GUIObject>
              <GUIObject><type v="COMMENT"/><GUILayout><gpos v="250 0 0"/><size v="100 100"/></GUILayout><title v="Constant"/></GUIObject>
            </GUIObjects></dynamicValue></paramValue></parameter></parameters></compFilter></compImplementation></compNode>
        </compNodes>
        <GUIObjects>
          <GUIObject><type v="COMMENT"/><GUILayout><gpos v="0 0 0"/><size v="160 200"/></GUILayout><title v="Texture Generators - Noises"/></GUIObject>
          <GUIObject><type v="COMMENT"/><GUILayout><gpos v="400 0 0"/><size v="160 200"/></GUILayout><title v="3D view - HDRI environments"/></GUIObject>
          <GUIObject><type v="COMMENT"/><GUILayout><gpos v="0 0 0"/><size v="10 10"/></GUILayout></GUIObject>
        </GUIObjects></graph></content></package>"#;

    /// Frames preserve spatial ordering and distinguish material and function entries.
    #[test]
    fn frames_list_what_sits_inside_them_top_to_bottom() {
        let frames = read_layout(LAYOUT.as_bytes(), "sbs://layout.sbs").unwrap();
        let graph: Vec<_> = frames.iter().filter(|frame| !frame.function).collect();
        assert_eq!(graph.len(), 2);
        assert_eq!(graph[0].title, "Texture Generators - Noises");
        assert_eq!(
            graph[0].items,
            vec![
                LayoutItem::Atomic { filter: "blur".into(), node: Some("blur") },
                LayoutItem::Node("input_grayscale"),
                LayoutItem::Atomic { filter: "blend".into(), node: Some("blend") },
                LayoutItem::Instance { package: "sbs://noise.sbs".into(), graph: "perlin_noise".into(), outputs: vec!["output".into()] },
            ]
        );
        assert_eq!(graph[1].items, vec![LayoutItem::Bitmap { resource: "Resources/studio".into() }]);

        let function: Vec<_> = frames.iter().filter(|frame| frame.function).collect();
        assert_eq!(function.len(), 2);
        assert_eq!(function[0].items, vec![LayoutItem::Operation("add".into())]);
        assert_eq!(function[1].items, vec![LayoutItem::FunctionInstance { package: "sbs://functions.sbs".into(), graph: "Functions/Math/Pi".into() }]);
    }

    /// Missing layouts and malformed input remain nonfatal.
    #[test]
    fn packages_without_frames_have_no_layout_and_bad_input_errors() {
        assert!(read_layout(b"<package><content><graph><compNodes/></graph></content></package>", "sbs://x.sbs").unwrap().is_empty());
        assert!(read_layout(b"<nope/>", "sbs://x.sbs").is_err());
        assert!(read_layout(b"\xff\xfe", "sbs://x.sbs").is_err());
        assert!(read_layout(b"<package><content><graph><compNodes><compNode><GUILayout><gpos v=\"nan\"/></GUILayout></compNode></compNodes><GUIObjects><GUIObject><type v=\"COMMENT\"/><GUILayout><gpos v=\"0 0\"/><size v=\"-5 1e40\"/></GUILayout><title v=\"X\"/></GUIObject></GUIObjects></graph></content></package>", "sbs://x.sbs").unwrap().is_empty());
    }

    /// Relative layout references resolve beside the layout, including parent directories.
    #[test]
    fn review_layout_resolves_relative_dependencies() {
        let layout = LAYOUT.replace("sbs://noise.sbs", "../noise.sbs").replace("sbs://functions.sbs", "functions.sbs");

        let frames = read_layout(layout.as_bytes(), "sbs://layouts/library.sbs").unwrap();

        assert!(frames.iter().flat_map(|frame| &frame.items).any(|item| matches!(item,
            LayoutItem::Instance { package, .. } if package == "sbs://noise.sbs")));
        assert!(frames.iter().flat_map(|frame| &frame.items).any(|item| matches!(item,
            LayoutItem::FunctionInstance { package, .. } if package == "sbs://layouts/functions.sbs")));
    }
}
