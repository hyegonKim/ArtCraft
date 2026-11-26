use materialcraft_core::{FunctionGraph, Graph, Package, Value};
use materialcraft_formats::sbs::{read_sbs, write_sbs};

#[test]
fn conversion_preserves_function_bodies_and_replaces_presentation_metadata() {
    let mut package = Package::new("Functions");
    let mut graph = Graph::new("AddOne");
    graph.folder = "Functions/Math".into();
    graph.attributes.description = "Source description".into();
    graph.attributes.author = "Source author".into();
    graph.attributes.author_url = "https://example.org/source".into();
    graph.attributes.user_data = "Source notes".into();
    graph.attributes.category = "Function".into();
    graph.attributes.tags = "Maths".into();
    graph.function_type = 256;
    let mut body = FunctionGraph::default();
    let left = body.add_constant(Value::Float(1.0));
    let right = body.add_constant(Value::Float(2.0));
    body.output = Some(body.add("add", &[left, right], None));
    graph.function = Some(body);
    package.graphs.push(graph);
    let bytes = write_sbs(&package).unwrap();
    let imported = read_sbs(&bytes, "source.sbs", &|_| None).unwrap().pkg;

    let converted = super::convert::package(&bytes, "source.sbs").unwrap();
    let exported = write_sbs(&converted).unwrap();
    let reopened = read_sbs(&exported, "converted.sbs", &|_| None).unwrap().pkg;

    assert_eq!(converted.graphs[0].sbs_path(), "Functions/Math/AddOne");
    assert_ne!(converted.graphs[0].attributes.description, imported.graphs[0].attributes.description);
    assert!(converted.graphs[0].attributes.author.is_empty());
    assert!(converted.graphs[0].attributes.author_url.is_empty());
    assert!(converted.graphs[0].attributes.user_data.is_empty());
    assert_eq!(converted.graphs[0].attributes.category, "Function");
    assert_eq!(converted.graphs[0].attributes.tags, "Maths");
    assert_eq!(reopened.graphs[0].attributes.category, "Function");
    assert_eq!(reopened.graphs[0].attributes.tags, "Maths");
    assert_eq!(converted.graphs[0].function, reopened.graphs[0].function);
    let mut restored = converted.graphs[0].function.clone().unwrap();
    let original = imported.graphs[0].function.as_ref().unwrap();
    for (node, source) in restored.nodes.iter_mut().zip(&original.nodes) {
        assert_ne!(node.pos, source.pos);
        node.pos = source.pos;
    }
    assert_eq!(&restored, original);
    assert_eq!(exported, write_sbs(&super::convert::package(&bytes, "source.sbs").unwrap()).unwrap());
}

#[test]
fn layout_separates_nodes_and_rejects_invalid_connections_without_partial_edits() {
    let mut function = FunctionGraph::default();
    let constant = function.add_constant(Value::Float(1.0));
    function.output = Some(function.add("add", &[constant, constant], None));
    let disconnected = function.add_constant(Value::Float(2.0));
    function.nodes.reverse();

    super::layout::reposition(&mut function).unwrap();

    let source = function.node(constant).unwrap();
    let result = function.node(function.output.unwrap()).unwrap();
    assert!(result.pos[0] > source.pos[0]);
    assert_ne!(source.pos, function.node(disconnected).unwrap().pos);
    function.nodes[0].inputs = vec![function.output];
    function.nodes[1].inputs = vec![Some(disconnected)];
    let cyclic = function.clone();
    assert!(super::layout::reposition(&mut function).unwrap_err().contains("cycle"));
    assert_eq!(function, cyclic);
    function.nodes[0].inputs = vec![Some(9999)];
    assert!(super::layout::reposition(&mut function).unwrap_err().contains("missing node"));
}

#[test]
fn conversion_preserves_empty_classification_attributes() {
    let mut package = Package::new("Functions");
    let mut graph = Graph::new("EmptyMetadata");
    graph.folder = "Functions/Math".into();
    let mut function = FunctionGraph::default();
    function.output = Some(function.add_constant(Value::Float(1.0)));
    graph.function = Some(function);
    graph.function_type = 256;
    package.graphs.push(graph);
    let bytes = write_sbs(&package).unwrap();

    let converted = super::convert::package(&bytes, "source.sbs").unwrap();

    assert!(converted.graphs[0].attributes.category.is_empty());
    assert!(converted.graphs[0].attributes.tags.is_empty());
}
