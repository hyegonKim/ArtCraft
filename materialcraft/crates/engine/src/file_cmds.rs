pub use crate::Exported;
pub use crate::linked_resource::reader as linked_files;

use serde_json::{Value as J, json};

use materialcraft_core::{Package, Resource, ResourceBytes};
use materialcraft_formats::{Depth, encode, output_name};

use crate::commands::{CommandSpec, always, bad, has_graph, has_package, strp, u64p};
use crate::samples::{TEMPLATES, template};
use crate::{ExportRequest, Result, Session, cmd};

pub(crate) fn specs() -> Vec<CommandSpec> {
    vec![
        cmd!(
            "file.new",
            "Graph...",
            ["File", "New"],
            Some("Ctrl+N"),
            "{name?, folder?, kind? (material|function), template? (see graph.templates), size? (px, power of two)} adds a new graph to the package and opens it",
            always,
            new_graph
        ),
        cmd!("file.newPackage", "Empty package", ["File", "New"], None, "{name?} opens a new package alongside the existing packages", always, new_package),
        cmd!(
            "file.open",
            "Open...",
            ["File"],
            Some("Ctrl+O"),
            "{path} opens an SBS or MaterialCraft package; returns `warnings` and `path` (null when unsupported content requires Save As)",
            always,
            open
        ),
        cmd!("file.save", "Save", ["File"], Some("Ctrl+S"), "{path?} saves .sbs with adjacent resources or an independent .mcraft archive", has_package, save),
        cmd!("file.saveAs", "Save as...", ["File"], Some("Ctrl+Shift+S"), "{path} saves the package under a new name", has_package, save),
        cmd!(
            "file.export",
            "Export outputs...",
            ["File"],
            Some("Ctrl+Shift+E"),
            "{dir, format? (png|jpg|tga|bmp|webp|exr|hdr), pattern? ($(graph)_$(identifier)), depth? (8|16|32), size? (px of the larger side, or [width, height]; rounded down to powers of two, 16..8192; default the graph's size, keeping its proportions), outputs? ([identifier]: default every connected output), colorSpaces? ({identifier: srgb|linear|raw}: default sRGB for color usages, raw for data; linear and raw write values unchanged), graph?} cooks the graph at full size and writes the outputs",
            has_graph,
            export
        ),
        cmd!(
            "file.importBitmap",
            "Import bitmap...",
            ["File"],
            None,
            "{path, name?, folder?} imports an image into Resources or its subfolders; returns its name without adding a graph node",
            has_package,
            import_bitmap
        ),
        cmd!("graph.templates", "Graph templates", [], None, "{} lists the templates for file.new", always, |_, _| Ok(json!(
            TEMPLATES.iter().map(|(id, l)| json!({"id": id, "label": l})).collect::<Vec<_>>()
        ))),
    ]
}

fn new_graph(session: &mut Session, parameters: &J) -> Result<J> {
    let kind = match strp(parameters, "kind").unwrap_or("material") {
        "material" => materialcraft_core::GraphKind::Material,
        "function" => materialcraft_core::GraphKind::Function,
        _ => return Err(bad("file.new", "kind must be material or function")),
    };
    let template_id = strp(parameters, "template").unwrap_or("pbr");
    let label = if kind == materialcraft_core::GraphKind::Function {
        "Function Graph"
    } else {
        TEMPLATES.iter().find(|entry| entry.0 == template_id).map_or("Graph", |entry| entry.1)
    };
    let size = u64p(parameters, "size").unwrap_or(1024).clamp(16, 8192);
    let log2 = 63 - size.leading_zeros();
    let base = strp(parameters, "name").map(str::to_string).unwrap_or_else(|| label.replace(' ', "_"));
    let folder = strp(parameters, "folder").unwrap_or("");
    crate::folder_path::require(&session.pkg, folder)?;
    crate::folder_path::require_graph_folder(folder)?;
    let base = crate::folder_path::name(&base)?;

    let name = crate::folder_path::unique(&session.pkg, folder, base);
    let mut graph = if kind == materialcraft_core::GraphKind::Function {
        let mut graph = materialcraft_core::Graph::new(&name);
        graph.function = Some(materialcraft_core::FunctionGraph::default());
        graph
    } else {
        template(template_id, &name, log2)?
    };
    graph.folder = folder.into();
    if !session.has_package() {
        session.open_package(Package::new("Untitled package"), None)?;
    }
    session.edit("New graph", |session| {
        session.pkg.graphs.push(graph);
        session.graph = session.pkg.graphs.len() - 1;
        session.open_material_preview();
        session.selection.clear();
        session.annotation_selection = None;
        session.view2d.node = None;
        Ok(json!({"graph": session.graph, "name": name, "kind": kind}))
    })
}

