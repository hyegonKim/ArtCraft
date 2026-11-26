//! The session: the open package, the graph being edited, selection, views, snapshot undo and
//! the cook cache.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, PoisonError};

use materialcraft_core::{Graph, NodeId, Package, ResourceBytes};
use materialcraft_eval::{Cache, Cooked, Options};
use materialcraft_image::Image;
use serde::{Deserialize, Serialize};

use crate::snapshot::Snapshot;
use crate::{AnnotationSelection, Clip, EngineError, Result};

/// The 3D view's scene and look.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct View3d {
    /// Built-in mesh: `rounded_cube`, `cube`, `sphere`, `cylinder`, `plane`, `torus`, `material_ball`.
    pub scene: String,
    /// UV repeats of the material on the mesh.
    pub tiling: f32,
    /// Camera yaw and pitch (degrees) and distance (mesh radii).
    pub yaw: f32,
    pub pitch: f32,
    pub distance: f32,
    /// Where the camera orbits: offset from the mesh center (mesh radii, world axes).
    pub target: [f32; 3],
    /// Environment: `studio`, `sunset`, `overcast`, `night`.
    pub environment: String,
    pub env_rotation: f32,
    pub exposure: f32,
    /// Height relief in mesh radii
    pub height_scale: f32,
    pub height_level: f32,
    pub normal_y_flip: bool,
    pub show_axis: bool,
    pub show_grid: bool,
    /// See through the mesh where the material's alpha (opacity output, else base color alpha)
    /// is below one.
    pub transparency: bool,
    /// Show the material's alpha channel on the mesh instead of the lit material.
    pub alpha_view: bool,
}

impl Default for View3d {
    fn default() -> Self {
        View3d {
            scene: "rounded_cube".into(),
            tiling: 1.0,
            yaw: 35.0,
            pitch: 25.0,
            distance: 3.2,
            target: [0.0; 3],
            environment: "studio".into(),
            env_rotation: 0.0,
            exposure: 0.0,
            height_scale: 0.04,
            height_level: 0.0,
            normal_y_flip: false,
            show_axis: false,
            show_grid: false,
            transparency: true,
            alpha_view: false,
        }
    }
}

/// The 3D view's camera preferences. The app keeps them in its preferences (Edit ▸ Preferences)
/// and pushes them in with [`Session::set_camera_prefs`]; headless sessions use the defaults.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct CameraPrefs {
    /// How far the orbit target may pan from the mesh center, in mesh radii.
    pub pan_limit: f32,
    /// Pan speed: 1 keeps the mesh under the pointer.
    pub pan_speed: f32,
}

impl Default for CameraPrefs {
    fn default() -> Self {
        CameraPrefs { pan_limit: 5.0, pan_speed: 1.0 }
    }
}

impl CameraPrefs {
    /// These preferences with any non-finite or negative value replaced by its default.
    pub fn sanitized(self) -> Self {
        let d = CameraPrefs::default();
        let or = |v: f32, default: f32| if v.is_finite() && v >= 0.0 { v } else { default };
        CameraPrefs { pan_limit: or(self.pan_limit, d.pan_limit), pan_speed: or(self.pan_speed, d.pan_speed) }
    }

    /// An orbit target held within the pan limit (non-finite parts become 0).
    pub fn clamp_target(&self, t: [f32; 3]) -> [f32; 3] {
        let limit = self.sanitized().pan_limit;
        t.map(|v| if v.is_finite() { v.clamp(-limit, limit) } else { 0.0 })
    }
}

/// What the 2D view shows.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct View2d {
    pub node: Option<NodeId>,
    pub port: usize,
    /// Repeat the image 3×3 to check tiling.
    pub tiled: bool,
    /// `rgba`, `r`, `g`, `b`, `a`.
    pub channel: String,
    /// Overlay the luminance/channel histogram.
    pub histogram: bool,
}

/// The prefix of warning lines in [`Session::log`].
pub const WARNING_PREFIX: &str = "Warning: ";

