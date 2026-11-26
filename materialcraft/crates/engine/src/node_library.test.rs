use super::*;
use materialcraft_core::Graph;

#[path = "../tests/support/node_library.rs"]
mod fixture;

/// Category aliases and frame kinds resolve only to recognized categories.
#[test]
fn frame_titles_map_to_library_categories() {
    assert_eq!(frame_category("Texture Generators - Noises", false, false).as_deref(), Some("Texture generators/Noises"));
    assert_eq!(frame_category("Spline & Path tools - Path tools", false, false).as_deref(), Some("Spline & Path tools/Path tools"));
    assert_eq!(frame_category("Material Filters - 1 click", false, false).as_deref(), Some("Material filters/1-click"));
    assert_eq!(frame_category("Mesh-based generators - Weathering", false, false).as_deref(), Some("Mesh-based generators/Weathering"));
    assert_eq!(frame_category("PBR Materials", false, false).as_deref(), Some("PBR materials"));
    assert_eq!(frame_category("Atomic Nodes", false, false).as_deref(), Some("Atomic nodes"));

    // The category MaterialCraft renamed still reads from older layouts
    assert_eq!(frame_category("SDF functions - Primitives", false, false).as_deref(), Some("MCF functions/Primitives"));
    assert_eq!(frame_category("Functions Inside", false, false), None);
    assert_eq!(frame_category("Filters - Nowhere", false, false), None);
    assert_eq!(frame_category("Comparison", true, true).as_deref(), Some("Function nodes/Comparison"));
    assert_eq!(frame_category("Comparison", true, false).as_deref(), Some("Functions/Comparison"));
    assert_eq!(frame_category("Unknown", true, true), None);
}

/// Identifier labels preserve recognized acronyms and omit empty words.
#[test]
fn identifiers_read_as_labels() {
    assert_eq!(humanize("3D_perlin_noise"), "3D Perlin Noise");
    assert_eq!(humanize("bnw_spots_1"), "BnW Spots 1");
    assert_eq!(humanize("rgb-a_merge"), "RGB-A Merge");
    assert_eq!(humanize("Functions/Math/acescg_to_srgb"), "ACEScg To sRGB");
    assert_eq!(humanize("__"), "");
}

/// Function categories use exact path segments and known name overrides.
#[test]
fn functions_sort_into_sub_categories_by_folder_and_name() {
    assert_eq!(function_category("Functions/Math/Pi"), "Functions/Constant");
    assert_eq!(function_category("Functions/Math/Equality_Float2"), "Functions/Comparison");
    assert_eq!(function_category("Functions/Math/Parity/EvenCount"), "Functions/Parity");
    assert_eq!(function_category("Functions/Math/Trigo/acos"), "Functions/Maths");
    assert_eq!(function_category("Functions/Math/Trigo/deg_to_rad"), "Functions/Conversion");
    assert_eq!(function_category("Functions/easings/sine/ease_in_sine"), "Functions/Easings");
    assert_eq!(function_category("Functions/switch/Float2/switch_float2_4_inputs"), "Functions/Various");
    assert_eq!(function_category("Functions/Tonemappers/tonemap_ACES"), "Functions/Color");
    assert_eq!(function_category("Functions/Mathematics/x"), "Functions", "a folder name must match whole");
    assert_eq!(function_category("Loose"), "Functions");
}

/// The isolated bundled catalog has unique entries in recognized categories.
#[test]
fn the_bundled_library_fills_the_tree_with_what_materialcraft_has() {
    let library = fixture::library();
    assert!(library.layouts.is_empty());
    assert!(library.packages.contains_key("sbs://functions.sbs"));

    let entries = library.entries(&library.packages);
    let ids: BTreeSet<_> = entries.iter().map(|entry| entry.id.as_str()).collect();
    assert_eq!(ids.len(), entries.len(), "ids are unique");
    assert!(entries.iter().all(|entry| entry.missing.is_none()), "everything listed can be added");
    let known = |category: &str| {
        let mut parts = category.split('/');
        let top = parts.next().unwrap_or("");
        CATEGORIES.iter().any(|(name, subs)| *name == top && parts.next().is_none_or(|sub| subs.contains(&sub)))
    };
    for entry in &entries {
        assert!(known(&entry.category), "{} sits in unknown category {}", entry.id, entry.category);
    }
    let count = |category: &str| entries.iter().filter(|entry| entry.category == category).count();
    let find = |id: &str| entries.iter().find(|entry| entry.id == id).unwrap_or_else(|| panic!("no entry {id}"));

    for top in ["Function nodes", "Functions"] {
        let (_, subs) = CATEGORIES.iter().find(|(name, _)| *name == top).unwrap();
        for sub in *subs {
            assert!(count(&format!("{top}/{sub}")) > 0, "{top}/{sub} is empty");
        }
    }

    // Every bundled function has a sub-category
    let functions = library.packages["sbs://functions.sbs"].graphs.iter().filter(|graph| graph.function.is_some()).count();
    assert_eq!(entries.iter().filter(|entry| entry.category.starts_with("Functions/")).count(), functions);
    assert_eq!(count("Functions"), 0);
    assert_eq!(find("sbs://functions.sbs#Functions/Tonemappers/tonemap_ACES").category, "Functions/Color");
    assert_eq!(
        entries.iter().filter(|entry| entry.category == "Graph items").map(|entry| entry.label.as_str()).collect::<Vec<_>>(),
        ["Comment", "Frame", "Pin", "Dot node"]
    );
    for id in ["node:blur", "node:bitmap", "node:output", "node:input_color", "node:input_value"] {
        assert_eq!(find(id).category, "Atomic nodes", "{id}");
    }
    assert_eq!(find("function:add").category, "Function nodes/Operator");
    assert_eq!(find("pattern:paramset").category, "FxMap nodes");
    let pi = find("sbs://functions.sbs#Functions/Math/Pi");
    assert_eq!((pi.label.as_str(), pi.category.as_str()), ("Pi", "Functions/Constant"));
    assert_eq!(find("sbs://functions.sbs#Functions/Math/Equality_Float2").category, "Functions/Comparison");
}