fn new_package(s: &mut Session, p: &J) -> Result<J> {
    let mut pkg = Package::new(strp(p, "name").unwrap_or("Untitled package"));
    pkg.graphs.push(template("pbr", "New_graph", 10)?);
    s.open_package(pkg, None)?;
    Ok(json!({"name": s.pkg.name}))
}

/// Replace the active document explicitly, resetting its history and views
pub fn replace_package(s: &mut Session, pkg: Package, path: Option<String>) {
    // Reloaded archive dependencies must replace cached snapshots before links are refreshed
    s.linked_sources.extend(pkg.dependencies.clone());
    s.pkg = pkg;
    s.document_open = true;
    s.clipboard = None;
    s.embedded_clipboard = None;
    s.clear_decoded();
    s.path = path;
    s.graph = 0;
    s.material_preview = crate::MaterialPreview::for_graph(&s.pkg, 0);
    s.clear_preview_results();
    s.selection.clear();
    s.annotation_selection = None;
    s.view2d.node = None;
    s.reset_history();
    s.modified = false;
    s.dirty = true;
    s.load_warnings.clear();
}

fn has_extension(path: &str, ext: &str) -> bool {
    path.rsplit_once('.').is_some_and(|(_, e)| e.eq_ignore_ascii_case(ext))
}

pub fn package_path(path: &str) -> String {
    if has_extension(path, materialcraft_formats::sbs::SBS_EXT) || is_mcraft(path) { path.to_string() } else { format!("{path}.sbs") }
}

pub fn is_mcraft(path: &str) -> bool {
    has_extension(path, materialcraft_formats::project::EXT)
}

pub fn open_bytes(session: &mut Session, bytes: &[u8], name: Option<String>) -> Result<()> {
    open_bytes_linked(session, bytes, name, &|_| None)
}

pub fn open_bytes_linked(session: &mut Session, bytes: &[u8], name: Option<String>, linked: &dyn Fn(&str) -> Option<Vec<u8>>) -> Result<()> {
    let name = name.map(|path| crate::document_path::normalize(&path));
    if let Some(name) = &name
        && let Some((id, ..)) = session.open_packages().into_iter().find(|entry| entry.2 == Some(name.as_str()))
    {
        return session.activate_document(id);
    }

    if materialcraft_formats::material::is_material(bytes) {
        let (manifest, source) = materialcraft_formats::material::read_material(bytes)?;
        session.open_package(materialcraft_formats::sbs::read_sbs(&source, "source.sbs", &|_| None)?.pkg, None)?;
        session.graph = session.pkg.graph_by_name(&manifest.graph).map_or(0, |(index, _)| index);
        session.open_material_preview();
        return Ok(());
    }

    let mut read = read_package_bytes(bytes, name.as_deref().unwrap_or(""), linked)?;
    for (path, source) in &session.pkg.dependencies {
        read.pkg.dependencies.entry(path.clone()).or_insert_with(|| source.clone());
    }
    let link_warnings = crate::linked_files::load(&mut read.pkg);
    let path = if read.warnings.is_empty() { name } else { None };
    read.warnings.extend(link_warnings);
    session.open_package(read.pkg, path)?;
    for warning in &read.warnings {
        session.warn(warning.clone());
    }
    session.load_warnings = read.warnings;
    Ok(())
}

pub(crate) fn read_package_bytes(bytes: &[u8], name: &str, linked: &dyn Fn(&str) -> Option<Vec<u8>>) -> Result<materialcraft_formats::sbs::SbsRead> {
    if materialcraft_formats::project::is_archive(bytes) || is_mcraft(name) {
        return Ok(materialcraft_formats::project::read(bytes, name)?);
    }

    let mut read = materialcraft_formats::sbs::read_sbs(bytes, name, linked)?;
    if !name.is_empty() {
        crate::linked_files::normalize_references(&mut read.pkg, name);
    }
    Ok(read)
}

