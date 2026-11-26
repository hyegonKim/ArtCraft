use materialcraft_formats::sbs::{read_sbs, write_sbs};

const LIBRARY: &str = r#"<package><dependencies><dependency><uid v="9"/><filename v="sbs://functions.sbs"/></dependency></dependencies><content>
<group><identifier v="Functions"/><content><group><identifier v="Math"/><content>
<function><identifier v="Pi"/><type v="256"/><paramValue><dynamicValue><rootnode v="1"/><paramNodes>
<paramNode><uid v="1"/><function v="const_float1"/><type v="256"/><funcDatas><funcData><name v="const_float1"/><constantValueFloat1 v="3.1415927"/></funcData></funcDatas></paramNode>
</paramNodes></dynamicValue></paramValue></function>
</content></group></content></group>
<function><identifier v="Caller"/><type v="256"/><paramValue><dynamicValue><rootnode v="2"/><paramNodes>
<paramNode><uid v="2"/><function v="instance"/><type v="256"/><funcDatas><funcData><name v="instance"/><constantValueString v="pkg:///Functions/Math/Pi?dependency=9"/></funcData></funcDatas></paramNode>
</paramNodes></dynamicValue></paramValue></function></content></package>"#;

#[test]
fn unresolved_polymorphic_output_preserves_the_declared_function_type() {
    let xml = LIBRARY.replace("<function v=\"instance\"/><type v=\"256\"/>", "<function v=\"instance\"/><type v=\"118772\"/>");
    let source = read_sbs(xml.as_bytes(), "source.sbs", &|_| None).unwrap().pkg;

    let saved = read_sbs(&write_sbs(&source).unwrap(), "saved.sbs", &|_| None).unwrap().pkg;

    assert_eq!(saved.graph_by_name("Caller").unwrap().1.function_type, 256);
    assert_eq!(saved.graph_by_name("Caller").unwrap().1.function.as_ref().unwrap().nodes[0].type_code, 118772);
}

#[test]
fn nested_function_paths_and_library_aliases_survive_save() {
    let read = read_sbs(LIBRARY.as_bytes(), "test.sbs", &|_| None).unwrap();
    assert!(read.warnings.is_empty(), "{:?}", read.warnings);
    let (_, pi) = read.pkg.graph_by_name("Functions/Math/Pi").expect("package path must remain addressable");
    assert_eq!(pi.name, "Pi");
    let call = read.pkg.graph_by_name("Caller").unwrap().1.function.as_ref().unwrap();
    assert_eq!(call.nodes[0].instance.as_ref().unwrap().package, "sbs://functions.sbs");
    let bytes = write_sbs(&read.pkg).unwrap();
    let saved = read_sbs(&bytes, "saved.sbs", &|_| None).unwrap();
    assert!(saved.warnings.is_empty(), "{:?}", saved.warnings);
    assert!(saved.pkg.graph_by_name("Functions/Math/Pi").is_some());
    assert!(String::from_utf8(bytes).unwrap().contains(&format!("<updaterVersion v=\"{}\"", env!("CARGO_PKG_VERSION"))));
}

#[test]
fn unknown_operations_keep_their_data_and_output() {
    let xml = LIBRARY.replace("const_float1", "future_operation");
    let read = read_sbs(xml.as_bytes(), "missing.sbs", &|_| None).unwrap();
    let graph = read.pkg.graph_by_name("Functions/Math/Pi").unwrap().1.function.as_ref().unwrap();
    assert_eq!(graph.nodes.len(), 1);
    assert_eq!(graph.output, Some(1));
    assert!(materialcraft_nodes::function::compile(graph, 0).err().unwrap().contains("Missing"));
    let bytes = write_sbs(&read.pkg).unwrap();
    let saved = read_sbs(&bytes, "saved.sbs", &|_| None).unwrap();
    let graph = saved.pkg.graph_by_name("Functions/Math/Pi").unwrap().1.function.as_ref().unwrap();
    assert_eq!(graph.nodes[0].op, "future_operation");
    assert!(graph.nodes[0].missing_data.contains("3.1415927"));
    assert!(graph.output.is_some());
}

#[test]
fn preserved_function_data_cannot_inject_xml_outside_its_element() {
    let read = read_sbs(LIBRARY.replace("const_float1", "future_operation").as_bytes(), "missing.sbs", &|_| None).unwrap();
    let mut package = read.pkg;
    let index = package.graph_by_name("Functions/Math/Pi").unwrap().0;
    let node = &mut package.graphs[index].function.as_mut().unwrap().nodes[0];
    node.missing_data = "<?xml version=\"1.0\"?><funcDatas/>".into();

    let bytes = write_sbs(&package).unwrap();
    assert!(read_sbs(&bytes, "saved.sbs", &|_| None).is_ok());
    package.graphs[index].function.as_mut().unwrap().nodes[0].missing_data = "<content/>".into();
    assert!(write_sbs(&package).is_err());
}

