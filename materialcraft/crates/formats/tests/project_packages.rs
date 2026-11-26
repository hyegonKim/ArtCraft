use materialcraft_core::{Graph, GraphInstance, Package, Resource, ResourceBytes, Value};
use materialcraft_formats::{
    Depth, encode,
    project::{self, ProjectFiles},
};
use std::io::{Cursor, Write};

const SVG: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" width="2" height="2"><rect width="2" height="2" fill="red"/></svg>"#;

fn package() -> Package {
    let mut package = Package::new("Surface");
    let pixels = materialcraft_image::Image::filled(2, 2, materialcraft_image::Kind::Color, [0.2, 0.4, 0.8, 1.0]);
    let png = encode(&pixels, "png", Depth::U8, true).unwrap();
    package.resources.push(Resource::new("Texture", "texture.png", ResourceBytes::new(png)));
    package.resources.push(Resource::new("Shape", "shape.svg", ResourceBytes::new(SVG.to_vec())));
    let mut graph = Graph::new("Material");
    for (def, name) in [("bitmap", "Texture"), ("svg", "Shape")] {
        let id = graph.add_node(def, [0.0; 2]);
        graph.node_mut(id).unwrap().params.insert("resource".into(), Value::Text(name.into()));
    }
    package.graphs.push(graph);
    package
}

fn zip(entries: &[(&str, &[u8])]) -> Vec<u8> {
    let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
    for (name, bytes) in entries {
        writer.start_file(*name, zip::write::SimpleFileOptions::default()).unwrap();
        writer.write_all(bytes).unwrap();
    }
    writer.finish().unwrap().into_inner()
}

#[test]
fn archive_is_only_native_xml_and_original_resources() {
    let original = package();
    let files = ProjectFiles::build(&original, "surface.mcraft", true).unwrap();

    assert_eq!(files.entry, "package.sbs");
    assert_eq!(ProjectFiles::build(&original, "renamed.mcraft", true).unwrap().entry, "package.sbs");
    assert_eq!(files.files.len(), 3);

    let xml = std::str::from_utf8(files.xml().unwrap()).unwrap();

    assert!(xml.starts_with("<?xml"));
    assert!(!xml.contains("binembedded"));
    assert_eq!(xml.matches("<externalcopy>").count(), 2);

    for resource in &original.resources {
        assert!(files.files.iter().any(|(name, bytes)| name.starts_with("resources/") && *bytes == resource.bytes));
    }
    let bytes = files.zip().unwrap();
    let archive = zip::ZipArchive::new(Cursor::new(&bytes)).unwrap();

    assert_eq!(archive.len(), 4);
    assert!(archive.file_names().all(|name| name == "package.sbs" || name.starts_with("resources/")));

    let read = project::read(&bytes, "moved.mcraft").unwrap();

    assert!(read.warnings.is_empty(), "{:?}", read.warnings);

    for resource in &original.resources {
        assert_eq!(read.pkg.resource(&resource.name).unwrap().bytes, resource.bytes);
    }
    let second = ProjectFiles::build(&read.pkg, "surface.mcraft", true).unwrap();

    assert_eq!(files.files.keys().collect::<Vec<_>>(), second.files.keys().collect::<Vec<_>>(), "resource names stay stable across saves");
    assert_eq!(original.resources[0].source, "");
}

#[test]
fn case_variant_resource_names_round_trip_in_both_project_layouts() {
    let mut original = Package::new("Case variants");
    for (name, bytes) in
        [("Mask", SVG.to_vec()), ("mask", SVG.to_vec()), ("MASK", String::from_utf8(SVG.to_vec()).unwrap().replace("red", "blue").into_bytes())]
    {
        original.resources.push(Resource::new(name, "shape.SVG", ResourceBytes::new(bytes)));
    }

    for archive in [false, true] {
        let files = ProjectFiles::build(&original, "case-variants.sbs", archive).unwrap();
        let reopened = project::read(&files.zip().unwrap(), "case-variants.mcraft").unwrap();

        assert!(reopened.warnings.is_empty(), "{:?}", reopened.warnings);
        assert_eq!(reopened.pkg.resources.len(), original.resources.len());

        for resource in &original.resources {
            assert_eq!(reopened.pkg.resource(&resource.name).unwrap().bytes, resource.bytes);
        }

        let saved_again = ProjectFiles::build(&reopened.pkg, "case-variants.sbs", archive).unwrap();

        assert_eq!(files.files.keys().collect::<Vec<_>>(), saved_again.files.keys().collect::<Vec<_>>(), "resource paths must stay stable across saves");
    }
}