pub fn save_bytes(session: &Session) -> Result<Vec<u8>> {
    save_bytes_at(session, session.path.as_deref())
}

pub(crate) fn save_bytes_at(session: &Session, path: Option<&str>) -> Result<Vec<u8>> {
    session.package(session.document_id)?;
    if let Some(path) = path.filter(|path| is_mcraft(path)) {
        return Ok(save_files_at(session, path)?.zip()?);
    }

    // Byte-only consumers (portable materials and recovery) retain standalone embedded SBS
    Ok(materialcraft_formats::sbs::write_sbs(&package_for_save(session, path))?)
}

fn package_for_save(session: &Session, path: Option<&str>) -> Package {
    let mut package = session.pkg.clone();
    if let Some(path) = path {
        for resource in &mut package.resources {
            if !resource.source.is_empty() {
                resource.source = crate::document_path::relative(path, &resource.source);
            }
        }

        for instance in package.graphs.iter_mut().flat_map(|graph| graph.instances_mut()) {
            if !instance.package.is_empty() {
                instance.package = crate::document_path::relative(path, &instance.package);
            }
        }
    }
    package
}

pub fn save_files_at(session: &Session, path: &str) -> Result<materialcraft_formats::project::ProjectFiles> {
    session.package(session.document_id)?;
    let archive = is_mcraft(path);
    let package = if archive { session.pkg.clone() } else { package_for_save(session, Some(path)) };
    let filename = std::path::Path::new(path).file_name().and_then(|name| name.to_str()).unwrap_or("package.sbs");
    Ok(materialcraft_formats::project::ProjectFiles::build(&package, filename, archive)?)
}

#[cfg(not(target_arch = "wasm32"))]
pub fn write_package_at(session: &Session, path: &str) -> Result<usize> {
    let files = save_files_at(session, path)?;
    if is_mcraft(path) {
        let bytes = files.zip()?;
        materialcraft_formats::write_atomic(std::path::Path::new(path), &bytes)?;
        Ok(bytes.len())
    } else {
        materialcraft_formats::project::write_files(std::path::Path::new(path), &files)?;
        Ok(files.xml()?.len())
    }
}

/// Open the package file at `path` (native), with the bitmaps an SBS package keeps next to it
#[cfg(not(target_arch = "wasm32"))]
pub fn open_file(s: &mut Session, path: &str) -> Result<()> {
    if path.starts_with("sbs://") {
        let bytes = materialcraft_formats::sbs::library::read(path).map_err(|error| bad("file.open", error))?;
        return open_bytes_linked(s, &bytes, Some(path.into()), &linked_files(path));
    }

    let bytes = std::fs::read(path).map_err(|e| bad("file.open", format!("{path}: {e}")))?;
    open_bytes_linked(s, &bytes, Some(path.to_string()), &linked_files(path))
}

#[cfg(not(target_arch = "wasm32"))]
fn open(s: &mut Session, p: &J) -> Result<J> {
    let path = strp(p, "path").ok_or_else(|| bad("file.open", "`path` is required"))?;
    open_file(s, path)?;
    s.log(format!("Opened {path}"));
    Ok(json!({"name": s.pkg.name, "graphs": s.pkg.graphs.len(), "path": s.path, "warnings": s.load_warnings}))
}

#[cfg(target_arch = "wasm32")]
fn open(session: &mut Session, parameters: &J) -> Result<J> {
    let path = strp(parameters, "path")
        .filter(|path| path.starts_with("sbs://"))
        .ok_or_else(|| bad("file.open", "the browser opens local files with the file picker"))?;
    let bytes = materialcraft_formats::sbs::library::read(path).map_err(|error| bad("file.open", error))?;
    open_bytes_linked(session, &bytes, Some(path.into()), &linked_files(path))?;

    Ok(json!({"name": session.pkg.name, "graphs": session.pkg.graphs.len(), "path": session.path, "warnings": session.load_warnings}))
}