#[test]
fn absent_dependency_records_do_not_erase_function_nodes() {
    let xml = LIBRARY.replace("dependency=9", "dependency=404");
    let read = read_sbs(xml.as_bytes(), "missing.sbs", &|_| None).unwrap();

    let function = read.pkg.graph_by_name("Caller").unwrap().1.function.as_ref().unwrap();
    assert_eq!(function.nodes.len(), 1);
    assert_eq!(function.output, Some(2));
    assert!(function.nodes[0].instance.is_some());
}

#[test]
fn missing_resource_links_survive_save_and_can_be_repaired() {
    let xml = r#"<package><content><resource><identifier v="texture"/><type v="bitmap"/><format v="png"/><source><externalcopy><filename v="sbs://missing.png"/></externalcopy></source></resource></content></package>"#;
    let read = read_sbs(xml.as_bytes(), "missing.sbs", &|_| None).unwrap();

    let bytes = write_sbs(&read.pkg).unwrap();
    let repaired = read_sbs(&bytes, "saved.sbs", &|path| (path == "sbs://missing.png").then(|| vec![1, 2, 3])).unwrap();

    assert_eq!(repaired.pkg.resources.len(), 1);
    assert_eq!(repaired.pkg.resources[0].bytes.as_slice(), &[1, 2, 3]);
}

#[test]
#[ignore = "local interoperability input; set MC_FUNCTIONS_SBS to an external package"]
fn supplied_function_package_round_trip() {
    let path = std::env::var("MC_FUNCTIONS_SBS").unwrap();
    let bytes = std::fs::read(&path).unwrap();
    let read = read_sbs(&bytes, &path, &|_| None).unwrap();
    assert!(read.warnings.is_empty(), "{:?}", read.warnings);
    assert!(read.pkg.graphs.iter().all(|graph| graph.function.as_ref().is_some_and(|function| !function.nodes.is_empty())));
    let saved = read_sbs(&write_sbs(&read.pkg).unwrap(), "saved.sbs", &|_| None).unwrap();
    let bundled = read_sbs(include_bytes!("../../../assets/sbs/functions.sbs"), "functions.sbs", &|_| None).unwrap();
    assert!(saved.warnings.is_empty(), "{:?}", saved.warnings);
    assert!(bundled.warnings.is_empty(), "{:?}", bundled.warnings);
    assert_eq!(read.pkg.graphs.len(), 171);
    assert_eq!(read.pkg.graphs.len(), bundled.pkg.graphs.len());
    assert_eq!(read.pkg.folder_paths(), bundled.pkg.folder_paths());
    for graph in &read.pkg.graphs {
        let other = saved.pkg.graph_by_name(&graph.sbs_path()).unwrap().1;
        let replacement = bundled.pkg.graph_by_name(&graph.sbs_path()).unwrap().1;
        assert_eq!(graph.function_type, replacement.function_type, "{}", graph.sbs_path());
        assert_eq!(graph.function_type, other.function_type, "{}", graph.sbs_path());
        let mut expected_inputs = graph.exposed.clone();
        for input in &mut expected_inputs {
            input.description.clear();
        }
        assert_eq!(expected_inputs, replacement.exposed, "{}", graph.sbs_path());
        assert_ne!(graph.attributes.description, replacement.attributes.description, "{}", graph.sbs_path());
        assert!(replacement.attributes.author.is_empty());
        assert_eq!(replacement.attributes.category, graph.attributes.category);
        assert_eq!(replacement.attributes.tags, graph.attributes.tags);
        assert!(replacement.frames.is_empty() && replacement.comments.is_empty() && replacement.pins.is_empty());
        assert_same_body(graph, replacement);
        assert_eq!(graph.name, other.name);
        assert_eq!(graph.exposed, other.exposed);
        assert_eq!(graph.function.as_ref().unwrap().nodes.len(), other.function.as_ref().unwrap().nodes.len());
        let before = graph.function.as_ref().unwrap();
        let after = other.function.as_ref().unwrap();
        let ids: std::collections::BTreeMap<_, _> = before.nodes.iter().zip(&after.nodes).map(|(left, right)| (left.id, right.id)).collect();
        assert_eq!(before.output.map(|id| ids[&id]), after.output);
        for (left, right) in before.nodes.iter().zip(&after.nodes) {
            assert_eq!(left.op, right.op, "{}", graph.sbs_path());
            assert_eq!(left.value, right.value, "{}", graph.sbs_path());
            assert_eq!(left.type_code, right.type_code);
            assert_eq!(left.pos, right.pos);
            assert_eq!(left.instance, right.instance);
            assert_eq!(left.inputs.iter().map(|id| id.map(|id| ids[&id])).collect::<Vec<_>>(), right.inputs);
        }
    }
    eprintln!("Round-tripped and compared all {} converted function bodies against the supplied package", read.pkg.graphs.len());
}