pub struct Session {
    pub environment: materialcraft_core::EvaluationEnvironment,
    pub document_id: u64,
    /// False after closing the last document; `pkg` then holds only cleared storage.
    pub(crate) document_open: bool,
    pub(crate) next_document_id: u64,
    pub(crate) documents: std::collections::BTreeMap<u64, crate::OpenDocument>,
    pub(crate) linked_sources: std::collections::BTreeMap<String, Arc<Package>>,
    /// Snapshots inserted by the library, excluding explicitly supplied dependencies
    pub(crate) library_sources: std::collections::BTreeMap<String, Arc<Package>>,
    pub pkg: Package,
    /// Where the package was opened from or last saved to.
    pub path: Option<String>,
    /// Index of the graph being edited.
    pub graph: usize,
    pub selection: Vec<NodeId>,
    pub annotation_selection: Option<AnnotationSelection>,
    pub annotation_target: crate::AnnotationTarget,
    pub view2d: View2d,
    pub view3d: View3d,
    pub material_preview: crate::MaterialPreview,
    pub(crate) cooked_graph: Option<usize>,
    pub(crate) material_cooked: std::collections::BTreeMap<usize, Cooked>,
    /// Cap on node output size while editing (`None` = full size).
    pub preview_size: Option<u32>,
    /// 3D view camera preferences (set with [`Session::set_camera_prefs`]).
    pub camera: CameraPrefs,
    pub history_prefs: crate::HistoryPrefs,
    pub modified: bool,
    pub log: Vec<String>,
    /// What the last opened SBS package held that MaterialCraft couldn't read; empty after a clean open.
    pub load_warnings: Vec<String>,
    pub clipboard: Option<Clip>,
    pub embedded_clipboard: Option<crate::EmbeddedClipboard>,
    pub explorer_clipboard: Option<crate::ExplorerClipboard>,
    /// The last cook of the open graph.
    pub cooked: Cooked,
    /// Set when the graph changed and needs cooking.
    pub dirty: bool,
    /// Synchronous cooks invalidate older worker results
    pub cook_epoch: u64,
    /// Cooked node outputs, shared with background cooks.
    pub cache: Arc<Mutex<Cache>>,
    /// A mesh loaded for the 3D view (`scene: "custom"`): its file name and geometry.
    pub custom_mesh: Option<(String, Arc<materialcraft_geom::Mesh>)>,
    /// The node library loaded from the SBS library folder
    pub library: Arc<crate::node_library::NodeLibrary>,
    pub(crate) history: crate::document_history::DocumentHistory,
    group: Option<(Snapshot, bool)>,
    decoded: Arc<Mutex<HashMap<(u64, bool), Image>>>,
}

impl Session {
    /// Open a package with the process's installed node library.
    pub fn new(pkg: Package) -> Session {
        Self::with_library(pkg, crate::node_library::NodeLibrary::shared())
    }

    /// Open a package with an explicit library, preserving dependencies supplied by the caller.
    pub fn with_library(mut pkg: Package, library: Arc<crate::NodeLibrary>) -> Session {
        let mut library_sources = std::collections::BTreeMap::new();
        for (path, package) in &library.packages {
            if let std::collections::btree_map::Entry::Vacant(entry) = pkg.dependencies.entry(path.clone()) {
                entry.insert(package.clone());
                library_sources.insert(path.clone(), package.clone());
            }
        }
        let mut session = Session {
            environment: Default::default(),
            document_id: 0,
            document_open: true,
            next_document_id: 1,
            documents: std::collections::BTreeMap::new(),
            linked_sources: pkg.dependencies.clone(),
            library_sources,
            material_preview: crate::MaterialPreview::for_graph(&pkg, 0),
            cooked_graph: None,
            material_cooked: std::collections::BTreeMap::new(),
            pkg,
            path: None,
            graph: 0,
            selection: Vec::new(),
            annotation_selection: None,
            annotation_target: Default::default(),
            view2d: View2d { channel: "rgba".into(), ..View2d::default() },
            view3d: View3d::default(),
            preview_size: Some(512),
            camera: CameraPrefs::default(),
            history_prefs: crate::HistoryPrefs::default(),
            modified: false,
            log: Vec::new(),
            load_warnings: Vec::new(),
            clipboard: None,
            embedded_clipboard: None,
            explorer_clipboard: None,
            cooked: Cooked::default(),
            dirty: true,
            cook_epoch: 0,
            cache: Arc::new(Mutex::new(Cache::default())),
            custom_mesh: None,
            library: Default::default(),
            history: Default::default(),
            group: None,
            decoded: Arc::new(Mutex::new(HashMap::new())),
        };
        session.refresh_links();
        for warning in &library.warnings {
            session.warn(warning.clone());
        }
        if !library.packages.contains_key(crate::function_library::STANDARD_PACKAGE) {
            session.warn(format!("Missing function library: {} did not load", crate::function_library::STANDARD_PACKAGE));
        }
        session.library = library;
        session
    }