#[cfg(not(target_arch = "wasm32"))]
fn save(s: &mut Session, p: &J) -> Result<J> {
    let path = match strp(p, "path") {
        Some(path) => package_path(path),
        None => s.path.clone().ok_or_else(|| bad("file.save", "the package has no file yet (new, or opened from another program): give a `path`"))?,
    };

    let path = crate::document_path::normalize(&path);
    if path.contains("://") {
        return Err(bad("file.save", "library aliases cannot be overwritten; save to a local .sbs path"));
    }

    if s.open_packages().into_iter().any(|entry| entry.0 != s.document_id && entry.2 == Some(path.as_str())) {
        return Err(bad("file.save", "another open package already uses this file"));
    }

    let bytes = write_package_at(s, &path)?;
    s.path = Some(path.clone());
    s.refresh_links();
    s.modified = false;
    s.log(format!("Saved {path}"));
    Ok(json!({"path": path, "bytes": bytes}))
}

#[cfg(target_arch = "wasm32")]
fn save(_: &mut Session, _: &J) -> Result<J> {
    Err(bad("file.save", "the browser saves with a download"))
}

/// Cook the requested graph at full size and encode its requested outputs (`pattern` names files)
pub fn export_outputs(s: &Session, request: &ExportRequest) -> Result<Vec<Exported>> {
    let mut package = s.pkg.clone();
    if let Some(size) = request.size {
        let graph = package.graph_mut(request.graph)?;
        graph.size = size.log2_size(graph.size);
    }

    let graph = package.graph(request.graph)?;
    let decoder = s.resource_decoder();
    let mut options = materialcraft_eval::Options::new(&decoder);
    options.environment = s.environment;
    let cooked = materialcraft_eval::cook(&package, request.graph, &options, &s.cache);

    let mut files = Vec::new();
    let mut names = std::collections::HashSet::new();
    for (id, usage, identifier, image) in materialcraft_eval::outputs(graph, &cooked) {
        let Some(image) = image.filter(|_| request.includes(&identifier)) else { continue };

        let file = format!("{}.{}", output_name(&request.pattern, &graph.name, &identifier, usage), request.format);
        if !names.insert(file.to_lowercase()) {
            return Err(bad("file.export", format!("duplicate export filename `{file}`: use unique output identifiers or a different filename pattern")));
        }

        let node = graph.node(id)?;
        let output_format = if node.base.format == materialcraft_core::OutputFormat::Inherit { graph.format } else { node.base.format };
        let depth = request.depth.unwrap_or_else(|| Depth::from_format(output_format));
        let bytes = encode(image, &request.format, depth, request.color_space(&identifier, usage).encodes_srgb())?;
        files.push(Exported { file, bytes });
    }

    if files.is_empty() {
        let reason = if request.outputs.is_some() { "the selected outputs are not connected" } else { "the graph has no connected output nodes" };
        return Err(bad("file.export", reason));
    }
    Ok(files)
}

#[cfg(not(target_arch = "wasm32"))]
fn export(s: &mut Session, p: &J) -> Result<J> {
    let dir = strp(p, "dir").ok_or_else(|| bad("file.export", "`dir` is required"))?;
    let files = export_outputs(s, &ExportRequest::from_params(s, p)?)?;
    let folder = std::path::Path::new(dir);
    std::fs::create_dir_all(folder).map_err(|error| bad("file.export", format!("{dir}: {error}")))?;

    // Check every name first so a rejected export leaves the folder untouched
    if let Some(existing) = files.iter().find(|exported| folder.join(&exported.file).exists()) {
        return Err(bad(
            "file.export",
            format!("duplicate export filename `{}`: use unique output identifiers or a different filename pattern", existing.file),
        ));
    }

    let mut written = Vec::new();
    for exported in files {
        let path = folder.join(&exported.file);
        materialcraft_formats::write_atomic(&path, &exported.bytes)?;
        written.push(path.display().to_string());
    }
    s.log(format!("Exported {} files to {dir}", written.len()));
    Ok(json!({"files": written}))
}

#[cfg(target_arch = "wasm32")]
fn export(_: &mut Session, _: &J) -> Result<J> {
    Err(bad("file.export", "the browser exports with downloads"))
}

/// Embed image bytes as a resource; returns the resource name
pub fn add_resource(s: &mut Session, file: &str, bytes: Vec<u8>, name: Option<&str>) -> Result<String> {
    add_resource_into(s, file, bytes, name, "Resources")
}