fn assert_same_body(source: &materialcraft_core::Graph, converted: &materialcraft_core::Graph) {
    let before = source.function.as_ref().unwrap();
    let after = converted.function.as_ref().unwrap();
    assert_eq!(before.nodes.len(), after.nodes.len(), "{}", source.sbs_path());
    let identifiers: std::collections::BTreeMap<_, _> = before.nodes.iter().zip(&after.nodes).map(|(left, right)| (left.id, right.id)).collect();

    assert_eq!(before.output.map(|identifier| identifiers[&identifier]), after.output);
    for (left, right) in before.nodes.iter().zip(&after.nodes) {
        assert_eq!(left.op, right.op, "{}", source.sbs_path());
        assert_eq!(left.value, right.value, "{}", source.sbs_path());
        assert_eq!(left.type_code, right.type_code);
        assert_eq!(left.instance, right.instance);
        assert_eq!(left.inputs.iter().map(|identifier| identifier.map(|identifier| identifiers[&identifier])).collect::<Vec<_>>(), right.inputs);
        assert_ne!(left.pos, right.pos, "layout was not replaced in {}", source.sbs_path());
    }
}

#[test]
fn bundled_library_has_complete_bodies_and_current_export_metadata() {
    let bytes = include_bytes!("../../../assets/sbs/functions.sbs");
    let read = read_sbs(bytes, "functions.sbs", &|_| None).unwrap();
    let xml = roxmltree::Document::parse(std::str::from_utf8(bytes).unwrap()).unwrap();
    let version = xml.descendants().find(|element| element.has_tag_name("updaterVersion")).unwrap().attribute("v").unwrap();

    assert_eq!(version, env!("CARGO_PKG_VERSION"));
    assert!(read.warnings.is_empty(), "{:?}", read.warnings);
    assert_eq!(read.pkg.graphs.len(), 171);
    assert_eq!(read.pkg.graphs.iter().flat_map(|graph| graph.functions()).map(|function| function.nodes.len()).sum::<usize>(), 2334);
    for graph in &read.pkg.graphs {
        let function = graph.function.as_ref().unwrap();
        assert!(function.output.is_some(), "{}", graph.sbs_path());
        assert!(function.nodes.iter().all(|node| node.missing.is_none()), "{}", graph.sbs_path());
        assert!(!graph.attributes.description.is_empty());
        assert!(graph.attributes.author.is_empty());
        assert!(graph.frames.is_empty() && graph.comments.is_empty() && graph.pins.is_empty());
    }
    let pi = &read.pkg.graph_by_name("Functions/Math/Pi").unwrap().1.attributes;
    assert!(pi.category.is_empty());
    assert_eq!(pi.tags, "Constant");
    let saved = write_sbs(&read.pkg).unwrap();
    let reopened = read_sbs(&saved, "functions.sbs", &|_| None).unwrap();
    for graph in &read.pkg.graphs {
        let reloaded = reopened.pkg.graph_by_name(&graph.sbs_path()).unwrap().1;
        assert_eq!(reloaded.attributes.category, graph.attributes.category);
        assert_eq!(reloaded.attributes.tags, graph.attributes.tags);
    }
    assert!(!xml.descendants().any(|element| element.has_tag_name("icon")));
}

#[test]
fn library_aliases_reject_paths_outside_the_asset_root() {
    use materialcraft_formats::sbs::library::relative_path;

    assert_eq!(relative_path("sbs://folder/file.sbs").unwrap(), "folder/file.sbs");
    for invalid in ["sbs://../private", "sbs:///etc/passwd", "sbs://folder/../../private", "sbs://%2e%2e/private", "sbs://C:/private", "sbs://folder\\private"]
    {
        assert!(relative_path(invalid).is_err(), "{invalid}");
    }
}

#[test]
fn library_relative_resources_stay_with_the_owning_package() {
    use materialcraft_formats::sbs::library::resolve;

    assert_eq!(resolve("sbs://functions.sbs", "resources/tile.png").unwrap(), "sbs://resources/tile.png");
    assert_eq!(resolve("sbs://nested/functions.sbs", "resources/tile.png").unwrap(), "sbs://nested/resources/tile.png");
    assert_eq!(resolve("sbs://nested/functions.sbs", "../resources/tile.png").unwrap(), "sbs://resources/tile.png");
    assert_eq!(resolve("sbs://functions.sbs", ".\\resources\\tile.png").unwrap(), "sbs://resources/tile.png");
    assert_eq!(resolve("sbs://nested/functions.sbs", "sbs://resources/tile.png").unwrap(), "sbs://resources/tile.png");
    for invalid in ["../private", "../../private", "/etc/passwd", "C:\\private", "https://example.com/image.png", "%2e%2e/private", ""] {
        assert!(resolve("sbs://functions.sbs", invalid).is_err(), "{invalid}");
    }
}

#[test]
fn duplicate_leaf_names_require_their_package_folder() {
    let mut package = materialcraft_core::Package::new("Folders");
    for folder in ["A", "B"] {
        let mut graph = materialcraft_core::Graph::new("Same");
        graph.folder = folder.into();
        graph.function = Some(materialcraft_core::FunctionGraph::constant(materialcraft_core::Value::Float(1.0)));
        package.graphs.push(graph);
    }

    let read = read_sbs(&write_sbs(&package).unwrap(), "folders.sbs", &|_| None).unwrap();

    assert!(read.pkg.graph_by_name("Same").is_none());
    assert_eq!(read.pkg.graph_by_name("A/Same").unwrap().1.folder, "A");
    assert_eq!(read.pkg.graph_by_name("B/Same").unwrap().1.folder, "B");
}
