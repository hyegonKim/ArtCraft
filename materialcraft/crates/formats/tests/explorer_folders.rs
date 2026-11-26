use materialcraft_formats::sbs::{read_sbs, write_sbs};

#[test]
fn native_folder_descriptions_survive_xml_round_trip() {
    let xml = br#"<package><identifier v="Folders"/><content><group><identifier v="Tools"/><desc v="Masks &amp; shapes&#10;Notes"/><content/></group></content></package>"#;
    let imported = read_sbs(xml, "folders.sbs", &|_| None).unwrap();

    assert_eq!(imported.pkg.folder_descriptions["Tools"], "Masks & shapes\nNotes");

    let saved = write_sbs(&imported.pkg).unwrap();
    let document = roxmltree::Document::parse(std::str::from_utf8(&saved).unwrap()).unwrap();
    let group = document.descendants().find(|node| node.has_tag_name("group") && node.children().any(|child| child.attribute("v") == Some("Tools"))).unwrap();

    assert_eq!(group.children().find(|node| node.has_tag_name("desc")).unwrap().attribute("v"), Some("Masks & shapes\nNotes"));
    assert_eq!(read_sbs(&saved, "saved.sbs", &|_| None).unwrap().pkg.folder_descriptions, imported.pkg.folder_descriptions);
}

#[test]
fn empty_nested_folders_survive_sbs_round_trip() {
    let xml = br#"<package><identifier v="Folders"/><content><group><identifier v="Tools"/><content><group><identifier v="Empty"/><content/></group></content></group></content></package>"#;
    let imported = read_sbs(xml, "folders.sbs", &|_| None).unwrap();

    let saved = write_sbs(&imported.pkg).unwrap();
    let document = roxmltree::Document::parse(std::str::from_utf8(&saved).unwrap()).unwrap();
    let groups: Vec<_> =
        document.descendants().filter(|node| node.has_tag_name("group") && !node.children().any(|child| child.attribute("v") == Some("Resources"))).collect();

    assert_eq!(groups.len(), 2);
    assert!(groups[1].ancestors().any(|node| node == groups[0]));
    assert!(imported.pkg.graphs.is_empty());
    assert!(imported.warnings.is_empty());
}

#[test]
fn nested_resource_paths_are_preserved_without_duplicate_groups() {
    use materialcraft_core::{Graph, Package, Resource, ResourceBytes, Value};
    let mut package = Package::new("Resources");
    let mut resource = Resource::new("Mask", "mask.png", ResourceBytes::new(vec![1, 2, 3]));
    resource.folder = "Tools/Images".into();
    package.resources.push(resource);
    package.folders.push("Tools/Images".into());
    let mut graph = Graph::new("Surface");
    graph.folder = "Tools".into();
    let bitmap = graph.add_node("bitmap", [0.0; 2]);
    graph.node_mut(bitmap).unwrap().params.insert("resource".into(), Value::Text("Mask".into()));
    package.graphs.push(graph);

    let bytes = write_sbs(&package).unwrap();
    let xml = std::str::from_utf8(&bytes).unwrap();

    assert!(xml.contains("pkg:///Tools/Images/Mask?dependency="));

    let read = read_sbs(&bytes, "resources.sbs", &|_| None).unwrap();

    assert_eq!(read.pkg.resources[0].folder, "Tools/Images");
    assert_eq!(read.pkg.graphs[0].nodes[0].params["resource"], Value::Text("Mask".into()));

    let bytes = write_sbs(&read.pkg).unwrap();
    let document = roxmltree::Document::parse(std::str::from_utf8(&bytes).unwrap()).unwrap();

    assert_eq!(document.descendants().filter(|node| node.has_tag_name("group")).count(), 3);
}

#[test]
fn empty_packages_round_trip_without_invented_graphs() {
    let package = materialcraft_core::Package::new("Empty");
    let read = read_sbs(&write_sbs(&package).unwrap(), "empty.sbs", &|_| None).unwrap();
    assert!(read.pkg.graphs.is_empty());
    assert!(read.warnings.is_empty());
}

#[test]
#[ignore = "local interoperability inputs; set MC_SBS_PACKAGES to a package directory"]
fn supplied_packages_keep_native_folder_hierarchies() {
    let mut pending = vec![std::path::PathBuf::from(std::env::var_os("MC_SBS_PACKAGES").unwrap())];
    let mut checked = 0;
    let mut descriptions_checked = 0;
    while let Some(path) = pending.pop() {
        if path.is_dir() {
            pending.extend(std::fs::read_dir(path).unwrap().map(|entry| entry.unwrap().path()));
            continue;
        }

        if path.extension().is_none_or(|extension| extension != "sbs") {
            continue;
        }

        let bytes = std::fs::read(&path).unwrap();
        let document = roxmltree::Document::parse(std::str::from_utf8(&bytes).unwrap().trim_start_matches('\u{feff}')).unwrap();
        let is_folder = |node: &roxmltree::Node<'_, '_>| node.has_tag_name("group") && node.children().any(|child| child.has_tag_name("content"));
        let mut paths: std::collections::BTreeSet<_> = document
            .descendants()
            .filter(is_folder)
            .map(|node| {
                let mut parts: Vec<_> = node
                    .ancestors()
                    .filter(is_folder)
                    .map(|node| node.children().find(|node| node.has_tag_name("identifier")).unwrap().attribute("v").unwrap())
                    .collect();
                parts.reverse();
                parts.join("/")
            })
            .collect();
        paths.insert("Resources".to_string());
        let descriptions: std::collections::BTreeMap<_, _> = document
            .descendants()
            .filter(is_folder)
            .filter_map(|node| {
                let description = node.children().find(|child| child.has_tag_name("desc"))?.attribute("v").filter(|description| !description.is_empty())?;
                let mut parts: Vec<_> = node
                    .ancestors()
                    .filter(is_folder)
                    .map(|node| node.children().find(|child| child.has_tag_name("identifier")).unwrap().attribute("v").unwrap())
                    .collect();
                parts.reverse();
                Some((parts.join("/"), description.to_string()))
            })
            .collect();
        let imported = read_sbs(&bytes, &path.to_string_lossy(), &|_| None).unwrap();

        assert_eq!(imported.pkg.folder_paths(), paths, "{}: import", path.display());
        assert_eq!(imported.pkg.folder_descriptions, descriptions, "{}: imported folder descriptions", path.display());

        let saved = write_sbs(&imported.pkg).unwrap();
        let reopened = read_sbs(&saved, "roundtrip.sbs", &|_| None).unwrap();

        assert_eq!(reopened.pkg.folder_paths(), paths, "{}: export", path.display());
        assert_eq!(reopened.pkg.folder_descriptions, descriptions, "{}: exported folder descriptions", path.display());

        descriptions_checked += descriptions.len();
        checked += 1;
    }

    assert!(checked > 0);

    println!("Verified native folder hierarchies and {descriptions_checked} descriptions in {checked} supplied SBS packages");
}