#[test]
fn rejects_unsafe_ambiguous_and_truncated_archives() {
    let xml = b"<package><content/></package>";
    for entries in [
        vec![("../escape", &b"x"[..]), ("p.sbs", &xml[..])],
        vec![("/absolute", &b"x"[..]), ("p.sbs", &xml[..])],
        vec![("resources\\bad", &b"x"[..]), ("p.sbs", &xml[..])],
        vec![("p.sbs", &xml[..]), ("P.SBS", &xml[..])],
        vec![("p.sbs", &xml[..]), ("q.sbs", &xml[..])],
        vec![("nested/p.sbs", &xml[..])],
    ] {
        assert!(project::read(&zip(&entries), "bad.mcraft").is_err());
    }
    let bytes = zip(&[("p.sbs", xml)]);
    for length in [0, 1, 4, bytes.len() / 2, bytes.len() - 1] {
        assert!(project::read(&bytes[..length], "bad.mcraft").is_err());
    }
}

#[test]
fn missing_resources_cannot_be_silently_saved_away() {
    let mut package = package();
    package.resources[0].bytes = ResourceBytes::default();
    package.resources[0].source = "resources/missing.png".into();
    for archive in [false, true] {
        assert!(ProjectFiles::build(&package, "missing.sbs", archive).is_err());
    }
}

#[test]
fn rejects_symlinks() {
    let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
    writer.add_symlink("resources/link", "/outside", zip::write::SimpleFileOptions::default()).unwrap();

    assert!(project::read(&writer.finish().unwrap().into_inner(), "bad.mcraft").is_err());
}

#[test]
fn resource_counts_do_not_truncate_saved_packages() {
    let mut original = Package::new("Resources");
    let bytes = ResourceBytes::new(SVG.to_vec());
    for index in 0..4097 {
        original.resources.push(Resource::new(format!("Shape_{index}"), "shape.svg", bytes.clone()));
    }

    for archive in [false, true] {
        let files = ProjectFiles::build(&original, "resources.sbs", archive).unwrap();
        let read = project::read(&files.zip().unwrap(), "resources.mcraft").unwrap();

        assert!(read.warnings.is_empty(), "{:?}", read.warnings);
        assert_eq!(read.pkg.resources.len(), original.resources.len());
        assert_eq!(read.pkg.resource("Shape_4096").unwrap().bytes, bytes);
    }
}

#[test]
fn archives_preserve_all_linked_packages() {
    let mut original = Package::new("Dependencies");
    let mut graph = Graph::new("Material");
    let mut source = Package::new("Source");
    source.graphs.push(Graph::new("Material"));
    let source = std::sync::Arc::new(source);
    for index in 0..65 {
        let path = format!("/source/{index}.sbs");
        let node = graph.add_node("graph_instance", [0.0; 2]);
        graph.node_mut(node).unwrap().instance = Some(GraphInstance {
            label: String::new(),
            package: path.clone(),
            graph: "Material".into(),
            inputs: Vec::new(),
            outputs: Vec::new(),
            defaults: Default::default(),
        });
        original.dependencies.insert(path, source.clone());
    }
    original.graphs.push(graph);

    let files = ProjectFiles::build(&original, "dependencies.mcraft", true).unwrap();
    let read = project::read(&files.zip().unwrap(), "dependencies.mcraft").unwrap();

    assert!(read.warnings.is_empty(), "{:?}", read.warnings);
    assert_eq!(read.pkg.dependencies.len(), original.dependencies.len() + 1);

    for node in &read.pkg.graphs[0].nodes {
        let instance = node.instance.as_ref().unwrap();
        assert!(read.pkg.dependencies[&instance.package].graph_by_name(&instance.graph).is_some());
    }
}

#[test]
fn archives_bundle_linked_graphs_and_can_be_converted_back_to_sbs() {
    let source = package();
    let mut package = Package::new("Consumer");
    let mut graph = Graph::new("Graph");
    let id = graph.add_node("graph_instance", [0.0; 2]);
    graph.node_mut(id).unwrap().instance = Some(GraphInstance {
        label: String::new(),
        package: "/source/material.sbs".into(),
        graph: "Material".into(),
        inputs: Vec::new(),
        outputs: Vec::new(),
        defaults: Default::default(),
    });
    package.graphs.push(graph);
    package.dependencies.insert("/source/material.sbs".into(), std::sync::Arc::new(source));
    let files = ProjectFiles::build(&package, "consumer.mcraft", true).unwrap();
    let read = project::read(&files.zip().unwrap(), "consumer.mcraft").unwrap();

    assert!(read.warnings.is_empty(), "{:?}", read.warnings);
    let reference = &read.pkg.graphs[0].nodes[0].instance.as_ref().unwrap().package;
    assert!(reference.starts_with("mcraft://"));
    let dependency = read.pkg.dependencies.get(reference).unwrap();
    assert_eq!(dependency.resources[1].bytes.as_slice(), SVG);

    let native = ProjectFiles::build(&read.pkg, "consumer.sbs", false).unwrap();
    let xml = std::str::from_utf8(native.xml().unwrap()).unwrap();

    assert!(xml.contains("resources/packages/"));
    assert!(!xml.contains("mcraft://"));
    assert!(!xml.contains("/source/material.sbs"));
    assert!(native.files.keys().any(|name| name.ends_with(".svg")));
}