/// Import original image bytes into an existing asset folder in one undo step.
pub fn add_resource_into(s: &mut Session, file: &str, bytes: Vec<u8>, name: Option<&str>, folder: &str) -> Result<String> {
    crate::folder_path::require_resource_folder(&s.pkg, folder)?;
    let extension = file.rsplit_once('.').map_or("", |(_, extension)| extension);
    validate_resource_extension(file, extension.eq_ignore_ascii_case("svg"))?;
    materialcraft_formats::decode(&bytes, false)?;
    s.edit("Import resource", |s| {
        let file_name = materialcraft_formats::safe_file_name(file);
        let base = name.unwrap_or_else(|| file_name.rsplit_once('.').map_or(file_name.as_str(), |(stem, _)| stem));
        crate::folder_path::name(base)?;
        let name = resource_name(&s.pkg, folder, base);
        let name = push_resource(&mut s.pkg, file, bytes, Some(&name));
        if let Some(resource) = s.pkg.resources.iter_mut().find(|resource| resource.name == name) {
            resource.folder = folder.into();
        }
        Ok(name)
    })
}

pub(crate) fn resource_name(package: &Package, folder: &str, base: &str) -> String {
    let occupied: std::collections::BTreeSet<_> = package
        .resources
        .iter()
        .map(|resource| resource.name.clone())
        .chain(
            package
                .folder_paths()
                .into_iter()
                .filter_map(|path| path.rsplit_once('/').filter(|(parent, _)| *parent == folder).map(|(_, name)| name.to_string())),
        )
        .chain(package.graphs.iter().filter(|graph| graph.folder == folder).map(|graph| graph.name.clone()))
        .collect();
    (0..=occupied.len())
        .map(|index| if index == 0 { base.to_string() } else { format!("{base}_{index}") })
        .find(|name| !occupied.contains(name))
        .unwrap_or_else(|| base.to_string())
}

/// Add `bytes` to `pkg` as a resource named `name` (or after `file`), made unique; returns the name
pub(crate) fn push_resource(pkg: &mut Package, file: &str, bytes: Vec<u8>, name: Option<&str>) -> String {
    let file = materialcraft_formats::safe_file_name(file);
    let stem = name.map(str::to_string).unwrap_or_else(|| file.rsplit_once('.').map_or(file.clone(), |(a, _)| a.to_string()));
    let mut name = stem.clone();
    let mut i = 1;
    while pkg.resource(&name).is_some() {
        name = format!("{stem}_{i}");
        i += 1;
    }
    pkg.resources.push(Resource::new(name.clone(), format!("{name}_{file}"), ResourceBytes::new(bytes)));
    name
}

#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn import_resource_file(s: &mut Session, p: &J, vector: bool) -> Result<J> {
    let path = strp(p, "path").ok_or_else(|| bad("file.importBitmap", "`path` is required"))?;
    validate_resource_extension(path, vector)?;
    let bytes = std::fs::read(path).map_err(|e| bad("file.importBitmap", format!("{path}: {e}")))?;
    let file = std::path::Path::new(path).file_name().and_then(|f| f.to_str()).unwrap_or("bitmap.png").to_string();
    let name = add_resource_into(s, &file, bytes, strp(p, "name"), strp(p, "folder").unwrap_or("Resources"))?;
    let def = if file.to_ascii_lowercase().ends_with(".svg") { "svg" } else { "bitmap" };
    Ok(json!({"name": name, "use": {"command": "graph.node.add", "params": {"def": def, "params": {"resource": name}}}}))
}

#[cfg(target_arch = "wasm32")]
pub(crate) fn import_resource_file(_: &mut Session, _: &J, _: bool) -> Result<J> {
    Err(bad("file.importBitmap", "the browser imports with the file picker"))
}

fn import_bitmap(session: &mut Session, parameters: &J) -> Result<J> {
    import_resource_file(session, parameters, false)
}

pub fn validate_resource_extension(file: &str, vector: bool) -> Result<()> {
    let extension = file.rsplit_once('.').map_or("", |(_, extension)| extension).to_ascii_lowercase();
    let supported = if vector { materialcraft_formats::VECTOR_EXTENSIONS } else { materialcraft_formats::BITMAP_EXTENSIONS };
    if !supported.contains(&extension.as_str()) {
        return Err(bad(
            if vector { "file.importVector" } else { "file.importBitmap" },
            if vector { "Choose an SVG file" } else { "Choose a supported bitmap format" },
        ));
    }

    Ok(())
}