    /// The graph being edited.
    pub fn graph(&self) -> Result<&Graph> {
        self.pkg.graph(self.graph).map_err(EngineError::from)
    }

    pub fn graph_mut(&mut self) -> Result<&mut Graph> {
        self.pkg.graph_mut(self.graph).map_err(EngineError::from)
    }

    fn snapshot(&self, label: &str) -> Snapshot {
        Snapshot {
            pkg: self.pkg.clone(),
            graph: self.graph,
            selection: self.selection.clone(),
            annotation_selection: self.annotation_selection,
            annotation_target: self.annotation_target.clone(),
            material_preview: self.material_preview.clone(),
            label: label.into(),
            modified: self.modified,
            dirty: self.dirty,
            cook_epoch: self.cook_epoch,
        }
    }

    /// Run `f` as one undoable step. On error the session is restored.
    pub fn edit<T>(&mut self, label: &str, f: impl FnOnce(&mut Session) -> Result<T>) -> Result<T> {
        self.finish_edit();
        self.edit_inner(label, None, f)
    }

    /// Like [`Session::edit`], but consecutive edits with the same `key` (dragging one slider)
    /// make a single undo step.
    pub fn edit_merged<T>(&mut self, key: &str, label: &str, f: impl FnOnce(&mut Session) -> Result<T>) -> Result<T> {
        self.edit_inner(label, Some(key), f)
    }

    /// End the current gesture without adding an undo step
    pub fn finish_edit(&mut self) {
        self.history.finish_edit();
        for document in self.documents.values_mut() {
            document.history.finish_edit();
        }
    }

    pub fn begin_group(&mut self, label: &str) {
        if self.group.is_none() {
            self.finish_edit();
            self.group = Some((self.snapshot(label), false));
        }
    }

    pub fn end_group(&mut self) {
        if let Some((snap, edited)) = self.group.take()
            && edited
        {
            self.history.record(snap, None, self.history_prefs.undo_limit as usize);
            self.refresh_links();
        }
    }

    pub fn group<T>(&mut self, label: &str, f: impl FnOnce(&mut Session) -> Result<T>) -> Result<T> {
        if self.group.is_some() {
            let snap = self.snapshot(label);
            let edited = self.group.as_ref().is_some_and(|(_, edited)| *edited);
            let result = f(self);
            if result.is_err() {
                self.restore(snap);
                if let Some((_, flag)) = self.group.as_mut() {
                    *flag = edited;
                }
            }
            return result;
        }
        self.begin_group(label);
        let r = f(self);
        match r {
            Ok(_) => self.end_group(),
            Err(_) => {
                if let Some((snap, _)) = self.group.take() {
                    self.restore(snap);
                }
            }
        }
        r
    }