/// A layout that places a library node MaterialCraft doesn't have yet, a function and a sub-category
/// the PBR keywords decide
const LAYOUT: &str = r#"<package><dependencies>
    <dependency><filename v="sbs://3d_perlin_noise.sbs"/><uid v="2"/></dependency>
    <dependency><filename v="sbs://materials/bricks_001.sbs"/><uid v="3"/></dependency></dependencies>
    <content><graph><identifier v="Layout"/><compNodes>
      <compNode><uid v="10"/><GUILayout><gpos v="20 40 0"/></GUILayout><compImplementation><compInstance><path v="pkg:///3D_perlin_noise?dependency=2"/></compInstance></compImplementation></compNode>
      <compNode><uid v="11"/><GUILayout><gpos v="220 40 0"/></GUILayout><compImplementation><compInstance><path v="pkg:///bricks_001?dependency=3"/></compInstance></compImplementation></compNode>
    </compNodes><GUIObjects>
      <GUIObject><type v="COMMENT"/><GUILayout><gpos v="0 0 0"/><size v="160 200"/></GUILayout><title v="Texture Generators - Noises"/></GUIObject>
      <GUIObject><type v="COMMENT"/><GUILayout><gpos v="200 0 0"/><size v="160 200"/></GUILayout><title v="PBR Materials"/></GUIObject>
    </GUIObjects></graph></content></package>"#;

/// A missing layout entry becomes available when its package is supplied.
#[test]
fn a_layout_lists_what_it_places_and_a_library_package_makes_it_available() {
    let frames = materialcraft_formats::sbs::read_layout(LAYOUT.as_bytes(), "sbs://layout.sbs").unwrap();
    let library = NodeLibrary { placements: placements(&frames), ..Default::default() };
    let entries = library.entries(&BTreeMap::new());
    let perlin = entries.iter().find(|entry| entry.id == "sbs://3d_perlin_noise.sbs#3D_perlin_noise").unwrap();
    assert_eq!((perlin.label.as_str(), perlin.category.as_str()), ("3D Perlin Noise", "Texture generators/Noises"));
    assert!(perlin.missing.is_some(), "a placed node whose package is absent is listed but not addable");
    let bricks = entries.iter().find(|entry| entry.id == "sbs://materials/bricks_001.sbs#bricks_001").unwrap();
    assert_eq!(bricks.category, "PBR materials/Bricks");

    let mut graph = Graph::new("3D_perlin_noise");
    graph.add_node("uniform", [0.0; 2]);
    graph.add_node("output", [200.0, 0.0]);
    let mut package = Package::new("3d_perlin_noise");
    package.graphs.push(graph.clone());
    graph.name = "Loose".into();
    graph.attributes.category = "Filters/Blurs".into();
    package.graphs.push(graph);
    let dependencies = BTreeMap::from([("sbs://3d_perlin_noise.sbs".to_string(), Arc::new(package))]);

    let entries = library.entries(&dependencies);
    let perlin = entries.iter().find(|entry| entry.id == "sbs://3d_perlin_noise.sbs#3D_perlin_noise").unwrap();
    assert!(perlin.missing.is_none());
    assert_eq!(perlin.category, "Texture generators/Noises");
    let loose = entries.iter().find(|entry| entry.id == "sbs://3d_perlin_noise.sbs#Loose").unwrap();
    assert_eq!((loose.category.as_str(), loose.canvas), ("Filters/Blurs", LibraryCanvas::Material));
}

/// Invalid library files produce warnings while native entries remain usable.
#[test]
fn unreadable_library_files_warn_instead_of_failing() {
    let library = NodeLibrary::load(&["sbs://does_not_exist.sbs".into(), "sbs://../escape.sbs".into()]);
    assert!(library.packages.is_empty() && library.layouts.is_empty());
    assert_eq!(library.warnings.len(), 2);
    let entries = library.entries(&BTreeMap::new());
    assert!(entries.iter().any(|entry| entry.id == "node:blur" && entry.category == "Atomic nodes"));
    assert!(entries.iter().any(|entry| entry.id == "function:add" && entry.category == "Function nodes/Operator"));
}

/// Classification tags take precedence over package folders and support multiple placements.
#[test]
fn function_tags_choose_library_filters_before_path_fallbacks() {
    let mut package = Package::new("Functions");
    for (name, category, tags) in [("Pi", "Function", "Maths"), ("Tagged", "", "Comparison; Easing"), ("Fallback", "", "Unknown")] {
        let mut graph = Graph::new(name);
        graph.folder = "Functions/Math".into();
        graph.function = Some(Default::default());
        graph.attributes.category = category.into();
        graph.attributes.tags = tags.into();
        package.graphs.push(graph);
    }
    let dependencies = BTreeMap::from([("sbs://classification.sbs".to_string(), Arc::new(package))]);

    let entries = NodeLibrary::default().entries(&dependencies);
    let categories = |name: &str| entries.iter().filter(|entry| entry.label == name).map(|entry| entry.category.as_str()).collect::<Vec<_>>();

    assert_eq!(categories("Pi"), ["Functions/Maths"]);
    assert_eq!(categories("Tagged"), ["Functions/Comparison", "Functions/Easings"]);
    assert_eq!(categories("Fallback"), ["Functions/Maths"]);
}