    fn edit_inner<T>(&mut self, label: &str, key: Option<&str>, f: impl FnOnce(&mut Session) -> Result<T>) -> Result<T> {
        if !self.has_package() {
            return Err(crate::commands::bad("edit", "no package is open"));
        }

        let snapshot = self.snapshot(label);
        match f(self) {
            Ok(value) => {
                if let Some((_, edited)) = self.group.as_mut() {
                    *edited = true;
                } else {
                    self.history.record(snapshot, key, self.history_prefs.undo_limit as usize);
                }
                self.changed();
                Ok(value)
            }
            Err(error) => {
                self.restore(snapshot);
                Err(error)
            }
        }
    }

    fn restore(&mut self, s: Snapshot) {
        self.pkg = s.pkg;
        self.graph = s.graph.min(self.pkg.graphs.len().saturating_sub(1));
        self.selection = s.selection;
        self.annotation_selection = s.annotation_selection;
        self.annotation_target = s.annotation_target;
        self.material_preview = s.material_preview;
        self.modified = s.modified;
        self.dirty = s.dirty || self.cook_epoch != s.cook_epoch;
        if self.cook_epoch != s.cook_epoch {
            self.clear_preview_results();
        }
    }

    /// Mark the document modified and in need of a cook.
    pub fn changed(&mut self) {
        self.modified = true;
        self.dirty = true;
        // Publish a grouped edit only after it succeeds, so rollback cannot dirty other packages
        if self.group.is_none() {
            self.refresh_links();
        }
    }

    pub fn can_undo(&self) -> bool {
        self.history.can_undo()
    }

    pub fn can_redo(&self) -> bool {
        self.history.can_redo()
    }

    /// Labels of the undo stack, oldest first.
    pub fn history(&self) -> Vec<String> {
        self.history.labels()
    }

    pub fn undo(&mut self) -> Option<String> {
        self.end_group();
        if !self.can_undo() {
            return None;
        }

        let s = self.history.undo(self.snapshot(""), self.history_prefs.undo_limit as usize)?;
        let label = s.label.clone();
        self.restore(s);
        self.finish_edit();
        self.dirty = true;
        self.refresh_links();
        Some(label)
    }

    pub fn redo(&mut self) -> Option<String> {
        self.end_group();
        if !self.can_redo() {
            return None;
        }

        let s = self.history.redo(self.snapshot(""), self.history_prefs.undo_limit as usize)?;
        let label = s.label.clone();
        self.restore(s);
        self.finish_edit();
        self.dirty = true;
        self.refresh_links();
        Some(label)
    }

    /// Forget undo history (after opening a package).
    pub fn reset_history(&mut self) {
        self.history.clear();
        self.group = None;
    }

    /// A decoder for the package's bitmap resources (memoized, shareable with worker threads).
    pub fn resource_decoder(&self) -> impl Fn(&str) -> Option<Image> + Sync + Send + 'static {
        let mut res: Vec<(String, ResourceBytes)> = self.pkg.resources.iter().map(|r| (r.name.clone(), r.bytes.clone())).collect();
        res.extend(
            self.pkg
                .dependencies
                .iter()
                .flat_map(|(file, package)| package.resources.iter().map(move |resource| (format!("{file}\0{}", resource.name), resource.bytes.clone()))),
        );
        let memo = self.decoded.clone();
        // Release decoded images of deleted or replaced resources
        memo.lock().unwrap_or_else(PoisonError::into_inner).retain(|(k, _), _| res.iter().any(|(_, b)| b.content_hash() == *k));
        move |requested: &str| {
            let (name, linear) = materialcraft_image::split_resource_key(requested);
            let (_, bytes) = res.iter().find(|(n, _)| n == name)?;
            let key = (bytes.content_hash(), linear);
            if let Some(i) = memo.lock().unwrap_or_else(PoisonError::into_inner).get(&key) {
                return Some(i.clone());
            }
            let img = materialcraft_formats::decode(bytes.as_slice(), linear).ok()?;
            memo.lock().unwrap_or_else(PoisonError::into_inner).insert(key, img.clone());
            Some(img)
        }
    }

    /// Forget every decoded resource image (the package was replaced).
    pub(crate) fn clear_decoded(&self) {
        self.decoded.lock().unwrap_or_else(PoisonError::into_inner).clear();
    }

    /// Cook the open graph now (at the preview size), on this thread.
    pub fn cook(&mut self) -> &Cooked {
        let dec = self.resource_decoder();
        let mut opts = Options::new(&dec);
        opts.environment = self.environment;
        opts.max_size = self.preview_size;
        let cooked = crate::PreviewCook::run(&self.pkg, self.graph, &self.material_preview.graphs(), &opts, &self.cache);
        self.accept_preview(cooked);
        self.dirty = false;
        self.cook_epoch = self.cook_epoch.wrapping_add(1);
        &self.cooked
    }

    /// Publish a completed editor and material cook together
    pub fn accept_preview(&mut self, result: crate::PreviewCook) {
        if result.graph != self.graph || result.cancelled() {
            return;
        }
        self.cooked_graph = Some(result.graph);
        self.cooked = result.cooked;
        self.material_cooked = result.materials;
    }

    /// Cook graph `gi` at full size, without touching the preview results.
    pub fn cook_full(&self, gi: usize) -> Cooked {
        let dec = self.resource_decoder();
        let mut opts = Options::new(&dec);
        opts.environment = self.environment;
        materialcraft_eval::cook(&self.pkg, gi, &opts, &self.cache)
    }

    /// Cook if the graph changed since the last cook.
    pub fn cook_if_dirty(&mut self) {
        if self.dirty {
            self.cook();
        }
    }

    /// The image the 2D view shows: the chosen node's port, else the first output node, else the
    /// last selected node.
    pub fn view2d_image(&self) -> Option<(NodeId, &Image)> {
        let g = self.graph().ok()?;
        let id = self
            .view2d
            .node
            .filter(|id| g.node(*id).is_ok())
            .or_else(|| self.selection.last().copied())
            .or_else(|| g.nodes.iter().find(|n| n.def == "output").map(|n| n.id))?;
        let port = if Some(id) == self.view2d.node { self.view2d.port } else { 0 };
        self.cooked.image(id, port).or_else(|| self.cooked.image(id, 0)).map(|i| (id, i))
    }

    /// Use new 3D camera preferences, pulling the orbit target back within a lowered pan limit.
    pub fn set_camera_prefs(&mut self, prefs: CameraPrefs) {
        self.camera = prefs.sanitized();
        self.view3d.target = self.camera.clamp_target(self.view3d.target);
    }

    /// Show a mesh file in the 3D view (OBJ, glTF, GLB).
    pub fn load_mesh_bytes(&mut self, name: &str, bytes: &[u8]) -> Result<()> {
        let mesh = materialcraft_formats::mesh::load_mesh(name, bytes)?;
        self.custom_mesh = Some((name.to_string(), Arc::new(mesh)));
        self.view3d.scene = "custom".into();
        Ok(())
    }

    /// Nodes of the open graph whose last cook failed, with the error, and how many placeholders for missing SBS nodes failed besides.
    pub fn cook_failures(&self) -> (Vec<(NodeId, &str)>, usize) {
        let mut failed = Vec::new();
        let mut placeholders = 0;
        for node in self.graph().map(|g| g.nodes.as_slice()).unwrap_or_default() {
            let Some(error) = self.cooked.nodes.get(&node.id).and_then(|r| r.error.as_deref()) else { continue };
            if node.def == materialcraft_core::MISSING_NODE {
                placeholders += 1;
            } else {
                failed.push((node.id, error));
            }
        }
        (failed, placeholders)
    }

    /// Log a warning, shown highlighted in the Console.
    pub fn warn(&mut self, msg: impl Into<String>) {
        self.log(format!("{WARNING_PREFIX}{}", msg.into()));
    }

    pub fn log(&mut self, msg: impl Into<String>) {
        self.log.push(msg.into());
        if self.log.len() > 500 {
            self.log.remove(0);
        }
    }
}

#[cfg(test)]
#[path = "session.test.rs"]
mod tests;
