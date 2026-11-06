//! Full-resolution local RGBA image editing. Display zoom and GPU preview size never resize
//! document pixels. History is session-only and bounded by retained unique pixel-buffer bytes.

use std::{collections::HashSet, fs, io::Cursor, path::Path, sync::Arc};

use egui::{Color32, Pos2, Rect, Sense, Stroke, TextureHandle, TextureOptions, Ui, Vec2, pos2, vec2};
use rfd::FileDialog;
use image::GenericImageView;

use crate::overlays::lightbox::fit_inside;
use crate::pages::common::{self, AppRequest, Env};
use crate::pages::levels::Levels;
use crate::pages::curves::Curves;
use crate::pages::image_project::{ImageProject, MAX_PROJECT_LAYERS};
use crate::prompt_box::refs::{ImageSlot, RefKind};
use crate::theme;
use crate::ui::icons::Icon;
use crate::ui::widgets::{self, Kind};

pub const DEFAULT_CANVAS_W: u32 = 1024;
pub const DEFAULT_CANVAS_H: u32 = 768;
pub const MAX_DOCUMENT_BYTES: usize = 512 * 1024 * 1024;
const HISTORY_BUDGET_BYTES: usize = 128 * 1024 * 1024;
const HISTORY_MAX_STEPS: usize = 64;

pub struct EditImagePage {
  pub width: u32,
  pub height: u32,
  pub layers: Vec<CanvasLayer>,
  pub active_layer: usize,
  pub tool: DrawTool,
  pub brush_radius: f32,
  pub brush_hardness: f32,
  pub brush_color: [u8; 4],
  pub sharpen: f32,
  pub edge_contrast: f32,
  pub edge_shift: f32,
  pub filter: FilterPreset,
  pub filter_intensity: f32,
  pub exposure: f32,
  pub contrast: f32,
  pub saturation: f32,
  levels: Levels,
  levels_preview: bool,
  curves: Curves,
  curves_preview: bool,
  edit_mask: bool,
  mask_value: u8,
  rename_buffer: Option<String>,
  initialized: bool,
  texture: Option<TextureHandle>,
  composite_cache: Option<Vec<u8>>,
  texture_region: Option<[u32; 6]>,
  preview_reduced: bool,
  dirty: bool,
  last_drag_uv: Option<Pos2>,
  brush_gesture: BrushGesture,
  fit_view: bool,
  zoom: f32,
  pan: Vec2,
  undo: Vec<DocumentSnapshot>,
  redo: Vec<DocumentSnapshot>,
  pending_edit: Option<DocumentSnapshot>,
  edit_mutated: bool,
  history_budget: usize,
  history_note: Option<String>,
  error: Option<String>,
  pending_open: Option<(u32, u32, Vec<u8>, String)>,
  pending_project: Option<ImageProject>,
}

/// UI-only stroke ownership. An outside sample breaks the anchor, not the undo group.
#[derive(Default)]
struct BrushGesture {
  active: bool,
  anchor: Option<Pos2>,
}

#[derive(Default, Debug, PartialEq)]
struct BrushTransition {
  finish_before: bool,
  begin: bool,
  segment: Option<(Option<Pos2>, Pos2)>,
  finish_after: bool,
}

impl BrushGesture {
  fn event(&mut self, event: &egui::Event, topmost: bool, paint_rect: Rect) -> BrushTransition {
    let mut transition = BrushTransition::default();
    let point = match event {
      egui::Event::PointerMoved(point) => Some(*point),
      egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed, .. } => {
        if *pressed {
          transition.finish_before = self.active;
          self.active = topmost && pos.is_finite() && paint_rect.contains(*pos);
          self.anchor = None;
          transition.begin = self.active;
        } else if self.active {
          transition.finish_after = true;
        }
        Some(*pos)
      },
      egui::Event::PointerGone | egui::Event::WindowFocused(false)
      | egui::Event::PointerButton { button: egui::PointerButton::Middle, pressed: true, .. }
      | egui::Event::Key { key: egui::Key::Space, pressed: true, .. } => {
        transition.finish_after = self.active;
        self.active = false;
        self.anchor = None;
        return transition;
      },
      _ => None,
    };
    if self.active {
      if let Some(point) = point {
        if topmost && point.is_finite() && paint_rect.contains(point) {
          if self.anchor != Some(point) {
            transition.segment = Some((self.anchor, point));
          }
          self.anchor = Some(point);
        } else {
          self.anchor = None; // Never clamp an outside sample onto an image edge.
        }
      }
    }
    if transition.finish_after {
      self.active = false;
      self.anchor = None;
    }
    transition
  }
}

/// A valid topmost press in the batch is ownership evidence even if its release ended outside
/// the widget before egui produced a click/drag response. This canvas has no child widgets.
fn brush_canvas_input_allowed(ui: &Ui, response: &egui::Response, paint_rect: Rect, document_modal: bool, panning: bool) -> bool {
  let ctx = ui.ctx();
  let other_widget_owns_input = ctx.interaction_snapshot(|state| {
    [state.clicked, state.dragged, state.drag_started, state.drag_stopped].into_iter().flatten().any(|id| id != response.id)
  });
  let batch_press = ctx.input(|input| input.events.clone()).iter().any(|event| {
    matches!(event, egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed: true, .. }
      if pos.is_finite() && paint_rect.intersect(ui.clip_rect()).contains(*pos) && ctx.layer_id_at(*pos) == Some(response.layer_id))
  });
  let owns_canvas = response.contains_pointer() || response.is_pointer_button_down_on()
    || response.dragged() || response.drag_stopped() || response.clicked() || batch_press;
  ui.is_enabled() && response.enabled() && owns_canvas
    && ctx.memory(|memory| memory.allows_interaction(response.layer_id))
    && !egui::Popup::is_any_open(ctx) && !other_widget_owns_input && !document_modal && !panning
    && ctx.input(|input| input.focused && !input.key_down(egui::Key::Space) && !input.pointer.button_down(egui::PointerButton::Middle))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DrawTool {
  Move,
  Brush,
  Eraser,
  LiquifyPush,
  LiquifyBloat,
  LiquifyPucker,
  ColorPicker,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BlendMode {
  Normal,
  Multiply,
  Screen,
  Overlay,
  Add,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FilterPreset {
  EdgeGlow,
  Dither,
  Posterize,
  FilmGrain,
  WarmGrade,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CanvasLayer {
  pub name: String,
  pub visible: bool,
  pub opacity: f32,
  pub blend: BlendMode,
  pub width: u32,
  pub height: u32,
  pub pixels: Arc<Vec<u8>>,
  pub mask: Option<LayerMask>,
  pub transform: LayerTransform,
}

#[derive(Clone, Debug, PartialEq)]
pub struct LayerMask {
  pub enabled: bool,
  /// One coverage byte per source-layer pixel, in layer-local coordinates.
  pub pixels: Arc<Vec<u8>>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LayerTransform {
  pub offset: [f64; 2],
  pub rotation_deg: f64,
  pub scale: [f64; 2],
  pub flip_x: bool,
  pub flip_y: bool,
}

impl Default for LayerTransform {
  fn default() -> Self {
    Self { offset: [0.0; 2], rotation_deg: 0.0, scale: [1.0; 2], flip_x: false, flip_y: false }
  }
}

impl LayerTransform {
  pub fn validate(&self) -> Result<(), String> {
    if self.offset.iter().any(|value| !value.is_finite() || value.abs() > 1e12) {
      return Err("Layer position must be finite and within ±1e12 pixels".into());
    }
    if !self.rotation_deg.is_finite() {
      return Err("Layer angle must be finite".into());
    }
    if self.scale.iter().any(|value| !value.is_finite() || !(1e-6..=1e6).contains(value)) {
      return Err("Layer scale must be finite and between 0.000001 and 1000000".into());
    }
    Ok(())
  }

  pub fn local_to_document(&self, point: [f64; 2], width: u32, height: u32) -> Option<[f64; 2]> {
    self.validate().ok()?;
    let center = [width as f64 * 0.5, height as f64 * 0.5];
    let (sin, cos) = self.rotation_deg.rem_euclid(360.0).to_radians().sin_cos();
    let x = (point[0] - center[0]) * self.scale[0] * if self.flip_x { -1.0 } else { 1.0 };
    let y = (point[1] - center[1]) * self.scale[1] * if self.flip_y { -1.0 } else { 1.0 };
    let result = [center[0] + self.offset[0] + cos * x - sin * y, center[1] + self.offset[1] + sin * x + cos * y];
    result.iter().all(|value| value.is_finite()).then_some(result)
  }

  pub fn document_to_local(&self, point: [f64; 2], width: u32, height: u32) -> Option<[f64; 2]> {
    self.validate().ok()?;
    let center = [width as f64 * 0.5, height as f64 * 0.5];
    let (sin, cos) = self.rotation_deg.rem_euclid(360.0).to_radians().sin_cos();
    let x = point[0] - center[0] - self.offset[0];
    let y = point[1] - center[1] - self.offset[1];
    let result = [center[0] + (cos * x + sin * y) / self.scale[0] * if self.flip_x { -1.0 } else { 1.0 }, center[1] + (-sin * x + cos * y) / self.scale[1] * if self.flip_y { -1.0 } else { 1.0 }];
    result.iter().all(|value| value.is_finite()).then_some(result)
  }
}

#[derive(Clone, PartialEq)]
struct DocumentSnapshot {
  width: u32,
  height: u32,
  layers: Vec<CanvasLayer>,
  active_layer: usize,
  exposure: f32,
  contrast: f32,
  saturation: f32,
  levels: Levels,
  levels_preview: bool,
  curves: Curves,
  curves_preview: bool,
  initialized: bool,
}

impl Default for EditImagePage {
  fn default() -> Self {
    Self { width: DEFAULT_CANVAS_W, height: DEFAULT_CANVAS_H, layers: vec![CanvasLayer::new_solid("Layer 1", DEFAULT_CANVAS_W, DEFAULT_CANVAS_H, [0, 0, 0, 0])], active_layer: 0, tool: DrawTool::Brush, brush_radius: 10.0, brush_hardness: 0.75, brush_color: [45, 129, 255, 255], sharpen: 0.55, edge_contrast: 0.40, edge_shift: 0.0, filter: FilterPreset::EdgeGlow, filter_intensity: 0.65, exposure: 0.0, contrast: 0.0, saturation: 0.0, levels: Levels::default(), levels_preview: true, curves: Curves::default(), curves_preview: true, edit_mask: false, mask_value: 0, rename_buffer: None, initialized: false, texture: None, composite_cache: None, texture_region: None, preview_reduced: false, dirty: true, last_drag_uv: None, brush_gesture: BrushGesture::default(), fit_view: true, zoom: 1.0, pan: Vec2::ZERO, undo: Vec::new(), redo: Vec::new(), pending_edit: None, edit_mutated: false, history_budget: HISTORY_BUDGET_BYTES, history_note: None, error: None, pending_open: None, pending_project: None }
  }
}

impl EditImagePage {
  /// A first import initializes an untouched document at source size. Further imports add a
  /// top-left-aligned layer, expanding the canvas if necessary, without resampling either layer.
  pub fn import_rgba_layer(&mut self, width: u32, height: u32, pixels: Vec<u8>, name: String) -> Result<(), String> {
    validate_rgba(width, height, &pixels)?;
    validate_layer_name(&name)?;
    if !self.initialized {
      return self.open_rgba_document(width, height, pixels, name);
    }
    if self.layers.len() >= MAX_PROJECT_LAYERS {
      return Err("The project supports at most 256 layers".into());
    }
    checked_rgba_len(self.width.max(width), self.height.max(height))?;
    if current_pixel_bytes(&self.layers).saturating_add(pixels.capacity()) > MAX_DOCUMENT_BYTES {
      return Err("The layers exceed the 512 MiB editing memory budget. No image was resized.".into());
    }
    self.begin_edit();
    self.width = self.width.max(width);
    self.height = self.height.max(height);
    self.layers.push(CanvasLayer { name, visible: true, opacity: 1.0, blend: BlendMode::Normal, width, height, pixels: Arc::new(pixels), mask: None, transform: LayerTransform::default() });
    self.active_layer = self.layers.len() - 1;
    self.changed();
    self.finish_edit();
    Ok(())
  }

  /// Replaces a document explicitly; the UI confirms replacement when a document has been used.
  pub fn open_rgba_document(&mut self, width: u32, height: u32, pixels: Vec<u8>, name: String) -> Result<(), String> {
    validate_rgba(width, height, &pixels)?;
    validate_layer_name(&name)?;
    if pixels.capacity() > MAX_DOCUMENT_BYTES {
      return Err("The layer exceeds the 512 MiB editing memory budget".into());
    }
    self.begin_edit();
    self.width = width;
    self.height = height;
    self.layers = vec![CanvasLayer { name, visible: true, opacity: 1.0, blend: BlendMode::Normal, width, height, pixels: Arc::new(pixels), mask: None, transform: LayerTransform::default() }];
    self.active_layer = 0;
    self.exposure = 0.0;
    self.contrast = 0.0;
    self.saturation = 0.0;
    self.levels = Levels::default();
    self.levels_preview = true;
    self.curves = Curves::default();
    self.curves_preview = true;
    self.edit_mask = false;
    self.fit_view = true;
    self.pan = Vec2::ZERO;
    self.changed();
    self.finish_edit();
    Ok(())
  }

  fn add_blank_layer(&mut self) -> Result<(), String> {
    if self.layers.len() >= MAX_PROJECT_LAYERS {
      return Err("The project supports at most 256 layers".into());
    }
    let pixels = allocate_rgba(self.width, self.height)?;
    if current_pixel_bytes(&self.layers).saturating_add(pixels.capacity()) > MAX_DOCUMENT_BYTES {
      return Err("The layers exceed the 512 MiB editing memory budget".into());
    }
    self.begin_edit();
    self.layers.push(CanvasLayer { name: format!("Layer {}", self.layers.len() + 1), visible: true, opacity: 1.0, blend: BlendMode::Normal, width: self.width, height: self.height, pixels: Arc::new(pixels), mask: None, transform: LayerTransform::default() });
    self.active_layer = self.layers.len() - 1;
    self.changed();
    self.finish_edit();
    Ok(())
  }

  /// Composites top-left-aligned, original-size layers at the document's actual pixel dimensions.
  pub fn composite_rgba(&self) -> Result<Vec<u8>, String> {
    for layer in &self.layers {
      validate_layer(layer)?;
    }
    if current_pixel_bytes(&self.layers) > MAX_DOCUMENT_BYTES {
      return Err("Layer and mask data exceed the 512 MiB editing memory budget".into());
    }
    self.curves.validate()?;
    let mut out = allocate_rgba(self.width, self.height)?;
    let mut first = true;
    for layer in &self.layers {
      if !layer.visible || layer.opacity <= 0.0 {
        continue;
      }
      composite_layer(&mut out, self.width, self.height, layer, first)?;
      first = false;
    }
    apply_adjustments(&mut out, self.exposure, self.contrast, self.saturation);
    if self.levels_preview {
      self.levels.apply(&mut out);
    }
    if self.curves_preview {
      self.curves.apply(&mut out);
    }
    Ok(out)
  }

  /// Export and generation handoffs share this full-resolution, lossless RGBA PNG path.
  pub fn composite_png(&self) -> Result<Vec<u8>, String> {
    let rgba = self.composite_rgba()?;
    let image = image::RgbaImage::from_raw(self.width, self.height, rgba).ok_or("Invalid canvas pixels")?;
    let mut png = Vec::new();
    image.write_to(&mut Cursor::new(&mut png), image::ImageFormat::Png).map_err(|err| format!("Couldn't encode PNG: {err}"))?;
    Ok(png)
  }

  pub fn to_project(&self) -> ImageProject {
    ImageProject { width: self.width, height: self.height, layers: self.layers.clone(), active_layer: self.active_layer, exposure: self.exposure, contrast: self.contrast, saturation: self.saturation, levels: self.levels, levels_preview: self.levels_preview, curves: self.curves.clone(), curves_preview: self.curves_preview }
  }

  pub fn replace_project(&mut self, project: ImageProject) -> Result<(), String> {
    project.validate()?;
    for layer in &project.layers {
      validate_layer(layer)?;
    }
    if current_pixel_bytes(&project.layers) > MAX_DOCUMENT_BYTES {
      return Err("Project layer and mask data exceed the editing memory budget".into());
    }
    self.begin_edit();
    self.width = project.width;
    self.height = project.height;
    self.layers = project.layers;
    self.active_layer = project.active_layer;
    self.exposure = project.exposure;
    self.contrast = project.contrast;
    self.saturation = project.saturation;
    self.levels = project.levels;
    self.levels_preview = project.levels_preview;
    self.curves = project.curves;
    self.curves_preview = project.curves_preview;
    self.edit_mask = false;
    self.rename_buffer = None;
    self.fit_view = true;
    self.pan = Vec2::ZERO;
    self.changed();
    self.finish_edit();
    Ok(())
  }

  pub fn save_project(&self, path: &Path) -> Result<(), String> {
    self.to_project().save(path)
  }

  pub fn undo(&mut self) -> bool {
    self.finish_edit();
    let Some(previous) = self.undo.pop() else {
      return false;
    };
    let current = self.snapshot();
    self.restore(previous);
    self.redo.push(current);
    self.enforce_history_budget();
    true
  }

  pub fn redo(&mut self) -> bool {
    self.finish_edit();
    let Some(next) = self.redo.pop() else {
      return false;
    };
    let current = self.snapshot();
    self.restore(next);
    self.undo.push(current);
    self.enforce_history_budget();
    true
  }

  fn reset_levels(&mut self) {
    self.levels = Levels::default();
    self.levels_preview = true;
  }

  fn rename_active_layer(&mut self, name: String) -> Result<(), String> {
    validate_layer_name(&name)?;
    if name.trim().is_empty() {
      return Err("Layer name cannot be blank".into());
    }
    if self.active_layer >= self.layers.len() {
      return Err("Select a layer first".into());
    }
    self.begin_edit();
    self.layers[self.active_layer].name = name;
    self.changed();
    self.finish_edit();
    Ok(())
  }

  fn move_active_layer(&mut self, target: usize) -> Result<(), String> {
    if self.active_layer >= self.layers.len() || target >= self.layers.len() {
      return Err("Invalid layer order".into());
    }
    if target == self.active_layer {
      return Ok(());
    }
    self.begin_edit();
    let layer = self.layers.remove(self.active_layer);
    self.layers.insert(target, layer);
    self.active_layer = target;
    self.changed();
    self.finish_edit();
    Ok(())
  }

  fn duplicate_active_layer(&mut self) -> Result<(), String> {
    if self.layers.len() >= MAX_PROJECT_LAYERS {
      return Err("The project supports at most 256 layers".into());
    }
    let mut layer = self.layers.get(self.active_layer).cloned().ok_or("Select a layer first")?;
    layer.name = format!("{} Copy", layer.name);
    validate_layer_name(&layer.name)?;
    self.begin_edit();
    self.layers.push(layer);
    self.active_layer = self.layers.len() - 1;
    self.changed();
    self.finish_edit();
    Ok(())
  }

  fn add_mask_to_active(&mut self, value: u8) -> Result<(), String> {
    let layer = self.layers.get(self.active_layer).ok_or("Select a layer first")?;
    validate_layer(layer)?;
    if layer.mask.is_some() {
      return Err("This layer already has a mask".into());
    }
    let length = checked_rgba_len(layer.width, layer.height)? / 4;
    if current_pixel_bytes(&self.layers).saturating_add(length) > MAX_DOCUMENT_BYTES {
      return Err("The mask exceeds the 512 MiB layer memory budget".into());
    }
    let mut pixels = Vec::new();
    pixels.try_reserve_exact(length).map_err(|_| "Not enough memory for the layer mask")?;
    pixels.resize(length, value);
    self.begin_edit();
    self.layers[self.active_layer].mask = Some(LayerMask { enabled: true, pixels: Arc::new(pixels) });
    self.edit_mask = true;
    self.changed();
    self.finish_edit();
    Ok(())
  }

  fn remove_mask_from_active(&mut self) -> Result<(), String> {
    if !self.layers.get(self.active_layer).is_some_and(|layer| layer.mask.is_some()) {
      return Err("Select a layer mask first".into());
    }
    self.begin_edit();
    self.layers[self.active_layer].mask = None;
    self.edit_mask = false;
    self.changed();
    self.finish_edit();
    Ok(())
  }

  fn invert_active_mask(&mut self) -> Result<(), String> {
    self.begin_edit();
    let result = self.writable_mask_active().map(|pixels| {
      for pixel in pixels {
        *pixel = 255 - *pixel;
      }
    });
    if result.is_ok() {
      self.changed();
    }
    self.finish_edit();
    result
  }

  /// A programmatic stroke is one undo step; the UI groups all drag stamps into one step.
  pub fn apply_tool_at(&mut self, uv: Pos2, prev_uv: Option<Pos2>) -> Result<(), String> {
    if self.tool == DrawTool::ColorPicker {
      return self.apply_tool_stamp(uv, prev_uv);
    }
    self.begin_edit();
    let result = self.apply_tool_stamp(uv, prev_uv);
    self.finish_edit();
    result
  }

  pub fn apply_filter_to_active(&mut self) -> Result<(), String> {
    if self.edit_mask {
      return Err("Select Pixels to apply image filters".into());
    }
    self.begin_edit();
    let (filter, intensity) = (self.filter, self.filter_intensity);
    let result = self.writable_active().and_then(|layer| apply_filter(Arc::get_mut(&mut layer.pixels).ok_or("Layer pixels are still shared")?, layer.width, layer.height, filter, intensity));
    if result.is_ok() {
      self.changed();
    }
    self.finish_edit();
    result
  }

  pub fn refine_edges_of_active(&mut self) -> Result<(), String> {
    if self.edit_mask {
      return Err("Select Pixels to refine image edges".into());
    }
    self.begin_edit();
    let (sharpen, contrast, shift) = (self.sharpen, self.edge_contrast, self.edge_shift);
    let result = self.writable_active().and_then(|layer| refine_edges_rgba(Arc::get_mut(&mut layer.pixels).ok_or("Layer pixels are still shared")?, layer.width, layer.height, sharpen, contrast, shift));
    if result.is_ok() {
      self.changed();
    }
    self.finish_edit();
    result
  }

  pub fn ui(&mut self, ui: &mut Ui, env: &mut Env<'_>) {
    if self.rename_buffer.is_none() && self.pending_open.is_none() && self.pending_project.is_none() && !ui.ctx().egui_wants_keyboard_input() {
      let undo = egui::KeyboardShortcut::new(egui::Modifiers::COMMAND, egui::Key::Z);
      let redo = egui::KeyboardShortcut::new(egui::Modifiers::COMMAND | egui::Modifiers::SHIFT, egui::Key::Z);
      if ui.input_mut(|input| input.consume_shortcut(&redo)) {
        self.redo();
      } else if ui.input_mut(|input| input.consume_shortcut(&undo)) {
        self.undo();
      }
    }
    if let Err(error) = self.refresh_texture(&env.ctx) {
      self.error = Some(error);
    }
    let area = ui.max_rect().shrink2(vec2(16.0, 12.0));
    let mut root = ui.new_child(egui::UiBuilder::new().max_rect(area));
    root.columns(3, |cols| {
      cols[0].set_max_width(270.0);
      egui::ScrollArea::vertical().id_salt("edit_img_left").show(&mut cols[0], |ui| self.tools_column(ui, env));
      self.canvas_column(&mut cols[1], env);
      egui::ScrollArea::vertical().id_salt("edit_img_right").show(&mut cols[2], |ui| self.layers_column(ui, env));
    });
    if self.pending_edit.is_some() && !ui.input(|input| input.pointer.any_down()) {
      self.finish_edit();
    }
    if self.pending_open.is_some() {
      let mut decision = None;
      egui::Modal::new(egui::Id::new("replace_image_document")).show(ui.ctx(), |ui| {
        ui.heading("Open another image?");
        ui.label("Replace the current document with the selected image?");
        ui.label("Save a PNG first if you need a copy. Undo depends on available history memory.");
        ui.horizontal(|ui| {
          if ui.button("Open Image").clicked() {
            decision = Some(true);
          }
          if ui.button("Cancel").clicked() {
            decision = Some(false);
          }
        });
      });
      if let Some(open) = decision {
        let image = self.pending_open.take();
        if open {
          if let Some((w, h, pixels, name)) = image {
            if let Err(error) = self.open_rgba_document(w, h, pixels, name) {
              env.toasts.error(error);
            }
          }
        }
      }
    }
    if self.pending_project.is_some() {
      let mut decision = None;
      egui::Modal::new(egui::Id::new("replace_layer_project")).show(ui.ctx(), |ui| {
        ui.heading("Open another project?");
        ui.label("Replace the current document with this project? Save your project first to keep its layers.");
        ui.horizontal(|ui| {
          if ui.button("Open Project").clicked() {
            decision = Some(true);
          }
          if ui.button("Cancel").clicked() {
            decision = Some(false);
          }
        });
      });
      if let Some(open) = decision {
        if let Some(project) = self.pending_project.take() {
          if open {
            if let Err(error) = self.replace_project(project) {
              env.toasts.error(error);
            }
          }
        }
      }
    }
    if self.rename_buffer.is_some() {
      let mut action = None;
      egui::Modal::new(egui::Id::new("rename_image_layer")).show(ui.ctx(), |ui| {
        ui.heading("Rename Layer");
        if let Some(name) = &mut self.rename_buffer {
          ui.text_edit_singleline(name);
        }
        ui.horizontal(|ui| {
          if ui.button("Apply").clicked() {
            action = Some(true);
          }
          if ui.button("Cancel").clicked() {
            action = Some(false);
          }
        });
      });
      match action {
        Some(true) => {
          if let Some(name) = self.rename_buffer.clone() {
            match self.rename_active_layer(name) {
              Ok(()) => self.rename_buffer = None,
              Err(error) => env.toasts.error(error),
            }
          }
        },
        Some(false) => self.rename_buffer = None,
        None => {},
      }
    }
  }

  fn tools_column(&mut self, ui: &mut Ui, env: &mut Env<'_>) {
    widgets::section_label(ui, "01 / Tools", theme::ACCENT_INK);
    ui.horizontal(|ui| {
      if ui.add_enabled(!self.undo.is_empty(), egui::Button::new("Undo")).clicked() {
        self.undo();
      }
      if ui.add_enabled(!self.redo.is_empty(), egui::Button::new("Redo")).clicked() {
        self.redo();
      }
    });
    if let Some(note) = &self.history_note {
      ui.small(note);
    }
    if let Some(error) = &self.error {
      ui.colored_label(theme::MUTED, error);
    }
    for tool in DrawTool::ALL {
      let selected = self.tool == tool;
      if widgets::menu_row(ui, widgets::RowLead::Icon(Icon::Pencil), tool.label(), None, selected, false, selected).clicked() {
        self.tool = tool;
      }
    }
    ui.add_space(6.0);
    ui.add(egui::Slider::new(&mut self.brush_radius, 1.0..=500.0).text("Brush Radius (px)"));
    ui.add(egui::Slider::new(&mut self.brush_hardness, 0.1..=1.0).text("Hardness"));
    ui.color_edit_button_srgba_unmultiplied(&mut self.brush_color);
    if self.edit_mask {
      ui.label("Mask brush");
      ui.horizontal(|ui| {
        if ui.button("Hide · Black").clicked() {
          self.mask_value = 0;
        }
        if ui.button("Reveal · White").clicked() {
          self.mask_value = 255;
        }
      });
      ui.add(egui::Slider::new(&mut self.mask_value, 0..=255).text("Mask Value"));
      ui.small("Brush edits coverage; Eraser reveals. Image filters require Pixels.");
    }
    ui.add_space(10.0);
    widgets::section_label(ui, "02 / Sharpen & Edges", theme::ACCENT_INK);
    ui.add(egui::Slider::new(&mut self.sharpen, 0.0..=1.0).text("Sharpen"));
    ui.add(egui::Slider::new(&mut self.edge_contrast, 0.0..=1.0).text("Edge Contrast"));
    ui.add(egui::Slider::new(&mut self.edge_shift, -1.0..=1.0).text("Edge Shift"));
    if widgets::button(ui, Some(Icon::Sparkles), "Apply to Layer", Kind::Secondary, ui.available_width(), 32.0).clicked() {
      if let Err(error) = self.refine_edges_of_active() {
        env.toasts.error(error);
      }
    }
    ui.add_space(10.0);
    widgets::section_label(ui, "03 / Filters", theme::ACCENT_INK);
    egui::ComboBox::from_id_salt("edit_img_filter").selected_text(self.filter.label()).show_ui(ui, |ui| {
      for preset in FilterPreset::ALL {
        ui.selectable_value(&mut self.filter, preset, preset.label());
      }
    });
    ui.add(egui::Slider::new(&mut self.filter_intensity, 0.1..=1.0).text("Intensity"));
    if widgets::button(ui, Some(Icon::Sparkles), "Apply Filter", Kind::Secondary, ui.available_width(), 32.0).clicked() {
      if let Err(error) = self.apply_filter_to_active() {
        env.toasts.error(error);
      }
    }
  }

  fn canvas_column(&mut self, ui: &mut Ui, env: &mut Env<'_>) {
    widgets::hud(ui, &format!("CANVAS · {} × {} PX", self.width, self.height), theme::MUTED);
    ui.horizontal(|ui| {
      if ui.selectable_label(self.fit_view, "Fit").clicked() {
        self.fit_view = true;
        self.pan = Vec2::ZERO;
      }
      if ui.selectable_label(!self.fit_view && self.zoom == 1.0, "100%").clicked() {
        self.fit_view = false;
        self.zoom = 1.0;
        self.pan = Vec2::ZERO;
      }
      if ui.add(egui::Slider::new(&mut self.zoom, 0.001..=8.0).clamping(egui::SliderClamping::Edits).logarithmic(true).text("Zoom")).changed() {
        self.fit_view = false;
      }
    });
    ui.small("Scroll to pan · Space-drag or middle-drag to move the view");
    let frame_size = vec2(ui.available_width(), (ui.available_height() - 110.0).max(180.0));
    let (frame_rect, response) = ui.allocate_exact_size(frame_size, Sense::click_and_drag());
    ui.painter().rect(frame_rect, 0.0, theme::SUNKEN, Stroke::new(1.0, theme::LINE_STRONG), egui::StrokeKind::Inside);
    let viewport = frame_rect.shrink(6.0);
    let ppp = ui.ctx().pixels_per_point();
    if self.fit_view {
      self.zoom = (viewport.width() * ppp / self.width as f32).min(viewport.height() * ppp / self.height as f32);
    }
    let panning = response.dragged_by(egui::PointerButton::Middle) || (response.dragged_by(egui::PointerButton::Primary) && ui.input(|input| input.key_down(egui::Key::Space)));
    if panning {
      self.pan += ui.input(|input| input.pointer.delta());
    }
    if response.hovered() {
      self.pan += ui.input(|input| input.smooth_scroll_delta);
    }
    let canvas = view_rect(viewport, self.width, self.height, self.zoom, ppp, self.pan);
    let painter = ui.painter().with_clip_rect(viewport);
    painter.rect_filled(canvas, 0.0, Color32::from_gray(72));
    match self.refresh_view_texture(ui.ctx(), canvas, viewport) {
      Ok(region) => {
        if let Some(texture) = &self.texture {
          painter.image(texture.id(), region, Rect::from_min_max(Pos2::ZERO, pos2(1.0, 1.0)), Color32::WHITE);
        }
      },
      Err(error) => self.error = Some(error),
    }
    if matches!(self.tool, DrawTool::Brush | DrawTool::Eraser) {
      self.last_drag_uv = None;
      let document_modal = self.rename_buffer.is_some() || self.pending_open.is_some() || self.pending_project.is_some();
      let allowed = brush_canvas_input_allowed(ui, &response, canvas.intersect(viewport), document_modal, panning);
      if !allowed {
        self.finish_brush_gesture();
      } else {
        let events = ui.input(|input| input.events.clone());
        for event in &events {
          let topmost = match event {
            egui::Event::PointerMoved(point) | egui::Event::PointerButton { pos: point, .. } => {
              point.is_finite() && ui.ctx().layer_id_at(*point) == Some(response.layer_id)
            },
            _ => true,
          };
          let transition = self.brush_gesture.event(event, topmost, canvas.intersect(viewport).intersect(ui.clip_rect()));
          self.apply_brush_transition(transition, canvas);
        }
        // A release can be missing when the OS loses capture. Never resume that gesture later.
        if self.brush_gesture.active && !ui.input(|input| input.pointer.primary_down()) {
          self.finish_brush_gesture();
        }
      }
    } else {
      self.finish_brush_gesture();
      // Move, Liquify and the eyedropper keep their separate drag semantics.
      let pointer = response.interact_pointer_pos().filter(|point| viewport.contains(*point) && (self.last_drag_uv.is_some() || canvas.contains(*point)));
      if !panning && (response.dragged_by(egui::PointerButton::Primary) || response.clicked_by(egui::PointerButton::Primary)) {
        if let Some(point) = pointer {
          let uv = pos2(((point.x - canvas.left()) / canvas.width()).clamp(0.0, 1.0), ((point.y - canvas.top()) / canvas.height()).clamp(0.0, 1.0));
          if self.last_drag_uv.is_none() && self.tool != DrawTool::ColorPicker {
            self.begin_edit();
          }
          if let Err(error) = self.apply_tool_stamp(uv, self.last_drag_uv) {
            self.error = Some(error);
          }
          self.last_drag_uv = Some(uv);
        }
      }
      if !ui.input(|input| input.pointer.primary_down()) || panning {
        self.last_drag_uv = None;
        if panning {
          self.finish_edit();
        }
      }
    }
    if self.dirty { ui.ctx().request_repaint(); }
    ui.add_space(8.0);
    if self.preview_reduced && self.zoom >= 1.0 {
      ui.small("GPU-limited display preview; PNG export keeps every original pixel.");
    }
    ui.horizontal_wrapped(|ui| {
      if ui.button("Open Project…").clicked() {
        self.open_project_file(env);
      }
      if ui.button("Save Project…").clicked() {
        self.save_project_file(env);
      }
      if ui.button("Open Image…").clicked() {
        self.load_image_file(env, true);
      }
      if ui.button("Add Image Layer…").clicked() {
        self.load_image_file(env, false);
      }
      if ui.button("Send to Create Image").clicked() {
        self.send_composite(env, None);
      }
      if ui.button("Send to Create Video").clicked() {
        self.send_composite(env, Some(ImageSlot::FirstFrame));
      }
    });
  }

  fn layers_column(&mut self, ui: &mut Ui, env: &mut Env<'_>) {
    widgets::section_label(ui, "04 / Layers", theme::ACCENT_INK);
    ui.horizontal(|ui| {
      if ui.button("+ Layer").clicked() {
        if let Err(error) = self.add_blank_layer() {
          env.toasts.error(error);
        }
      }
      if ui.button("Duplicate").clicked() {
        if let Err(error) = self.duplicate_active_layer() {
          env.toasts.error(error);
        }
      }
      if ui.add_enabled(self.layers.len() > 1, egui::Button::new("Delete")).clicked() {
        self.begin_edit();
        self.layers.remove(self.active_layer);
        self.active_layer = self.active_layer.min(self.layers.len() - 1);
        self.changed();
        self.finish_edit();
      }
    });
    ui.horizontal(|ui| {
      if ui.button("Rename").clicked() {
        self.rename_buffer = self.layers.get(self.active_layer).map(|layer| layer.name.clone());
      }
      if ui.add_enabled(self.active_layer.saturating_add(1) < self.layers.len(), egui::Button::new("Move Up")).clicked() {
        if let Err(error) = self.move_active_layer(self.active_layer.saturating_add(1)) {
          env.toasts.error(error);
        }
      }
      if ui.add_enabled(self.active_layer > 0, egui::Button::new("Move Down")).clicked() {
        if let Err(error) = self.move_active_layer(self.active_layer - 1) {
          env.toasts.error(error);
        }
      }
    });
    let before = self.snapshot();
    let mut dirty = false;
    let mut mask_action = None;
    for (index, layer) in self.layers.iter_mut().enumerate().rev() {
      egui::Frame::new().fill(theme::CONTROLS).inner_margin(egui::Margin::same(6)).show(ui, |ui| {
        ui.horizontal(|ui| {
          dirty |= ui.checkbox(&mut layer.visible, "").changed();
          if ui.selectable_label(index == self.active_layer, &layer.name).clicked() {
            self.active_layer = index;
            self.edit_mask &= layer.mask.is_some();
          }
        });
        ui.small(format!("{} × {} px · original", layer.width, layer.height));
        dirty |= ui.add(egui::Slider::new(&mut layer.opacity, 0.0..=1.0).text("Opacity")).changed();
        egui::ComboBox::from_id_salt(("edit_img_blend", index)).selected_text(layer.blend.label()).show_ui(ui, |ui| {
          for mode in BlendMode::ALL {
            dirty |= ui.selectable_value(&mut layer.blend, mode, mode.label()).changed();
          }
        });
      });
    }
    if let Some(layer) = self.layers.get_mut(self.active_layer) {
      ui.add_space(8.0);
      ui.label("Edit Target");
      ui.horizontal(|ui| {
        ui.selectable_value(&mut self.edit_mask, false, "Pixels");
        ui.add_enabled_ui(layer.mask.is_some(), |ui| {
          ui.selectable_value(&mut self.edit_mask, true, "Mask");
        });
      });
      if let Some(mask) = &mut layer.mask {
        dirty |= ui.checkbox(&mut mask.enabled, "Enable Layer Mask").changed();
        ui.horizontal(|ui| {
          if ui.button("Invert Mask").clicked() {
            mask_action = Some(1);
          }
          if ui.button("Remove Mask").clicked() {
            mask_action = Some(2);
          }
        });
      } else if ui.button("Add Reveal-All Mask").clicked() {
        mask_action = Some(0);
      }
      ui.small("Mask follows this layer's transform and preserves source pixels.");
      ui.add_space(8.0);
      ui.label("Layer Transform");
      ui.horizontal(|ui| {
        ui.label("X");
        dirty |= ui.add(egui::DragValue::new(&mut layer.transform.offset[0]).speed(1.0).range(-1e9..=1e9).clamp_existing_to_range(false)).changed();
        ui.label("Y");
        dirty |= ui.add(egui::DragValue::new(&mut layer.transform.offset[1]).speed(1.0).range(-1e9..=1e9).clamp_existing_to_range(false)).changed();
      });
      ui.horizontal(|ui| {
        ui.label("Angle");
        dirty |= ui.add(egui::DragValue::new(&mut layer.transform.rotation_deg).speed(0.5).range(-180.0..=180.0).clamp_existing_to_range(false).suffix("°")).changed();
      });
      ui.horizontal(|ui| {
        ui.label("Scale X");
        dirty |= ui.add(egui::DragValue::new(&mut layer.transform.scale[0]).speed(0.01).range(0.01..=100.0).clamp_existing_to_range(false)).changed();
        ui.label("Y");
        dirty |= ui.add(egui::DragValue::new(&mut layer.transform.scale[1]).speed(0.01).range(0.01..=100.0).clamp_existing_to_range(false)).changed();
      });
      ui.horizontal(|ui| {
        dirty |= ui.checkbox(&mut layer.transform.flip_x, "Flip H").changed();
        dirty |= ui.checkbox(&mut layer.transform.flip_y, "Flip V").changed();
      });
      if ui.button("Reset Transform").clicked() {
        layer.transform = LayerTransform::default();
        dirty = true;
      }
      if let Err(error) = layer.transform.validate() {
        if let Some(original) = before.layers.get(self.active_layer) {
          layer.transform = original.transform;
        }
        env.toasts.error(error);
      }
      ui.small("Transforms keep original pixels. Content outside the canvas is hidden.");
    }
    ui.add_space(10.0);
    widgets::section_label(ui, "05 / Adjustments", theme::ACCENT_INK);
    dirty |= ui.add(egui::Slider::new(&mut self.exposure, -1.0..=1.0).text("Exposure")).changed();
    dirty |= ui.add(egui::Slider::new(&mut self.contrast, -1.0..=1.0).text("Contrast")).changed();
    dirty |= ui.add(egui::Slider::new(&mut self.saturation, -1.0..=1.0).text("Saturation")).changed();
    ui.label("Levels · RGB");
    dirty |= ui.checkbox(&mut self.levels_preview, "Preview Levels").changed();
    let max_black = self.levels.input_white.saturating_sub(1).min(254);
    dirty |= ui.add(egui::Slider::new(&mut self.levels.input_black, 0..=max_black).text("Input Black")).changed();
    let min_white = self.levels.input_black.saturating_add(1);
    dirty |= ui.add(egui::Slider::new(&mut self.levels.input_white, min_white..=255).text("Input White")).changed();
    dirty |= ui.add(egui::Slider::new(&mut self.levels.gamma, 0.1..=9.99).logarithmic(true).text("Midtone Gamma")).changed();
    dirty |= ui.add(egui::Slider::new(&mut self.levels.output_black, 0..=255).text("Output Black")).changed();
    dirty |= ui.add(egui::Slider::new(&mut self.levels.output_white, 0..=255).text("Output White")).changed();
    if ui.button("Reset Levels").clicked() {
      self.reset_levels();
      dirty = true;
    }
    ui.add_space(8.0);
    ui.label("Curves · Master / RGB");
    dirty |= ui.checkbox(&mut self.curves_preview, "Preview Curves").changed();
    dirty |= self.curves.ui(ui);
    if dirty {
      if self.pending_edit.is_none() {
        self.pending_edit = Some(before);
      }
      self.changed();
      self.enforce_history_budget();
      if !ui.input(|input| input.pointer.any_down()) {
        self.finish_edit();
      }
    }
    if let Some(action) = mask_action {
      self.finish_edit();
      let result = match action {
        0 => self.add_mask_to_active(255),
        1 => self.invert_active_mask(),
        _ => self.remove_mask_from_active(),
      };
      if let Err(error) = result {
        env.toasts.error(error);
      }
    }
    ui.small("Layer pixels stay unchanged by adjustments. PNG uses the visible composite.");
    ui.add_space(10.0);
    widgets::section_label(ui, "06 / Export", theme::ACCENT_INK);
    if ui.button("Send Source Pixels to BG Remove").clicked() {
      if let Some(layer) = self.layers.get(self.active_layer) {
        match clone_pixels(&layer.pixels) {
          Ok(pixels) => env.requests.push(AppRequest::SendRgbaToRemoveBg { width: layer.width, height: layer.height, pixels }),
          Err(error) => env.toasts.error(error),
        }
      }
    }
    if widgets::button(ui, Some(Icon::Download), "Save Composite PNG", Kind::Primary, ui.available_width(), 36.0).clicked() {
      self.save_composite(env);
    }
  }

  fn load_image_file(&mut self, env: &mut Env<'_>, replace: bool) {
    let Some(path) = common::pick_files(&[RefKind::Image], false).into_iter().next() else {
      return;
    };
    self.pending_project = None;
    let name = common::file_name(&path);
    let decoded = decode_original_rgba(&path);
    match decoded {
      Ok(image) => {
        let (width, height) = image.dimensions();
        let pixels = image.into_raw();
        if replace && self.initialized {
          self.pending_open = Some((width, height, pixels, name));
        } else {
          let result = if replace { self.open_rgba_document(width, height, pixels, name) } else { self.import_rgba_layer(width, height, pixels, name) };
          if let Err(error) = result {
            env.toasts.error(error);
          }
        }
      },
      Err(error) => env.toasts.error(format!("Couldn't open {name}: {error}")),
    }
  }

  fn open_project_file(&mut self, env: &mut Env<'_>) {
    let Some(path) = FileDialog::new().add_filter("ArtCraft layered project", &["acraft"]).pick_file() else {
      return;
    };
    match ImageProject::load(&path) {
      Ok(project) => {
        self.pending_open = None;
        if self.initialized {
          self.pending_project = Some(project);
        } else if let Err(error) = self.replace_project(project) {
          env.toasts.error(error);
        }
      },
      Err(error) => env.toasts.error(format!("Couldn't open project: {error}")),
    }
  }

  fn save_project_file(&self, env: &mut Env<'_>) {
    let Some(path) = FileDialog::new().set_file_name("artcraft.acraft").add_filter("ArtCraft layered project", &["acraft"]).save_file() else {
      return;
    };
    match self.save_project(&path) {
      Ok(()) => env.toasts.success(format!("Saved layered project · {}", common::file_name(&path))),
      Err(error) => env.toasts.error(format!("Couldn't save project: {error}")),
    }
  }

  fn send_composite(&self, env: &mut Env<'_>, video_slot: Option<ImageSlot>) {
    match self.composite_png() {
      Ok(png) => env.requests.push(match video_slot {
        Some(slot) => AppRequest::SendPngToCreateVideo { png, slot, prompt: None },
        None => AppRequest::SendPngToCreateImage { png, prompt: None },
      }),
      Err(error) => env.toasts.error(error),
    }
  }

  fn save_composite(&self, env: &mut Env<'_>) {
    let png = match self.composite_png() {
      Ok(png) => png,
      Err(error) => {
        env.toasts.error(error);
        return;
      },
    };
    let Some(path) = FileDialog::new().set_file_name("artcraft_composite.png").add_filter("PNG image", &["png"]).save_file() else {
      return;
    };
    match fs::write(&path, png) {
      Ok(()) => env.toasts.success(format!("Saved {} × {} PNG · {}", self.width, self.height, common::file_name(&path))),
      Err(error) => env.toasts.error(format!("Failed to save PNG: {error}")),
    }
  }

  fn refresh_texture(&mut self, ctx: &egui::Context) -> Result<(), String> {
    let _ = ctx;
    if !self.dirty && self.composite_cache.is_some() {
      return Ok(());
    }
    self.composite_cache = Some(self.composite_rgba()?);
    self.texture_region = None;
    self.dirty = false;
    Ok(())
  }

  fn refresh_view_texture(&mut self, ctx: &egui::Context, canvas: Rect, viewport: Rect) -> Result<Rect, String> {
    let maximum = ctx.input(|input| input.max_texture_side).max(1) as u32;
    let region = visible_preview_region(canvas, viewport, self.width, self.height, self.zoom, maximum);
    let [x, y, width, height, preview_w, preview_h] = region;
    self.preview_reduced = preview_w < width || preview_h < height;
    if self.texture_region != Some(region) {
      let source = self.composite_cache.as_ref().ok_or("Canvas preview is not ready")?;
      let mut preview = allocate_rgba(preview_w, preview_h)?;
      for row in 0..preview_h as usize {
        let sy = y as usize + row * height as usize / preview_h as usize;
        for column in 0..preview_w as usize {
          let sx = x as usize + column * width as usize / preview_w as usize;
          let from = (sy * self.width as usize + sx) * 4;
          let to = (row * preview_w as usize + column) * 4;
          preview[to..to + 4].copy_from_slice(&source[from..from + 4]);
        }
      }
      let image = egui::ColorImage::from_rgba_unmultiplied([preview_w as usize, preview_h as usize], &preview);
      match &mut self.texture {
        Some(texture) => texture.set(image, TextureOptions::NEAREST),
        None => self.texture = Some(ctx.load_texture("edit_image_visible_region", image, TextureOptions::NEAREST)),
      }
      self.texture_region = Some(region);
    }
    Ok(Rect::from_min_max(canvas.min + canvas.size() * vec2(x as f32 / self.width as f32, y as f32 / self.height as f32), canvas.min + canvas.size() * vec2((x + width) as f32 / self.width as f32, (y + height) as f32 / self.height as f32)))
  }

  fn finish_brush_gesture(&mut self) {
    if self.brush_gesture.active {
      self.brush_gesture = BrushGesture::default();
      self.finish_edit();
    }
  }

  fn apply_brush_transition(&mut self, transition: BrushTransition, canvas: Rect) {
    if transition.finish_before {
      self.finish_edit();
    }
    if transition.begin {
      self.begin_edit();
    }
    if let Some((previous, point)) = transition.segment {
      let to_uv = |point: Pos2| pos2((point.x - canvas.left()) / canvas.width(), (point.y - canvas.top()) / canvas.height());
      if let Err(error) = self.apply_tool_stamp(to_uv(point), previous.map(to_uv)) {
        self.error = Some(error);
        self.finish_brush_gesture();
      }
    }
    if transition.finish_after {
      self.finish_edit();
    }
  }

  fn apply_tool_stamp(&mut self, uv: Pos2, previous: Option<Pos2>) -> Result<(), String> {
    if matches!(self.tool, DrawTool::Brush | DrawTool::Eraser)
      && std::iter::once(uv).chain(previous).any(|point| !point.is_finite() || !(0.0..=1.0).contains(&point.x) || !(0.0..=1.0).contains(&point.y)) {
      return Err("Brush coordinates must be inside the document".into());
    }
    let (px, py) = document_pixel_from_uv(self.width, self.height, uv).ok_or("Invalid brush coordinates")?;
    if self.tool == DrawTool::ColorPicker {
      if self.edit_mask {
        let layer = self.layers.get(self.active_layer).ok_or("Select a layer first")?;
        validate_layer(layer)?;
        let local = LayerMapping::new(&layer.transform, layer.width, layer.height)?.to_local([px as f64 + 0.5, py as f64 + 0.5]);
        if local[0] < 0.0 || local[1] < 0.0 || local[0] >= layer.width as f64 || local[1] >= layer.height as f64 {
          return Err("The pointer is outside this layer mask".into());
        }
        let mask = layer.mask.as_ref().ok_or("Add a layer mask first")?;
        self.mask_value = mask.pixels[local[1].floor() as usize * layer.width as usize + local[0].floor() as usize];
        return Ok(());
      }
      let pixels = self.composite_rgba()?;
      let index = (py as usize * self.width as usize + px as usize) * 4;
      self.brush_color.copy_from_slice(&pixels[index..index + 4]);
      return Ok(());
    }
    let document_point = [px as f64 + 0.5, py as f64 + 0.5];
    let layer = self.layers.get(self.active_layer).ok_or("Select a layer first")?;
    validate_layer(layer)?;
    if self.tool == DrawTool::Move {
      let Some(previous) = previous else {
        return Ok(());
      };
      let delta = [(uv.x - previous.x) as f64 * self.width as f64, (uv.y - previous.y) as f64 * self.height as f64];
      let mut transform = layer.transform;
      for axis in 0..2 {
        transform.offset[axis] += delta[axis];
      }
      transform.validate()?;
      if transform != layer.transform {
        self.layers[self.active_layer].transform = transform;
        self.changed();
      }
      return Ok(());
    }
    let (layer_width, layer_height) = (layer.width, layer.height);
    let mapping = LayerMapping::new(&layer.transform, layer.width, layer.height)?;
    let local = mapping.to_local(document_point);
    let (tool, radius, hardness, color, edit_mask, mask_value) = (self.tool, self.brush_radius.clamp(1.0, 500.0) as f64, self.brush_hardness, self.brush_color, self.edit_mask, self.mask_value);
    if !radius.is_finite() || !hardness.is_finite() {
      return Err("Brush radius and hardness must be finite".into());
    }
    if edit_mask && !matches!(tool, DrawTool::Brush | DrawTool::Eraser) {
      return Err("Use Brush or Eraser to edit a mask; select Pixels for Liquify".into());
    }
    let continuous = matches!(tool, DrawTool::Brush | DrawTool::Eraser);
    let segment_start = if continuous {
      match previous {
        Some(previous) => {
          let (x, y) = document_pixel_from_uv(self.width, self.height, previous).ok_or("Invalid previous brush coordinates")?;
          let point = [x as f64 + 0.5, y as f64 + 0.5];
          if point == document_point {
            return Ok(()); // Non-airbrush: holding still does not add opacity every UI frame.
          }
          point
        },
        None => document_point,
      }
    } else { document_point };
    let (min_x, max_x) = (segment_start[0].min(document_point[0]) - radius, segment_start[0].max(document_point[0]) + radius);
    let (min_y, max_y) = (segment_start[1].min(document_point[1]) - radius, segment_start[1].max(document_point[1]) + radius);
    let corners = [[min_x, min_y], [max_x, min_y], [min_x, max_y], [max_x, max_y]].map(|point| mapping.to_local(point));
    let left = corners.iter().map(|point| point[0]).fold(f64::INFINITY, f64::min).floor().clamp(0.0, layer_width as f64) as u32;
    let right = corners.iter().map(|point| point[0]).fold(f64::NEG_INFINITY, f64::max).ceil().clamp(0.0, layer_width as f64) as u32;
    let top = corners.iter().map(|point| point[1]).fold(f64::INFINITY, f64::min).floor().clamp(0.0, layer_height as f64) as u32;
    let bottom = corners.iter().map(|point| point[1]).fold(f64::NEG_INFINITY, f64::max).ceil().clamp(0.0, layer_height as f64) as u32;
    if left >= right || top >= bottom {
      return Ok(());
    }
    if edit_mask {
      let pixels = self.writable_mask_active()?;
      for y in top..bottom {
        let Some((row_left, row_right)) = capsule_row_span(mapping, y, layer_width, segment_start, document_point, radius) else { continue };
        for x in left.max(row_left)..right.min(row_right) {
          let point = mapping.to_document([x as f64 + 0.5, y as f64 + 0.5]);
          let distance = segment_distance(point, segment_start, document_point);
          if distance >= radius {
            continue;
          }
          let coverage = ((1.0 - distance / radius).powf((1.0 - hardness as f64).max(0.15)) * color[3] as f64 / 255.0).clamp(0.0, 1.0);
          let index = y as usize * layer_width as usize + x as usize;
          let target = if tool == DrawTool::Eraser { 255 } else { mask_value };
          pixels[index] = (pixels[index] as f64 * (1.0 - coverage) + target as f64 * coverage).round().clamp(0.0, 255.0) as u8;
        }
      }
    } else {
      let previous_local = previous.map(|previous| mapping.to_local([previous.x as f64 * self.width as f64 + 0.5, previous.y as f64 * self.height as f64 + 0.5]));
      let source_layer = self.writable_active()?;
      let local_radius = (radius / source_layer.transform.scale[0].abs().min(source_layer.transform.scale[1].abs())).clamp(1.0, 10000.0) as f32;
      let pixels = Arc::get_mut(&mut source_layer.pixels).ok_or("Layer pixels are still shared")?;
      match tool {
        DrawTool::Brush | DrawTool::Eraser => {
          for y in top..bottom {
            let Some((row_left, row_right)) = capsule_row_span(mapping, y, layer_width, segment_start, document_point, radius) else { continue };
            for x in left.max(row_left)..right.min(row_right) {
              let point = mapping.to_document([x as f64 + 0.5, y as f64 + 0.5]);
              let distance = segment_distance(point, segment_start, document_point);
              if distance >= radius {
                continue;
              }
              let falloff = (1.0 - distance / radius).powf((1.0 - hardness as f64).max(0.15));
              let index = (y as usize * layer_width as usize + x as usize) * 4;
              if tool == DrawTool::Eraser {
                pixels[index + 3] = (pixels[index + 3] as f64 * (1.0 - falloff)).round() as u8;
              } else {
                let source_alpha = color[3] as f64 / 255.0 * falloff;
                let destination_alpha = pixels[index + 3] as f64 / 255.0;
                let alpha = source_alpha + destination_alpha * (1.0 - source_alpha);
                if alpha > 0.0 {
                  for channel in 0..3 {
                    pixels[index + channel] = ((color[channel] as f64 * source_alpha + pixels[index + channel] as f64 * destination_alpha * (1.0 - source_alpha)) / alpha).round().clamp(0.0, 255.0) as u8;
                  }
                  pixels[index + 3] = (alpha * 255.0).round() as u8;
                }
              }
            }
          }
        },
        DrawTool::LiquifyPush | DrawTool::LiquifyBloat | DrawTool::LiquifyPucker => {
          let delta = previous_local.map_or(Vec2::ZERO, |point| vec2((local[0] - point[0]) as f32, (local[1] - point[1]) as f32));
          apply_liquify_brush(pixels, layer_width, layer_height, local[0] as f32 - 0.5, local[1] as f32 - 0.5, local_radius * 1.4, tool, delta)?;
        },
        DrawTool::Move | DrawTool::ColorPicker => {},
      }
    }
    self.changed();
    Ok(())
  }

  fn writable_active(&mut self) -> Result<&mut CanvasLayer, String> {
    let layer = self.layers.get(self.active_layer).ok_or("Select a layer first")?;
    validate_layer(layer)?;
    if Arc::strong_count(&layer.pixels) > 1 {
      if !pixel_copy_fits(&self.layers, self.active_layer, MAX_DOCUMENT_BYTES) {
        return Err("Editing this duplicate would exceed the 512 MiB layer memory budget".into());
      }
      let pixels = clone_pixels(&layer.pixels)?;
      self.layers[self.active_layer].pixels = Arc::new(pixels);
    }
    self.enforce_history_budget();
    Ok(&mut self.layers[self.active_layer])
  }

  fn writable_mask_active(&mut self) -> Result<&mut Vec<u8>, String> {
    let layer = self.layers.get(self.active_layer).ok_or("Select a layer first")?;
    validate_layer(layer)?;
    let mask = layer.mask.as_ref().ok_or("Add a layer mask first")?;
    if Arc::strong_count(&mask.pixels) > 1 {
      if !buffer_copy_fits(&self.layers, self.active_layer, true, MAX_DOCUMENT_BYTES) {
        return Err("Editing this mask would exceed the 512 MiB layer memory budget".into());
      }
      let pixels = clone_pixels(&mask.pixels)?;
      self.layers.get_mut(self.active_layer).and_then(|layer| layer.mask.as_mut()).ok_or("Layer mask is unavailable")?.pixels = Arc::new(pixels);
    }
    self.enforce_history_budget();
    let mask = self.layers.get_mut(self.active_layer).and_then(|layer| layer.mask.as_mut()).ok_or("Layer mask is unavailable")?;
    Arc::get_mut(&mut mask.pixels).ok_or_else(|| "Layer mask is still shared".into())
  }

  fn snapshot(&self) -> DocumentSnapshot {
    DocumentSnapshot { width: self.width, height: self.height, layers: self.layers.clone(), active_layer: self.active_layer, exposure: self.exposure, contrast: self.contrast, saturation: self.saturation, levels: self.levels, levels_preview: self.levels_preview, curves: self.curves.clone(), curves_preview: self.curves_preview, initialized: self.initialized }
  }

  fn restore(&mut self, state: DocumentSnapshot) {
    self.width = state.width;
    self.height = state.height;
    self.layers = state.layers;
    self.active_layer = state.active_layer;
    self.exposure = state.exposure;
    self.contrast = state.contrast;
    self.saturation = state.saturation;
    self.levels = state.levels;
    self.levels_preview = state.levels_preview;
    self.curves = state.curves;
    self.curves_preview = state.curves_preview;
    self.edit_mask &= self.layers.get(self.active_layer).is_some_and(|layer| layer.mask.is_some());
    self.initialized = state.initialized;
    self.last_drag_uv = None;
    self.brush_gesture = BrushGesture::default();
    self.edit_mutated = false;
    self.dirty = true;
  }

  fn begin_edit(&mut self) {
    if self.pending_edit.is_none() && !self.edit_mutated {
      self.pending_edit = Some(self.snapshot());
    }
    self.enforce_history_budget();
  }

  fn changed(&mut self) {
    self.initialized = true;
    self.dirty = true;
    self.error = None;
    self.edit_mutated = true;
  }

  fn finish_edit(&mut self) {
    if let Some(previous) = self.pending_edit.take() {
      if previous != self.snapshot() {
        self.redo.clear();
        self.undo.push(previous);
      }
    } else if self.edit_mutated {
      self.redo.clear();
    }
    self.edit_mutated = false;
    self.enforce_history_budget();
  }

  fn retained_history_bytes(&self) -> usize {
    let mut seen = HashSet::new();
    let mut bytes = std::mem::size_of::<DocumentSnapshot>().saturating_mul(1 + self.undo.capacity() + self.redo.capacity() + usize::from(self.pending_edit.is_some()));
    bytes = bytes.saturating_add(self.curves.heap_bytes());
    for state in self.undo.iter().chain(self.redo.iter()).chain(self.pending_edit.iter()) {
      bytes = bytes.saturating_add(state.curves.heap_bytes());
    }
    for layers in std::iter::once(&self.layers).chain(self.undo.iter().map(|state| &state.layers)).chain(self.redo.iter().map(|state| &state.layers)).chain(self.pending_edit.iter().map(|state| &state.layers)) {
      bytes = bytes.saturating_add(layers.capacity() * std::mem::size_of::<CanvasLayer>());
      for layer in layers {
        bytes = bytes.saturating_add(layer.name.capacity());
        for buffer in std::iter::once(&layer.pixels).chain(layer.mask.iter().map(|mask| &mask.pixels)) {
          if seen.insert(Arc::as_ptr(buffer)) {
            bytes = bytes.saturating_add(buffer.capacity()).saturating_add(std::mem::size_of::<Vec<u8>>() + 2 * std::mem::size_of::<usize>());
          }
        }
      }
    }
    bytes
  }

  fn enforce_history_budget(&mut self) {
    let mut trimmed = false;
    while self.undo.len() > HISTORY_MAX_STEPS {
      self.undo.remove(0);
      trimmed = true;
    }
    while self.retained_history_bytes() > self.history_budget {
      if !self.undo.is_empty() {
        self.undo.remove(0);
      } else if !self.redo.is_empty() {
        self.redo.remove(0);
      } else if self.undo.capacity() > 0 {
        self.undo = Vec::new();
      } else if self.redo.capacity() > 0 {
        self.redo = Vec::new();
      } else if self.pending_edit.take().is_none() {
        break;
      }
      trimmed = true;
    }
    if trimmed || current_pixel_bytes(&self.layers) > self.history_budget {
      self.history_note = Some("History memory limit reached; older or oversized steps are unavailable (128 MiB).".into());
    }
  }
}

/// Validate working memory before decoding/allocating; failures reject the image, never resize it.
pub fn checked_rgba_len(width: u32, height: u32) -> Result<usize, String> {
  if width == 0 || height == 0 || width > i32::MAX as u32 || height > i32::MAX as u32 {
    return Err("Image dimensions must be positive and addressable".into());
  }
  let bytes = (width as usize).checked_mul(height as usize).and_then(|pixels| pixels.checked_mul(4)).ok_or("Image dimensions overflow memory addressing")?;
  if bytes > MAX_DOCUMENT_BYTES {
    return Err("This image exceeds the 512 MiB editing memory budget. No image was resized.".into());
  }
  Ok(bytes)
}

pub fn validate_rgba(width: u32, height: u32, pixels: &[u8]) -> Result<(), String> {
  if checked_rgba_len(width, height)? != pixels.len() {
    return Err("Image dimensions do not match its RGBA pixel buffer".into());
  }
  Ok(())
}

pub fn validate_layer(layer: &CanvasLayer) -> Result<(), String> {
  validate_layer_name(&layer.name)?;
  validate_rgba(layer.width, layer.height, &layer.pixels)?;
  layer.transform.validate()?;
  if !layer.opacity.is_finite() || !(0.0..=1.0).contains(&layer.opacity) {
    return Err("Layer opacity must be between zero and one".into());
  }
  if let Some(mask) = &layer.mask {
    if mask.pixels.len() != checked_rgba_len(layer.width, layer.height)? / 4 {
      return Err("Mask pixels do not match the source layer dimensions".into());
    }
  }
  Ok(())
}

fn validate_layer_name(name: &str) -> Result<(), String> {
  if name.len() > 4096 {
    return Err("Layer name exceeds 4096 UTF-8 bytes".into());
  }
  Ok(())
}

#[derive(Clone, Copy)]
struct LayerMapping {
  forward: [f64; 6],
  inverse: [f64; 6],
}

impl LayerMapping {
  fn new(transform: &LayerTransform, width: u32, height: u32) -> Result<Self, String> {
    transform.validate()?;
    let (sin, cos) = transform.rotation_deg.rem_euclid(360.0).to_radians().sin_cos();
    let sx = transform.scale[0] * if transform.flip_x { -1.0 } else { 1.0 };
    let sy = transform.scale[1] * if transform.flip_y { -1.0 } else { 1.0 };
    let (a, b, c, d) = (cos * sx, -sin * sy, sin * sx, cos * sy);
    let (cx, cy) = (width as f64 * 0.5, height as f64 * 0.5);
    let tx = cx + transform.offset[0] - a * cx - b * cy;
    let ty = cy + transform.offset[1] - c * cx - d * cy;
    let determinant = a * d - b * c;
    let inverse = [d / determinant, -b / determinant, -c / determinant, a / determinant, (b * ty - d * tx) / determinant, (c * tx - a * ty) / determinant];
    Ok(Self { forward: [a, b, c, d, tx, ty], inverse })
  }

  fn map(matrix: [f64; 6], point: [f64; 2]) -> [f64; 2] {
    [matrix[0] * point[0] + matrix[1] * point[1] + matrix[4], matrix[2] * point[0] + matrix[3] * point[1] + matrix[5]]
  }
  fn to_document(self, point: [f64; 2]) -> [f64; 2] {
    Self::map(self.forward, point)
  }
  fn to_local(self, point: [f64; 2]) -> [f64; 2] {
    Self::map(self.inverse, point)
  }
}

fn segment_distance(point: [f64; 2], start: [f64; 2], end: [f64; 2]) -> f64 {
  let delta = [end[0] - start[0], end[1] - start[1]];
  let length_squared = delta[0] * delta[0] + delta[1] * delta[1];
  let fraction = if length_squared > 0.0 {
    (((point[0] - start[0]) * delta[0] + (point[1] - start[1]) * delta[1]) / length_squared).clamp(0.0, 1.0)
  } else { 0.0 };
  (point[0] - start[0] - fraction * delta[0]).hypot(point[1] - start[1] - fraction * delta[1])
}

/// Intersect a transformed local pixel row with the two circular caps and central strip.
/// The union is convex, so its enclosing interval is a conservative candidate span. Exact
/// point-to-segment distance below decides coverage. No dabs or per-segment buffers are allocated.
fn capsule_row_span(mapping: LayerMapping, y: u32, width: u32, start: [f64; 2], end: [f64; 2], radius: f64) -> Option<(u32, u32)> {
  if width == 0 || !radius.is_finite() || radius <= 0.0 || start.iter().chain(end.iter()).any(|value| !value.is_finite()) {
    return None;
  }
  let origin = mapping.to_document([0.5, y as f64 + 0.5]);
  let vector = [mapping.forward[0], mapping.forward[2]];
  let speed = vector[0].hypot(vector[1]);
  if !speed.is_finite() || speed == 0.0 || origin.iter().any(|value| !value.is_finite()) {
    return None;
  }
  let unit = [vector[0] / speed, vector[1] / speed];
  // Conservative floating-point padding also covers large valid affine offsets/scales.
  let magnitude = origin.iter().chain(start.iter()).chain(end.iter()).fold(speed * width as f64, |value, next| value.max(next.abs()));
  let padded_radius = radius + 64.0 * f64::EPSILON * magnitude.max(1.0);
  let mut lower = f64::INFINITY;
  let mut upper = f64::NEG_INFINITY;
  for endpoint in [start, end] {
    let relative = [endpoint[0] - origin[0], endpoint[1] - origin[1]];
    let normal = relative[0] * unit[1] - relative[1] * unit[0];
    if normal.abs() <= padded_radius {
      let center = relative[0] * unit[0] + relative[1] * unit[1];
      let extent = (padded_radius * padded_radius - normal * normal).max(0.0).sqrt();
      lower = lower.min((center - extent) / speed);
      upper = upper.max((center + extent) / speed);
    }
  }
  let delta = [end[0] - start[0], end[1] - start[1]];
  let length = delta[0].hypot(delta[1]);
  if length > 0.0 {
    let direction = [delta[0] / length, delta[1] / length];
    let relative = [origin[0] - start[0], origin[1] - start[1]];
    let mut interval = [f64::NEG_INFINITY, f64::INFINITY];
    let along = relative[0] * direction[0] + relative[1] * direction[1];
    let along_slope = vector[0] * direction[0] + vector[1] * direction[1];
    let across = relative[0] * direction[1] - relative[1] * direction[0];
    let across_slope = vector[0] * direction[1] - vector[1] * direction[0];
    let padding = padded_radius - radius;
    if intersect_linear_interval(&mut interval, along, along_slope, -padding, length + padding)
      && intersect_linear_interval(&mut interval, across, across_slope, -padded_radius, padded_radius) {
      lower = lower.min(interval[0]);
      upper = upper.max(interval[1]);
    }
  }
  if lower > upper || upper < -1.0 || lower > width as f64 {
    return None;
  }
  let left = (lower.floor() - 1.0).clamp(0.0, width as f64) as u32;
  let right = (upper.ceil() + 2.0).clamp(0.0, width as f64) as u32;
  (left < right).then_some((left, right))
}

fn intersect_linear_interval(interval: &mut [f64; 2], origin: f64, slope: f64, lower: f64, upper: f64) -> bool {
  if slope == 0.0 {
    return lower <= origin && origin <= upper;
  }
  let a = (lower - origin) / slope;
  let b = (upper - origin) / slope;
  interval[0] = interval[0].max(a.min(b));
  interval[1] = interval[1].min(a.max(b));
  interval[0] <= interval[1]
}

fn composite_layer(output: &mut [u8], document_w: u32, document_h: u32, layer: &CanvasLayer, first: bool) -> Result<(), String> {
  let mask = layer.mask.as_ref().filter(|mask| mask.enabled);
  if layer.transform == LayerTransform::default() {
    for y in 0..layer.height.min(document_h) as usize {
      if first && layer.opacity == 1.0 && mask.is_none() {
        let width = layer.width.min(document_w) as usize;
        let source = y * layer.width as usize * 4;
        let target = y * document_w as usize * 4;
        output[target..target + width * 4].copy_from_slice(&layer.pixels[source..source + width * 4]);
        continue;
      }
      for x in 0..layer.width.min(document_w) as usize {
        let source = (y * layer.width as usize + x) * 4;
        let target = (y * document_w as usize + x) * 4;
        let coverage = mask.map_or(255, |mask| mask.pixels[y * layer.width as usize + x]);
        if first && layer.opacity == 1.0 {
          output[target..target + 4].copy_from_slice(&layer.pixels[source..source + 4]);
          output[target + 3] = ((layer.pixels[source + 3] as u32 * coverage as u32 + 127) / 255) as u8;
        } else {
          let rgba = [layer.pixels[source] as f64, layer.pixels[source + 1] as f64, layer.pixels[source + 2] as f64, layer.pixels[source + 3] as f64 / 255.0 * coverage as f64 / 255.0];
          blend_sample(output, target, rgba, layer.blend, layer.opacity);
        }
      }
    }
    return Ok(());
  }
  let mapping = LayerMapping::new(&layer.transform, layer.width, layer.height)?;
  let corners = [[0.0, 0.0], [layer.width as f64, 0.0], [0.0, layer.height as f64], [layer.width as f64, layer.height as f64]].map(|point| mapping.to_document(point));
  let left = corners.iter().map(|point| point[0]).fold(f64::INFINITY, f64::min).floor().clamp(0.0, document_w as f64) as u32;
  let right = corners.iter().map(|point| point[0]).fold(f64::NEG_INFINITY, f64::max).ceil().clamp(0.0, document_w as f64) as u32;
  let top = corners.iter().map(|point| point[1]).fold(f64::INFINITY, f64::min).floor().clamp(0.0, document_h as f64) as u32;
  let bottom = corners.iter().map(|point| point[1]).fold(f64::NEG_INFINITY, f64::max).ceil().clamp(0.0, document_h as f64) as u32;
  for y in top..bottom {
    for x in left..right {
      let local = mapping.to_local([x as f64 + 0.5, y as f64 + 0.5]);
      let rgba = sample_layer(layer, local);
      blend_sample(output, (y as usize * document_w as usize + x as usize) * 4, rgba, layer.blend, layer.opacity);
    }
  }
  Ok(())
}

/// Bilinear interpolation of premultiplied color and masked alpha avoids transparent-edge halos.
fn sample_layer(layer: &CanvasLayer, point: [f64; 2]) -> [f64; 4] {
  if point.iter().any(|value| !value.is_finite()) || point[0] < -0.5 || point[1] < -0.5 || point[0] > layer.width as f64 + 0.5 || point[1] > layer.height as f64 + 0.5 {
    return [0.0; 4];
  }
  let x = point[0] - 0.5;
  let y = point[1] - 0.5;
  let (left, top) = (x.floor() as i64, y.floor() as i64);
  let (fx, fy) = (x - x.floor(), y - y.floor());
  let mask = layer.mask.as_ref().filter(|mask| mask.enabled);
  let mut sampled = [0.0; 4];
  for (dy, wy) in [(0, 1.0 - fy), (1, fy)] {
    for (dx, wx) in [(0, 1.0 - fx), (1, fx)] {
      let (sx, sy) = (left + dx, top + dy);
      if sx < 0 || sy < 0 || sx >= layer.width as i64 || sy >= layer.height as i64 {
        continue;
      }
      let pixel = sy as usize * layer.width as usize + sx as usize;
      let index = pixel * 4;
      let alpha = layer.pixels[index + 3] as f64 / 255.0 * mask.map_or(1.0, |mask| mask.pixels[pixel] as f64 / 255.0) * wx * wy;
      for channel in 0..3 {
        sampled[channel] += layer.pixels[index + channel] as f64 * alpha;
      }
      sampled[3] += alpha;
    }
  }
  if sampled[3] > 0.0 {
    for channel in 0..3 {
      sampled[channel] /= sampled[3];
    }
  }
  sampled
}

fn blend_sample(output: &mut [u8], index: usize, source: [f64; 4], blend: BlendMode, opacity: f32) {
  let alpha = source[3] * opacity as f64;
  if alpha <= 0.0 {
    return;
  }
  let destination_alpha = output[index + 3] as f64 / 255.0;
  let combined = alpha + destination_alpha * (1.0 - alpha);
  for channel in 0..3 {
    let destination = output[index + channel];
    let blended = (1.0 - destination_alpha) * source[channel] + destination_alpha * blend.blend_channel(destination, source[channel].round().clamp(0.0, 255.0) as u8) as f64;
    output[index + channel] = ((blended * alpha + destination as f64 * destination_alpha * (1.0 - alpha)) / combined).round().clamp(0.0, 255.0) as u8;
  }
  output[index + 3] = (combined * 255.0).round().clamp(0.0, 255.0) as u8;
}

fn allocate_rgba(width: u32, height: u32) -> Result<Vec<u8>, String> {
  let length = checked_rgba_len(width, height)?;
  let mut pixels = Vec::new();
  pixels.try_reserve_exact(length).map_err(|_| "Not enough memory for the original-size image")?;
  pixels.resize(length, 0);
  Ok(pixels)
}

fn clone_pixels(source: &[u8]) -> Result<Vec<u8>, String> {
  let mut pixels = Vec::new();
  pixels.try_reserve_exact(source.len()).map_err(|_| "Not enough memory for this layer edit")?;
  pixels.extend_from_slice(source);
  Ok(pixels)
}

fn layer_buffers(layers: &[CanvasLayer]) -> impl Iterator<Item = &Arc<Vec<u8>>> {
  layers.iter().flat_map(|layer| std::iter::once(&layer.pixels).chain(layer.mask.iter().map(|mask| &mask.pixels)))
}

fn current_pixel_bytes(layers: &[CanvasLayer]) -> usize {
  let mut seen = HashSet::new();
  layer_buffers(layers).filter(|buffer| seen.insert(Arc::as_ptr(buffer))).fold(0usize, |bytes, buffer| bytes.saturating_add(buffer.capacity()))
}

fn buffer_copy_fits(layers: &[CanvasLayer], active: usize, mask: bool, budget: usize) -> bool {
  let Some(layer) = layers.get(active) else {
    return false;
  };
  let target = if mask {
    match &layer.mask {
      Some(mask) => &mask.pixels,
      None => return false,
    }
  } else {
    &layer.pixels
  };
  let shared_in_current = layer_buffers(layers).filter(|other| Arc::ptr_eq(other, target)).count() > 1;
  current_pixel_bytes(layers).saturating_add(if shared_in_current { target.len() } else { 0 }) <= budget
}

fn pixel_copy_fits(layers: &[CanvasLayer], active: usize, budget: usize) -> bool {
  buffer_copy_fits(layers, active, false, budget)
}

fn document_pixel_from_uv(width: u32, height: u32, uv: Pos2) -> Option<(u32, u32)> {
  if width == 0 || height == 0 || !uv.x.is_finite() || !uv.y.is_finite() {
    return None;
  }
  let x = (uv.x.clamp(0.0, 1.0) * width as f32).floor() as u32;
  let y = (uv.y.clamp(0.0, 1.0) * height as f32).floor() as u32;
  Some((x.min(width - 1), y.min(height - 1)))
}

fn view_rect(viewport: Rect, width: u32, height: u32, zoom: f32, pixels_per_point: f32, pan: Vec2) -> Rect {
  Rect::from_center_size(viewport.center() + pan, vec2(width as f32, height as f32) * zoom / pixels_per_point.max(0.01))
}

fn visible_preview_region(canvas: Rect, viewport: Rect, width: u32, height: u32, zoom: f32, maximum: u32) -> [u32; 6] {
  let visible = canvas.intersect(viewport);
  let x = (((visible.left() - canvas.left()) / canvas.width()).clamp(0.0, 1.0) * width as f32).floor() as u32;
  let y = (((visible.top() - canvas.top()) / canvas.height()).clamp(0.0, 1.0) * height as f32).floor() as u32;
  let right = ((((visible.right() - canvas.left()) / canvas.width()).clamp(0.0, 1.0) * width as f32).ceil() as u32).max(x.saturating_add(1)).min(width);
  let bottom = ((((visible.bottom() - canvas.top()) / canvas.height()).clamp(0.0, 1.0) * height as f32).ceil() as u32).max(y.saturating_add(1)).min(height);
  let (x, y) = (x.min(width - 1), y.min(height - 1));
  let (w, h) = ((right - x).max(1), (bottom - y).max(1));
  let scale = zoom.min(1.0).min(maximum.max(1) as f32 / w as f32).min(maximum.max(1) as f32 / h as f32);
  [x, y, w, h, ((w as f32 * scale).ceil().max(1.0) as u32).min(maximum.max(1)), ((h as f32 * scale).ceil().max(1.0) as u32).min(maximum.max(1))]
}

/// Checks declared dimensions before decoding and bounds the decoder allocation budget.
pub fn decode_original_rgba(path: &Path) -> Result<image::RgbaImage, String> {
  let (width, height) = image::ImageReader::open(path).map_err(|error| error.to_string())?.with_guessed_format().map_err(|error| error.to_string())?.into_dimensions().map_err(|error| error.to_string())?;
  checked_rgba_len(width, height)?;
  let mut reader = image::ImageReader::open(path).map_err(|error| error.to_string())?.with_guessed_format().map_err(|error| error.to_string())?;
  let mut limits = image::Limits::default();
  limits.max_alloc = Some(MAX_DOCUMENT_BYTES as u64);
  reader.limits(limits);
  let decoded = reader.decode().map_err(|error| format!("Image decode failed: {error}"))?;
  checked_rgba_len(decoded.width(), decoded.height())?;
  match decoded {
    image::DynamicImage::ImageRgba8(image) => Ok(image),
    other => {
      let (width, height) = other.dimensions();
      let mut pixels = allocate_rgba(width, height)?;
      for ((_, _, pixel), target) in other.pixels().zip(pixels.chunks_exact_mut(4)) {
        target.copy_from_slice(&pixel.0);
      }
      image::RgbaImage::from_raw(width, height, pixels).ok_or_else(|| "Decoded image pixels do not match its dimensions".into())
    },
  }
}

impl DrawTool {
  pub const ALL: [Self; 7] = [Self::Move, Self::Brush, Self::Eraser, Self::LiquifyPush, Self::LiquifyBloat, Self::LiquifyPucker, Self::ColorPicker];

  pub fn label(self) -> &'static str {
    match self {
      Self::Move => "Move Layer",
      Self::Brush => "Brush",
      Self::Eraser => "Eraser",
      Self::LiquifyPush => "Liquify: Push",
      Self::LiquifyBloat => "Liquify: Bloat",
      Self::LiquifyPucker => "Liquify: Pucker",
      Self::ColorPicker => "Eyedropper",
    }
  }
}

impl BlendMode {
  pub const ALL: [Self; 5] = [Self::Normal, Self::Multiply, Self::Screen, Self::Overlay, Self::Add];

  pub fn label(self) -> &'static str {
    match self {
      Self::Normal => "Normal",
      Self::Multiply => "Multiply",
      Self::Screen => "Screen",
      Self::Overlay => "Overlay",
      Self::Add => "Add",
    }
  }

  /// The separable blend function `B(Cb, Cs)` for one channel.
  pub fn blend_channel(self, base: u8, top: u8) -> u8 {
    let b = base as f32 / 255.0;
    let t = top as f32 / 255.0;
    let out = match self {
      Self::Normal => t,
      Self::Multiply => b * t,
      Self::Screen => 1.0 - (1.0 - b) * (1.0 - t),
      Self::Overlay => {
        if b < 0.5 {
          2.0 * b * t
        } else {
          1.0 - 2.0 * (1.0 - b) * (1.0 - t)
        }
      },
      Self::Add => (b + t).min(1.0),
    };
    (out.clamp(0.0, 1.0) * 255.0).round() as u8
  }
}

impl FilterPreset {
  pub const ALL: [Self; 5] = [Self::EdgeGlow, Self::Dither, Self::Posterize, Self::FilmGrain, Self::WarmGrade];

  pub fn label(self) -> &'static str {
    match self {
      Self::EdgeGlow => "Edge Glow",
      Self::Dither => "Two-Tone Dither",
      Self::Posterize => "Posterize",
      Self::FilmGrain => "Film Grain",
      Self::WarmGrade => "Warm Grade",
    }
  }
}

impl CanvasLayer {
  pub fn new_solid(name: impl Into<String>, width: u32, height: u32, rgba: [u8; 4]) -> Self {
    Self {
      name: name.into(),
      visible: true,
      opacity: 1.0,
      blend: BlendMode::Normal,
      width,
      height,
      pixels: Arc::new(
        checked_rgba_len(width, height)
          .ok()
          .and_then(|_| {
            let mut pixels = allocate_rgba(width, height).ok()?;
            for pixel in pixels.chunks_exact_mut(4) {
              pixel.copy_from_slice(&rgba);
            }
            Some(pixels)
          })
          .unwrap_or_default(),
      ),
      mask: None,
      transform: LayerTransform::default(),
    }
  }
}

/// `outer` shrunk to the largest centered rect with the given width / height `aspect`.
pub fn fit_rect(outer: Rect, aspect: f32) -> Rect {
  Rect::from_center_size(outer.center(), fit_inside(vec2(aspect, 1.0), outer.size()))
}

/// The largest size with the source's aspect ratio that fits inside `max_w x max_h` (at least 1 x 1).
pub fn fit_dims(src_w: u32, src_h: u32, max_w: u32, max_h: u32) -> (u32, u32) {
  if src_w == 0 || src_h == 0 {
    return (max_w, max_h);
  }
  let scale = (max_w as f64 / src_w as f64).min(max_h as f64 / src_h as f64);
  let w = ((src_w as f64 * scale).round() as u32).clamp(1, max_w.max(1));
  let h = ((src_h as f64 * scale).round() as u32).clamp(1, max_h.max(1));
  (w, h)
}

/// Scales `src` (`src_w x src_h` RGBA8) to fit inside `dst_w x dst_h`, keeping its aspect ratio,
/// centered on a transparent background.
pub fn fit_rgba(src: &[u8], src_w: u32, src_h: u32, dst_w: u32, dst_h: u32) -> Vec<u8> {
  let Ok(mut out) = allocate_rgba(dst_w, dst_h) else {
    return Vec::new();
  };
  if validate_rgba(src_w, src_h, src).is_err() {
    return out;
  }
  let (fit_w, fit_h) = fit_dims(src_w, src_h, dst_w, dst_h);
  let scaled = resample_rgba(src, src_w, src_h, fit_w, fit_h);
  if scaled.len() != checked_rgba_len(fit_w, fit_h).unwrap_or(0) {
    return out;
  }
  let row_len = fit_w as usize * 4;
  let x0 = ((dst_w - fit_w) / 2) as usize;
  let y0 = ((dst_h - fit_h) / 2) as usize;
  for row in 0..fit_h as usize {
    let to = ((y0 + row) * dst_w as usize + x0) * 4;
    out[to..to + row_len].copy_from_slice(&scaled[row * row_len..(row + 1) * row_len]);
  }
  out
}

/// Nearest-neighbour resize of `src` (`src_w x src_h` RGBA8) to exactly `dst_w x dst_h`.
pub fn resample_rgba(src: &[u8], src_w: u32, src_h: u32, dst_w: u32, dst_h: u32) -> Vec<u8> {
  let Ok(mut out) = allocate_rgba(dst_w, dst_h) else {
    return Vec::new();
  };
  if validate_rgba(src_w, src_h, src).is_err() {
    return out;
  }
  let (sw, sh, dw, dh) = (src_w as usize, src_h as usize, dst_w as usize, dst_h as usize);
  for y in 0..dh {
    let sy = (y * sh / dh).min(sh - 1);
    for x in 0..dw {
      let sx = (x * sw / dw).min(sw - 1);
      let s = (sy * sw + sx) * 4;
      let d = (y * dw + x) * 4;
      out[d..d + 4].copy_from_slice(&src[s..s + 4]);
    }
  }
  out
}

/// Sharpens color detail (each pixel against its 4-neighbour average) and, on soft alpha edges,
/// shifts and steepens the alpha ramp (`edge_shift` grows or shrinks the edge).
pub fn refine_edges_rgba(pixels: &mut [u8], width: u32, height: u32, sharpen: f32, edge_contrast: f32, edge_shift: f32) -> Result<(), String> {
  validate_rgba(width, height, pixels)?;
  if width < 2 || height < 2 {
    return Ok(());
  }
  let src = clone_pixels(pixels)?;
  let s = sharpen.clamp(0.0, 2.0);
  let c_k = (1.0 + edge_contrast.clamp(0.0, 2.0) * 2.2).max(1.0);
  let shift = edge_shift.clamp(-1.0, 1.0) * 0.35;

  for y in 0..height {
    let ym = y.saturating_sub(1);
    let yp = (y + 1).min(height - 1);
    for x in 0..width {
      let xm = x.saturating_sub(1);
      let xp = (x + 1).min(width - 1);
      let idx = ((y * width + x) * 4) as usize;
      let neighbours = [((y * width + xm) * 4) as usize, ((y * width + xp) * 4) as usize, ((ym * width + x) * 4) as usize, ((yp * width + x) * 4) as usize];
      let average = |c: usize| neighbours.iter().map(|&n| src[n + c] as f32).sum::<f32>() * 0.25;

      for c in 0..3 {
        let center = src[idx + c] as f32;
        let detail = center - average(c);
        pixels[idx + c] = (center + detail * s * 1.6).clamp(0.0, 255.0).round() as u8;
      }

      let a_c = src[idx + 3] as f32 / 255.0;
      let a_grad = (a_c - average(3) / 255.0).abs();
      if a_grad > 1e-3 || (0.02..0.98).contains(&a_c) {
        let shifted = (a_c + shift).clamp(0.0, 1.0);
        let contrasted = ((shifted - 0.5) * c_k + 0.5).clamp(0.0, 1.0);
        pixels[idx + 3] = (contrasted * 255.0).round() as u8;
      }
    }
  }
  Ok(())
}

/// Exposure (stops), contrast and saturation on an RGBA8 buffer; alpha is untouched.
pub fn apply_adjustments(rgba: &mut [u8], exposure: f32, contrast: f32, saturation: f32) {
  if exposure.abs() < 1e-4 && contrast.abs() < 1e-4 && saturation.abs() < 1e-4 {
    return;
  }
  let exp_mul = 2.0f32.powf(exposure);
  let contrast_k = (1.0 + contrast).max(0.0);
  let sat_k = (1.0 + saturation).max(0.0);
  for px in rgba.chunks_exact_mut(4) {
    let [r, g, b] = [px[0], px[1], px[2]].map(|v| ((v as f32 / 255.0) * exp_mul - 0.5) * contrast_k + 0.5);
    let luma = 0.2126 * r + 0.7152 * g + 0.0722 * b;
    for (c, v) in [r, g, b].into_iter().enumerate() {
      px[c] = ((luma + (v - luma) * sat_k).clamp(0.0, 1.0) * 255.0).round() as u8;
    }
  }
}

/// A local displacement brush: push along `delta`, bloat (magnify) or pucker (shrink) around
/// `(cx, cy)` within `radius`.
#[allow(clippy::too_many_arguments)]
pub fn apply_liquify_brush(pixels: &mut [u8], width: u32, height: u32, cx: f32, cy: f32, radius: f32, mode: DrawTool, delta: Vec2) -> Result<(), String> {
  validate_rgba(width, height, pixels)?;
  let src = clone_pixels(pixels)?;
  let r = radius.max(2.0);
  let min_x = (cx - r).floor().max(0.0) as u32;
  let max_x = (cx + r).ceil().min((width - 1) as f32) as u32;
  let min_y = (cy - r).floor().max(0.0) as u32;
  let max_y = (cy + r).ceil().min((height - 1) as f32) as u32;

  for y in min_y..=max_y {
    for x in min_x..=max_x {
      let dx = x as f32 - cx;
      let dy = y as f32 - cy;
      let dist = (dx * dx + dy * dy).sqrt();
      if dist >= r {
        continue;
      }
      let weight = (1.0 - dist / r).powi(2);
      let (sx, sy) = match mode {
        DrawTool::LiquifyPush => (x as f32 - delta.x * weight * 0.8, y as f32 - delta.y * weight * 0.8),
        DrawTool::LiquifyBloat => (x as f32 - dx * weight * 0.35, y as f32 - dy * weight * 0.35),
        DrawTool::LiquifyPucker => (x as f32 + dx * weight * 0.35, y as f32 + dy * weight * 0.35),
        _ => (x as f32, y as f32),
      };
      let sx = sx.round().clamp(0.0, (width - 1) as f32) as u32;
      let sy = sy.round().clamp(0.0, (height - 1) as f32) as u32;
      let s_idx = ((sy * width + sx) * 4) as usize;
      let d_idx = ((y * width + x) * 4) as usize;
      pixels[d_idx..d_idx + 4].copy_from_slice(&src[s_idx..s_idx + 4]);
    }
  }
  Ok(())
}

/// Applies a filter to an RGBA8 buffer, mixed with the original by `intensity`.
pub fn apply_filter(pixels: &mut [u8], width: u32, height: u32, preset: FilterPreset, intensity: f32) -> Result<(), String> {
  validate_rgba(width, height, pixels)?;
  let src = clone_pixels(pixels)?;
  let mix = intensity.clamp(0.0, 1.0);
  for y in 0..height {
    for x in 0..width {
      let idx = ((y * width + x) * 4) as usize;
      let r = src[idx] as f32;
      let g = src[idx + 1] as f32;
      let b = src[idx + 2] as f32;
      let (fr, fg, fb) = match preset {
        FilterPreset::EdgeGlow => {
          let rx = ((y * width + (x + 1).min(width - 1)) * 4) as usize;
          let ry = (((y + 1).min(height - 1) * width + x) * 4) as usize;
          let gx = (src[rx] as f32 - r).abs() + (src[rx + 1] as f32 - g).abs() + (src[rx + 2] as f32 - b).abs();
          let gy = (src[ry] as f32 - r).abs() + (src[ry + 1] as f32 - g).abs() + (src[ry + 2] as f32 - b).abs();
          let edge = ((gx + gy) * 1.4).clamp(0.0, 255.0);
          (edge * 0.6, edge * 0.9, (edge * 1.3).min(255.0))
        },
        FilterPreset::Dither => {
          let cell = (((x / 4) + (y / 4)) % 2) as f32;
          let luma = 0.299 * r + 0.587 * g + 0.114 * b;
          let v = if luma > (90.0 + cell * 65.0) { 240.0 } else { 28.0 };
          (v, v, v)
        },
        FilterPreset::Posterize => {
          let q = |v: f32| (v / 42.0).round() * 42.0;
          (q(r), q(g), q(b))
        },
        FilterPreset::FilmGrain => {
          let hash = (x.wrapping_mul(374_761_393).wrapping_add(y.wrapping_mul(668_265_263))) ^ 0x5BF0_3635;
          let noise = ((hash & 0xFF) as f32 - 128.0) * 0.32;
          (r + noise, g + noise, b + noise)
        },
        FilterPreset::WarmGrade => (255.0 - (255.0 - r) * 0.75, g * 0.92 + 12.0, b * 0.78),
      };
      pixels[idx] = (r * (1.0 - mix) + fr * mix).clamp(0.0, 255.0).round() as u8;
      pixels[idx + 1] = (g * (1.0 - mix) + fg * mix).clamp(0.0, 255.0).round() as u8;
      pixels[idx + 2] = (b * (1.0 - mix) + fb * mix).clamp(0.0, 255.0).round() as u8;
    }
  }
  Ok(())
}

#[cfg(test)]
mod tests {
  use super::*;

  fn page(width: u32, height: u32, pixels: Vec<u8>) -> EditImagePage {
    let mut page = EditImagePage::default();
    page.import_rgba_layer(width, height, pixels, "Source".into()).unwrap();
    page.undo.clear();
    page
  }

  fn pattern(width: u32, height: u32) -> Vec<u8> {
    let mut pixels = allocate_rgba(width, height).unwrap();
    for (index, pixel) in pixels.chunks_exact_mut(4).enumerate() {
      pixel.copy_from_slice(&[(index % 251) as u8, ((index / width as usize) % 253) as u8, ((index * 7) % 255) as u8, ((index % 4) * 85) as u8]);
    }
    pixels
  }

  #[test]
  fn original_4096_by_3072_rgba_and_png_roundtrip_are_exact() {
    let (width, height) = (4096, 3072);
    let original = pattern(width, height);
    let page = page(width, height, original.clone());
    assert_eq!((page.width, page.height), (width, height));
    assert_eq!(page.layers.len(), 1, "no forced white backing layer");
    assert_eq!(page.composite_rgba().unwrap(), original);
    let exported = image::load_from_memory(&page.composite_png().unwrap()).unwrap().to_rgba8();
    assert_eq!(exported.dimensions(), (width, height));
    assert_eq!(exported.into_raw(), original, "including hidden RGB at alpha zero and partial alpha");
  }

  #[test]
  fn imported_portrait_and_odd_dimensions_preserve_every_pixel() {
    for (width, height) in [(17, 1001), (1003, 19), (1, 501), (503, 1)] {
      let original = pattern(width, height);
      let page = page(width, height, original.clone());
      assert_eq!(page.composite_rgba().unwrap(), original);
      assert_eq!(page.layers[0].pixels.as_ref(), &original);
      let png = image::load_from_memory(&page.composite_png().unwrap()).unwrap();
      assert_eq!((png.width(), png.height()), (width, height));
    }
  }

  #[test]
  fn imported_images_keep_their_aspect_ratio_without_resampling() {
    let original = [255, 0, 0, 128].repeat(100 * 100);
    let page = page(100, 100, original.clone());
    assert_eq!((page.width, page.height), (100, 100));
    assert_eq!((page.layers[0].width, page.layers[0].height), (100, 100));
    assert_eq!(page.composite_rgba().unwrap(), original);
  }

  #[test]
  fn added_layers_keep_original_sizes_and_expand_canvas_at_top_left() {
    let mut page = page(3, 2, [200, 20, 40, 255].repeat(6));
    let base = page.layers[0].pixels.clone();
    let top = [10, 200, 30, 255].repeat(5);
    page.import_rgba_layer(1, 5, top.clone(), "Tall".into()).unwrap();
    assert_eq!((page.width, page.height), (3, 5));
    assert_eq!((page.layers[0].width, page.layers[0].height), (3, 2));
    assert_eq!((page.layers[1].width, page.layers[1].height), (1, 5));
    assert_eq!(page.layers[0].pixels, base);
    assert_eq!(page.layers[1].pixels.as_ref(), &top);
    let composite = page.composite_rgba().unwrap();
    assert_eq!(composite[..8], [10, 200, 30, 255, 200, 20, 40, 255]);
    assert_eq!(&composite[(4 * 3) * 4..(4 * 3 + 1) * 4], &[10, 200, 30, 255]);
    assert_eq!(&composite[(4 * 3 + 1) * 4..(4 * 3 + 2) * 4], &[0, 0, 0, 0]);
  }

  #[test]
  fn importing_after_brush_edit_adds_instead_of_replacing_the_document() {
    let mut page = EditImagePage::default();
    page.apply_tool_at(pos2(0.5, 0.5), None).unwrap();
    let edited = page.layers[0].pixels.clone();
    page.import_rgba_layer(1, 1, vec![4, 3, 2, 1], "Added".into()).unwrap();
    assert_eq!(page.layers.len(), 2);
    assert_eq!((page.width, page.height), (DEFAULT_CANVAS_W, DEFAULT_CANVAS_H));
    assert_eq!(page.layers[0].pixels, edited);
  }

  #[test]
  fn invalid_dimensions_and_buffers_leave_the_document_unchanged() {
    let mut page = page(7, 9, pattern(7, 9));
    let before = page.snapshot();
    for (width, height, pixels) in [(0, 1, vec![]), (1, 0, vec![]), (2, 2, vec![0; 15]), (1, 1, vec![0; 5]), (u32::MAX, u32::MAX, vec![])] {
      assert!(page.import_rgba_layer(width, height, pixels, "Invalid".into()).is_err());
      assert!(page.snapshot() == before);
    }
    assert!(checked_rgba_len(100_000, 100_000).is_err());
    assert!(page.open_rgba_document(0, 1, vec![], "Invalid".into()).is_err());
    assert!(page.snapshot() == before);
  }

  #[test]
  fn brush_coordinates_use_document_dimensions_and_active_layer_bounds() {
    let mut page = page(1001, 501, vec![0; 1001 * 501 * 4]);
    page.brush_radius = 1.0;
    page.brush_color = [230, 70, 20, 255];
    page.apply_tool_at(pos2(0.9, 0.8), None).unwrap();
    let index = (400 * 1001 + 900) * 4;
    assert_eq!(&page.layers[0].pixels[index..index + 4], &[230, 70, 20, 255]);
    page.import_rgba_layer(2, 2, vec![0; 16], "Small".into()).unwrap();
    page.apply_tool_at(pos2(0.9, 0.8), None).unwrap();
    assert_eq!(page.layers[1].pixels.as_slice(), &[0; 16], "outside the small layer is not rescaled into it");
    assert_eq!(document_pixel_from_uv(1001, 501, pos2(1.0, 1.0)), Some((1000, 500)));
    assert!(document_pixel_from_uv(1001, 501, pos2(f32::NAN, 0.0)).is_none());
  }

  #[test]
  fn semitransparent_brush_uses_straight_alpha_source_over() {
    let mut page = page(1, 1, vec![255, 0, 0, 128]);
    page.brush_radius = 1.0;
    page.brush_color = [0, 0, 255, 128];
    page.apply_tool_at(pos2(0.0, 0.0), None).unwrap();
    assert_eq!(page.layers[0].pixels.as_slice(), &[85, 0, 170, 192]);
  }

  #[test]
  fn fit_zoom_and_retina_100_percent_only_change_the_view() {
    let page = page(4097, 3, pattern(4097, 3));
    let viewport = Rect::from_min_size(Pos2::ZERO, vec2(800.0, 400.0));
    let actual = view_rect(viewport, page.width, page.height, 1.0, 2.0, Vec2::ZERO);
    assert_eq!(actual.width(), 4097.0 / 2.0, "one image pixel per physical monitor pixel");
    let region = visible_preview_region(actual, viewport, page.width, page.height, 1.0, 2048);
    assert_eq!(region[2], region[4], "100% uploads the full-resolution visible crop");
    assert_eq!(region[3], region[5]);
    assert!(region[2] < page.width);
    assert_eq!(page.composite_rgba().unwrap(), pattern(4097, 3));
    let fit = view_rect(viewport, 4097, 3, 800.0 * 2.0 / 4097.0, 2.0, vec2(30.0, 20.0));
    let physical_point = fit.min + fit.size() * vec2(0.75, 0.5);
    let uv = (physical_point - fit.min) / fit.size();
    assert_eq!(document_pixel_from_uv(4097, 3, pos2(uv.x, uv.y)), Some((3072, 1)));
  }

  #[test]
  fn filters_and_edges_use_the_active_layers_dynamic_dimensions() {
    let (width, height) = (701, 411);
    let mut layer = CanvasLayer::new_solid("Split", width, height, [0, 0, 0, 255]);
    for (index, pixel) in Arc::get_mut(&mut layer.pixels).unwrap().chunks_exact_mut(4).enumerate() {
      if index as u32 % width >= width / 2 {
        pixel.copy_from_slice(&[255, 255, 255, 255]);
      }
    }
    let mut page = page(width, height, layer.pixels.as_ref().clone());
    page.filter = FilterPreset::EdgeGlow;
    page.filter_intensity = 1.0;
    page.apply_filter_to_active().unwrap();
    let pixels = &page.layers[0].pixels;
    let at = |x: u32| ((10 * width + x) * 4) as usize;
    assert_eq!(pixels[at(width / 2 - 1) + 2], 255, "the pixel next to the edge glows");
    assert_eq!(pixels[at(600)..at(600) + 3], [0, 0, 0], "a flat area goes dark");
    page.refine_edges_of_active().unwrap();
    assert_eq!(page.composite_rgba().unwrap().len(), width as usize * height as usize * 4);
  }

  #[test]
  fn liquify_preserves_dynamic_dimensions_and_undo_restores_pixels() {
    let original = pattern(723, 417);
    let mut page = page(723, 417, original.clone());
    page.tool = DrawTool::LiquifyPush;
    page.apply_tool_at(pos2(0.5, 0.5), Some(pos2(0.45, 0.5))).unwrap();
    assert_ne!(page.layers[0].pixels.as_ref(), &original);
    assert_eq!(page.layers[0].pixels.len(), original.len());
    assert!(page.undo());
    assert_eq!(page.layers[0].pixels.as_ref(), &original);
  }

  #[test]
  fn a_drag_is_one_history_step_and_new_edits_clear_redo() {
    let mut page = page(41, 27, vec![0; 41 * 27 * 4]);
    let original = page.composite_rgba().unwrap();
    page.begin_edit();
    for x in [0.2, 0.4, 0.6] {
      page.apply_tool_stamp(pos2(x, 0.5), None).unwrap();
    }
    page.finish_edit();
    assert_eq!(page.undo.len(), 1);
    let painted = page.composite_rgba().unwrap();
    assert_ne!(painted, original);
    assert!(page.undo());
    assert_eq!(page.composite_rgba().unwrap(), original);
    assert!(page.redo());
    assert_eq!(page.composite_rgba().unwrap(), painted);
    assert!(page.undo());
    page.apply_tool_at(pos2(0.9, 0.5), None).unwrap();
    assert!(page.redo.is_empty());
  }

  #[test]
  fn undo_redo_restores_import_dimensions_layer_stack_and_adjustments() {
    let mut page = page(7, 5, pattern(7, 5));
    let original = page.snapshot();
    page.import_rgba_layer(9, 3, pattern(9, 3), "Second".into()).unwrap();
    assert!(page.undo());
    assert!(page.snapshot() == original);
    assert!(page.redo());
    assert_eq!((page.width, page.height, page.active_layer), (9, 5, 1));
    page.begin_edit();
    page.exposure = 0.6;
    page.levels.gamma = 1.5;
    page.changed();
    page.finish_edit();
    assert!(page.undo());
    assert_eq!((page.exposure, page.levels.gamma), (0.0, 1.0));
    page.begin_edit();
    page.layers.remove(1);
    page.active_layer = 0;
    page.changed();
    page.finish_edit();
    assert!(page.undo());
    assert_eq!((page.layers.len(), page.active_layer), (2, 1));
    page.open_rgba_document(3, 11, pattern(3, 11), "Opened".into()).unwrap();
    assert!(page.undo());
    assert_eq!((page.width, page.height, page.layers.len()), (9, 5, 2));
  }

  #[test]
  fn history_counts_unique_shared_buffers_and_drops_oversized_steps() {
    let mut page = page(16, 16, vec![0; 16 * 16 * 4]);
    page.history_budget = 16 * 16 * 4 * 3;
    for index in 0..12 {
      page.apply_tool_at(pos2(index as f32 / 12.0, 0.5), None).unwrap();
    }
    assert!(page.retained_history_bytes() <= page.history_budget);
    assert!(page.history_note.is_some());
    page.history_budget = 100;
    page.apply_tool_at(pos2(0.3, 0.2), None).unwrap();
    assert!(page.undo.is_empty() && page.redo.is_empty() && page.pending_edit.is_none());
    assert!(page.history_note.is_some());
  }

  #[test]
  fn idle_frames_and_zoom_do_not_clear_redo_after_a_trim_notice() {
    let mut page = page(41, 27, vec![0; 41 * 27 * 4]);
    page.apply_tool_at(pos2(0.3, 0.3), None).unwrap();
    assert!(page.undo());
    page.history_note = Some("Earlier history was trimmed".into());
    page.finish_edit();
    page.zoom = 2.0;
    page.pan = vec2(40.0, 50.0);
    page.finish_edit();
    assert!(page.redo(), "only a new document edit invalidates redo");
  }

  #[test]
  fn adding_a_blank_layer_to_a_pristine_document_does_not_replace_it() {
    let mut page = EditImagePage::default();
    let original = page.layers[0].pixels.clone();
    page.add_blank_layer().unwrap();
    assert_eq!(page.layers.len(), 2);
    assert_eq!(page.active_layer, 1);
    assert!(Arc::ptr_eq(&page.layers[0].pixels, &original));
    assert!(page.undo());
    assert_eq!(page.layers.len(), 1);
  }

  #[test]
  fn duplicate_pixel_copy_is_validated_against_unique_current_buffers() {
    let mut page = page(4, 4, vec![0; 64]);
    page.layers.push(page.layers[0].clone());
    assert_eq!(current_pixel_bytes(&page.layers), 64);
    assert!(!pixel_copy_fits(&page.layers, 1, 127));
    assert!(pixel_copy_fits(&page.layers, 1, 128));
    page.layers.pop();
    assert!(pixel_copy_fits(&page.layers, 0, 64), "history sharing alone doesn't grow current document buffers");
  }

  #[test]
  fn preview_dimensions_never_exceed_a_nondivisible_gpu_limit() {
    for (width, height, maximum) in [(983, 257, 257), (1001, 997, 127), (33_001, 3, 512)] {
      let canvas = Rect::from_min_size(Pos2::ZERO, vec2(width as f32, height as f32));
      let region = visible_preview_region(canvas, canvas, width, height, 1.0, maximum);
      assert!(region[4] <= maximum && region[5] <= maximum);
      assert!(region[4] > 0 && region[5] > 0);
    }
  }

  mod compositing {
    use super::*;

    #[test]
    fn blend_modes_show_the_layer_as_is_over_a_transparent_backdrop() {
      let mut page = page(1, 1, vec![200, 40, 40, 255]);
      page.layers[0].blend = BlendMode::Multiply;
      assert_eq!(page.composite_rgba().unwrap(), [200, 40, 40, 255]);
    }

    #[test]
    fn multiply_and_screen_blend_with_an_opaque_backdrop() {
      let mut page = page(1, 1, vec![128, 128, 128, 255]);
      page.import_rgba_layer(1, 1, vec![128, 255, 0, 255], "Top".into()).unwrap();
      page.layers[1].blend = BlendMode::Multiply;
      assert_eq!(page.composite_rgba().unwrap(), [64, 128, 0, 255]);
      page.layers[1].blend = BlendMode::Screen;
      assert_eq!(page.composite_rgba().unwrap(), [192, 255, 128, 255]);
    }

    #[test]
    fn half_opacity_mixes_with_the_backdrop() {
      let mut page = page(1, 1, vec![255; 4]);
      page.import_rgba_layer(1, 1, vec![255, 0, 0, 255], "Red".into()).unwrap();
      page.layers[1].opacity = 0.5;
      assert_eq!(page.composite_rgba().unwrap(), [255, 128, 128, 255]);
    }

    #[test]
    fn levels_preview_is_nondestructive_and_reset_restores_the_composite() {
      let mut page = page(1, 1, vec![64, 128, 192, 128]);
      let original = page.layers[0].pixels.clone();
      page.levels = Levels { input_black: 64, input_white: 192, ..Levels::default() };
      assert_eq!(page.composite_rgba().unwrap(), [0, 128, 255, 128]);
      assert_eq!(page.layers[0].pixels, original);
      page.levels_preview = false;
      assert_eq!(page.composite_rgba().unwrap(), [64, 128, 192, 128]);
      page.reset_levels();
      assert_eq!(page.composite_rgba().unwrap(), [64, 128, 192, 128]);
    }

    #[test]
    fn png_export_encodes_the_same_levels_result_as_the_preview() {
      let mut page = page(701, 411, [64, 128, 192, 128].repeat(701 * 411));
      page.levels.gamma = 2.0;
      let preview = page.composite_rgba().unwrap();
      let exported = image::load_from_memory(&page.composite_png().unwrap()).unwrap().to_rgba8();
      assert_eq!(exported.into_raw(), preview);
    }
  }
}

#[cfg(test)]
mod layer_editing_tests {
  use super::*;

  fn page(width: u32, height: u32, pixels: Vec<u8>) -> EditImagePage {
    let mut page = EditImagePage::default();
    page.open_rgba_document(width, height, pixels, "Source".into()).unwrap();
    page.undo.clear();
    page
  }

  #[test]
  fn mask_coverage_multiplies_original_alpha_without_touching_rgba() {
    let mut page = page(3, 1, vec![240, 30, 10, 128, 20, 230, 40, 255, 15, 35, 245, 85]);
    let original = page.layers[0].pixels.clone();
    page.add_mask_to_active(255).unwrap();
    page.layers[0].mask.as_mut().unwrap().pixels = Arc::new(vec![0, 128, 255]);
    assert_eq!(page.composite_rgba().unwrap(), [240, 30, 10, 0, 20, 230, 40, 128, 15, 35, 245, 85]);
    assert!(Arc::ptr_eq(&page.layers[0].pixels, &original));
    page.layers[0].mask.as_mut().unwrap().enabled = false;
    assert_eq!(page.composite_rgba().unwrap().as_slice(), original.as_slice());
    page.layers[0].mask.as_mut().unwrap().enabled = true;
    page.layers[0].opacity = 0.5;
    assert_eq!(page.composite_rgba().unwrap()[7], 64);
  }

  #[test]
  fn reveal_all_mask_keeps_neutral_png_rgba_exact() {
    let pixels = vec![33, 44, 55, 0, 255, 128, 17, 128, 5, 6, 7, 255];
    let mut page = page(3, 1, pixels.clone());
    page.add_mask_to_active(255).unwrap();
    let exported = image::load_from_memory(&page.composite_png().unwrap()).unwrap().to_rgba8();
    assert_eq!(exported.into_raw(), pixels);
  }

  #[test]
  fn mask_brush_and_eraser_preserve_source_pixels_and_undo() {
    let mut page = page(5, 3, [200, 100, 50, 255].repeat(15));
    let original = page.layers[0].pixels.clone();
    page.add_mask_to_active(255).unwrap();
    page.undo.clear();
    page.mask_value = 0;
    page.brush_radius = 1.0;
    page.apply_tool_at(pos2(0.5, 0.5), None).unwrap();
    assert_eq!(page.layers[0].mask.as_ref().unwrap().pixels[7], 0);
    assert_eq!(page.composite_rgba().unwrap()[7 * 4 + 3], 0);
    assert!(Arc::ptr_eq(&page.layers[0].pixels, &original));
    assert!(page.undo());
    assert_eq!(page.layers[0].mask.as_ref().unwrap().pixels[7], 255);
    assert!(page.redo());
    page.tool = DrawTool::Eraser;
    page.apply_tool_at(pos2(0.5, 0.5), None).unwrap();
    assert_eq!(page.layers[0].mask.as_ref().unwrap().pixels[7], 255);
    assert!(page.undo());
    assert_eq!(page.layers[0].mask.as_ref().unwrap().pixels[7], 0);
  }

  #[test]
  fn tiny_mask_and_rgba_cow_boundaries_count_unique_current_buffers() {
    let mut page = page(1, 1, vec![200, 100, 20, 255]);
    page.add_mask_to_active(255).unwrap();
    page.layers.push(page.layers[0].clone());
    page.active_layer = 1;
    assert_eq!(current_pixel_bytes(&page.layers), 5);
    assert!(!buffer_copy_fits(&page.layers, 1, true, 5));
    assert!(buffer_copy_fits(&page.layers, 1, true, 6));
    assert!(!pixel_copy_fits(&page.layers, 1, 8));
    assert!(pixel_copy_fits(&page.layers, 1, 9));
    page.invert_active_mask().unwrap();
    assert_eq!(current_pixel_bytes(&page.layers), 6);
    assert_eq!(page.layers[0].mask.as_ref().unwrap().pixels.as_slice(), &[255]);
    assert_eq!(page.layers[1].mask.as_ref().unwrap().pixels.as_slice(), &[0]);
    assert!(Arc::ptr_eq(&page.layers[0].pixels, &page.layers[1].pixels));
  }

  #[test]
  fn malformed_mask_and_invalid_transform_are_rejected() {
    let mut page = page(2, 1, vec![255; 8]);
    page.layers[0].mask = Some(LayerMask { enabled: true, pixels: Arc::new(vec![255]) });
    assert!(validate_layer(&page.layers[0]).is_err());
    assert!(page.composite_rgba().is_err());
    page.layers[0].mask = None;
    for transform in [LayerTransform { scale: [0.0, 1.0], ..LayerTransform::default() }, LayerTransform { offset: [f64::INFINITY, 0.0], ..LayerTransform::default() }, LayerTransform { rotation_deg: f64::NAN, ..LayerTransform::default() }, LayerTransform { scale: [1e7, 1.0], ..LayerTransform::default() }] {
      page.layers[0].transform = transform;
      assert!(page.composite_rgba().is_err());
    }
    page.layers[0].transform = LayerTransform::default();
    assert_eq!(sample_layer(&page.layers[0], [f64::MAX, -f64::MAX]), [0.0; 4]);
    assert_eq!(sample_layer(&page.layers[0], [f64::NAN, 0.0]), [0.0; 4]);
  }

  #[test]
  fn affine_forward_inverse_roundtrip_includes_flip_and_nonuniform_scale() {
    let transform = LayerTransform { offset: [13.25, -7.5], rotation_deg: 37.0, scale: [2.5, 0.75], flip_x: true, flip_y: true };
    for point in [[0.5, 0.5], [123.5, 41.5], [400.0, 300.0]] {
      let document = transform.local_to_document(point, 401, 301).unwrap();
      let recovered = transform.document_to_local(document, 401, 301).unwrap();
      assert!((recovered[0] - point[0]).abs() < 1e-9);
      assert!((recovered[1] - point[1]).abs() < 1e-9);
    }
  }

  #[test]
  fn horizontal_flip_and_clockwise_quarter_turn_render_expected_pixels() {
    let red = [255, 0, 0, 255];
    let green = [0, 255, 0, 255];
    let blue = [0, 0, 255, 255];
    let yellow = [255, 255, 0, 255];
    let mut flip = page(3, 1, [red, green, blue].concat());
    let original = flip.layers[0].pixels.clone();
    flip.layers[0].transform.flip_x = true;
    assert_eq!(flip.composite_rgba().unwrap(), [blue, green, red].concat());
    assert_eq!(flip.layers[0].pixels, original);
    let mut rotate = page(2, 2, [red, green, blue, yellow].concat());
    rotate.layers[0].transform.rotation_deg = 90.0;
    assert_eq!(rotate.composite_rgba().unwrap(), [blue, red, yellow, green].concat());
  }

  #[test]
  fn transform_clips_only_composite_and_does_not_resize_or_destroy_layer() {
    let original = [255, 0, 0, 255, 0, 0, 255, 255].to_vec();
    let mut page = page(2, 1, original.clone());
    page.layers[0].transform.offset = [1.0, 0.0];
    assert_eq!((page.width, page.height), (2, 1));
    assert_eq!(page.composite_rgba().unwrap(), [0, 0, 0, 0, 255, 0, 0, 255]);
    assert_eq!(page.layers[0].pixels.as_ref(), &original);
    page.layers[0].transform = LayerTransform::default();
    assert_eq!(page.composite_rgba().unwrap(), original);
  }

  #[test]
  fn transformed_brush_and_mask_use_inverse_layer_coordinates() {
    let mut page = page(4, 3, vec![0; 4 * 3 * 4]);
    page.width = 10;
    page.height = 6;
    page.layers[0].transform = LayerTransform { offset: [2.5, 1.0], scale: [2.0, 1.0], flip_x: true, ..LayerTransform::default() };
    page.brush_radius = 1.0;
    page.brush_color = [230, 70, 20, 255];
    page.apply_tool_at(pos2(0.55, 2.5 / 6.0), None).unwrap();
    assert_eq!(&page.layers[0].pixels[(1 * 4 + 1) * 4..(1 * 4 + 2) * 4], &[230, 70, 20, 255]);
    assert_eq!(page.layers[0].pixels.chunks_exact(4).filter(|pixel| pixel[3] != 0).count(), 1);
    page.add_mask_to_active(255).unwrap();
    page.mask_value = 0;
    page.apply_tool_at(pos2(0.55, 2.5 / 6.0), None).unwrap();
    assert_eq!(page.layers[0].mask.as_ref().unwrap().pixels[5], 0);
    page.tool = DrawTool::ColorPicker;
    page.mask_value = 255;
    page.apply_tool_at(pos2(0.55, 2.5 / 6.0), None).unwrap();
    assert_eq!(page.mask_value, 0);
  }

  #[test]
  fn move_tool_changes_only_metadata_and_undo_restores_mask_transform() {
    let mut page = page(100, 50, vec![255; 100 * 50 * 4]);
    page.add_mask_to_active(255).unwrap();
    let pixels = page.layers[0].pixels.clone();
    let mask = page.layers[0].mask.as_ref().unwrap().pixels.clone();
    page.tool = DrawTool::Move;
    page.apply_tool_at(pos2(0.3, 0.4), Some(pos2(0.2, 0.2))).unwrap();
    assert!((page.layers[0].transform.offset[0] - 10.0).abs() < 1e-5);
    assert!((page.layers[0].transform.offset[1] - 10.0).abs() < 1e-5);
    assert!(Arc::ptr_eq(&page.layers[0].pixels, &pixels));
    assert!(Arc::ptr_eq(&page.layers[0].mask.as_ref().unwrap().pixels, &mask));
    assert!(page.undo());
    assert_eq!(page.layers[0].transform, LayerTransform::default());
  }

  #[test]
  fn rename_order_mask_remove_and_transform_are_session_undoable() {
    let mut page = page(1, 1, vec![255, 0, 0, 255]);
    page.import_rgba_layer(1, 1, vec![0, 0, 255, 255], "Blue".into()).unwrap();
    assert_eq!(page.composite_rgba().unwrap(), [0, 0, 255, 255]);
    page.move_active_layer(0).unwrap();
    assert_eq!((page.active_layer, page.layers[0].name.as_str()), (0, "Blue"));
    assert_eq!(page.composite_rgba().unwrap(), [255, 0, 0, 255]);
    assert!(page.undo());
    assert_eq!(page.active_layer, 1);
    page.rename_active_layer("Sky".into()).unwrap();
    assert_eq!(page.layers[1].name, "Sky");
    assert!(page.undo());
    assert_eq!(page.layers[1].name, "Blue");
    page.add_mask_to_active(128).unwrap();
    page.remove_mask_from_active().unwrap();
    assert!(page.layers[1].mask.is_none());
    assert!(page.undo());
    assert_eq!(page.layers[1].mask.as_ref().unwrap().pixels.as_slice(), &[128]);
    page.begin_edit();
    page.layers[1].transform.flip_y = true;
    page.changed();
    page.finish_edit();
    assert!(page.undo());
    assert!(!page.layers[1].transform.flip_y);
    assert!(page.redo());
    assert!(page.layers[1].transform.flip_y);
  }

  #[test]
  fn curves_are_nondestructive_previewable_and_undoable_after_levels() {
    let original = vec![100, 150, 200, 128];
    let mut page = page(1, 1, original.clone());
    page.begin_edit();
    page.curves.channels[0].points[1].output = 128;
    page.changed();
    page.finish_edit();
    assert_eq!(page.composite_rgba().unwrap(), [50, 75, 100, 128]);
    assert_eq!(page.layers[0].pixels.as_ref(), &original);
    page.curves_preview = false;
    assert_eq!(page.composite_rgba().unwrap(), original);
    page.curves_preview = true;
    assert!(page.undo());
    assert_eq!(page.composite_rgba().unwrap(), original);
    assert!(page.redo());
    assert_eq!(page.composite_rgba().unwrap(), [50, 75, 100, 128]);
  }
}

#[cfg(test)]
mod project_integration_tests {
  use super::*;

  #[test]
  fn layered_project_roundtrip_preserves_raw_pixels_masks_transforms_and_curves() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("layered.acraft");
    let mut source = EditImagePage::default();
    source.open_rgba_document(3, 2, vec![17, 33, 55, 0, 22, 44, 66, 128, 44, 88, 132, 255, 8, 9, 10, 0, 99, 88, 77, 128, 4, 5, 6, 255], "Original α".into()).unwrap();
    source.import_rgba_layer(2, 1, vec![200, 100, 50, 255, 3, 4, 5, 0], "Overlay".into()).unwrap();
    source.add_mask_to_active(128).unwrap();
    source.layers[1].transform = LayerTransform { offset: [0.5, 0.5], rotation_deg: 25.0, scale: [0.75, 1.25], flip_x: true, flip_y: false };
    source.layers[1].blend = BlendMode::Screen;
    source.layers[1].opacity = 0.7;
    source.exposure = 0.2;
    source.levels.gamma = 1.2;
    source.curves.channels[0].points[1].output = 210;
    let before = source.composite_rgba().unwrap();
    source.save_project(&path).unwrap();
    let loaded = ImageProject::load(&path).unwrap();
    assert_eq!(loaded.layers, source.layers);
    assert_eq!(loaded.curves, source.curves);
    assert_eq!(loaded.levels, source.levels);
    assert_eq!((loaded.width, loaded.height, loaded.active_layer), (3, 2, 1));
    let mut reopened = EditImagePage::default();
    reopened.replace_project(loaded).unwrap();
    assert_eq!(reopened.composite_rgba().unwrap(), before);
    assert_eq!(reopened.layers[0].pixels.as_slice(), source.layers[0].pixels.as_slice(), "hidden RGB and alpha are stored, not flattened");
  }

  #[test]
  fn project_replacement_is_undoable_and_invalid_projects_leave_document_unchanged() {
    let mut page = EditImagePage::default();
    page.open_rgba_document(1, 1, vec![255, 0, 0, 255], "Before".into()).unwrap();
    let before = page.snapshot();
    let mut other = EditImagePage::default();
    other.open_rgba_document(5, 3, vec![128; 5 * 3 * 4], "After".into()).unwrap();
    other.add_mask_to_active(64).unwrap();
    other.layers[0].transform.flip_x = true;
    page.replace_project(other.to_project()).unwrap();
    assert_eq!((page.width, page.height), (5, 3));
    assert!(page.undo());
    assert!(page.snapshot() == before);
    assert!(page.redo());
    assert_eq!(page.layers[0].mask.as_ref().unwrap().pixels.len(), 15);
    let valid = page.snapshot();
    let mut invalid = other.to_project();
    invalid.active_layer = usize::MAX;
    assert!(page.replace_project(invalid).is_err());
    assert!(page.snapshot() == valid);
    let mut invalid_mask = other.to_project();
    invalid_mask.layers[0].mask.as_mut().unwrap().pixels = Arc::new(vec![255]);
    assert!(page.replace_project(invalid_mask).is_err());
    assert!(page.snapshot() == valid);
  }
}

#[cfg(test)]
mod project_limit_tests {
  use super::*;

  #[test]
  fn layer_creation_and_names_remain_within_the_project_format_limits() {
    let mut page = EditImagePage::default();
    page.open_rgba_document(1, 1, vec![255; 4], "Source".into()).unwrap();
    page.layers = vec![page.layers[0].clone(); MAX_PROJECT_LAYERS];
    page.active_layer = 0;
    let before = page.snapshot();
    assert!(page.add_blank_layer().is_err());
    assert!(page.duplicate_active_layer().is_err());
    assert!(page.import_rgba_layer(1, 1, vec![0; 4], "Extra".into()).is_err());
    assert!(page.rename_active_layer("x".repeat(4097)).is_err());
    assert!(page.snapshot() == before);
    page.rename_active_layer("한".repeat(1000)).unwrap();
    assert!(page.to_project().validate().is_ok());
  }

  #[test]
  fn failed_file_load_is_never_published_into_the_editor() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("broken.acraft");
    fs::write(&path, b"not an image project").unwrap();
    let mut page = EditImagePage::default();
    page.open_rgba_document(1, 1, vec![20, 40, 60, 80], "Keep".into()).unwrap();
    let before = page.snapshot();
    let result = ImageProject::load(&path).and_then(|project| page.replace_project(project));
    assert!(result.is_err());
    assert!(page.snapshot() == before);
  }
}

#[cfg(test)]
mod continuous_brush_tests {
  use super::*;

  fn page(width: u32, height: u32, pixels: Vec<u8>) -> EditImagePage {
    let mut page = EditImagePage::default();
    page.open_rgba_document(width, height, pixels, "Source".into()).unwrap();
    page.undo.clear();
    page
  }

  fn primary(point: Pos2, pressed: bool) -> egui::Event {
    egui::Event::PointerButton { pos: point, button: egui::PointerButton::Primary, pressed, modifiers: egui::Modifiers::NONE }
  }

  fn brush_events(page: &mut EditImagePage, events: &[egui::Event], rect: Rect) {
    for event in events {
      let transition = page.brush_gesture.event(event, true, rect);
      page.apply_brush_transition(transition, rect);
    }
  }

  #[test]
  fn capsule_row_candidates_cover_bruteforce_for_affine_layers() {
    for angle in [-179.0, -90.0, -37.0, 0.0, 23.0, 90.0, 179.0] {
      for scale in [[1.0, 1.0], [0.01, 3.0], [3.0, 0.01], [1e-6, 1e6], [1e6, 1e-6]] {
        for offset in [[0.0, 0.0], [-12.0, 9.0], [1e12, -1e12]] {
          let mapping = LayerMapping::new(&LayerTransform { offset, rotation_deg: angle, scale, flip_x: true, flip_y: angle < 0.0 }, 31, 19).unwrap();
          for (local_a, local_b) in [([2.5, 3.5], [27.5, 15.5]), ([15.5, 9.5], [15.5, 9.5]), ([30.5, 0.5], [0.5, 18.5])] {
            let (start, end) = (mapping.to_document(local_a), mapping.to_document(local_b));
            for radius in [1.0, 2.75, 500.0] {
              for y in 0..19 {
                let candidate = capsule_row_span(mapping, y, 31, start, end, radius);
                for x in 0..31 {
                  let point = mapping.to_document([x as f64 + 0.5, y as f64 + 0.5]);
                  if segment_distance(point, start, end) < radius {
                    assert!(candidate.is_some_and(|(left, right)| left <= x && x < right), "candidate dropped ({x},{y}), angle={angle} scale={scale:?} offset={offset:?} radius={radius}");
                  }
                }
              }
            }
          }
        }
      }
    }
  }

  #[test]
  fn thin_long_diagonal_capsule_has_bounded_row_candidates() {
    let mapping = LayerMapping::new(&LayerTransform::default(), 4097, 4097).unwrap();
    let candidates: u64 = (0..4097).map(|y| capsule_row_span(mapping, y, 4097, [0.5, 0.5], [4096.5, 4096.5], 1.0).map_or(0, |(left, right)| (right - left) as u64)).sum();
    assert!(candidates < 4097 * 12, "a thin diagonal must not scan its full square bbox");
    assert!(capsule_row_span(mapping, 0, 4097, [f64::NAN, 0.0], [1.0, 1.0], 1.0).is_none());
    assert!(capsule_row_span(mapping, 0, 0, [0.0, 0.0], [1.0, 1.0], 1.0).is_none());
    assert!(capsule_row_span(mapping, 0, 4097, [0.0, 0.0], [1.0, 1.0], f64::INFINITY).is_none());
  }

  #[test]
  fn continuous_brush_and_eraser_fill_long_pixel_center_segments() {
    for (width, height, start, end) in [(1025, 3, pos2(0.0, 0.5), pos2(1.0, 0.5)), (3, 1025, pos2(0.5, 0.0), pos2(0.5, 1.0)), (513, 513, pos2(0.0, 0.0), pos2(1.0, 1.0))] {
      let mut page = page(width, height, vec![0; width as usize * height as usize * 4]);
      page.brush_radius = 1.0;
      page.brush_color = [30, 140, 230, 255];
      page.apply_tool_at(end, Some(start)).unwrap();
      let count = width.max(height);
      for step in 0..count {
        let x = if width == 3 { 1 } else { step };
        let y = if height == 3 { 1 } else { step };
        let index = (y as usize * width as usize + x as usize) * 4;
        assert_eq!(&page.layers[0].pixels[index..index + 4], &[30, 140, 230, 255], "gap at {x},{y}");
      }
      let painted = page.layers[0].pixels.clone();
      page.tool = DrawTool::Eraser;
      page.apply_tool_at(start, Some(end)).unwrap();
      for step in 0..count {
        let x = if width == 3 { 1 } else { step };
        let y = if height == 3 { 1 } else { step };
        assert_eq!(page.layers[0].pixels[(y as usize * width as usize + x as usize) * 4 + 3], 0, "eraser center gap");
      }
      assert!(page.layers[0].pixels.chunks_exact(4).zip(painted.chunks_exact(4)).all(|(after, before)| after[3] <= before[3]), "soft eraser fringe can retain alpha but never adds it");
    }
  }

  #[test]
  fn transformed_rgba_and_masks_match_full_capsule_reference() {
    let (width, height) = (31, 19);
    let mut page = page(width, height, vec![0; width as usize * height as usize * 4]);
    page.width = 83;
    page.height = 57;
    page.layers[0].transform = LayerTransform { offset: [23.0, 14.0], rotation_deg: 27.0, scale: [1.4, 0.8], flip_x: true, flip_y: false };
    page.brush_radius = 2.75;
    page.brush_hardness = 0.65;
    page.brush_color = [80, 160, 240, 128];
    let (start_uv, end_uv) = (pos2(0.2, 0.25), pos2(0.75, 0.65));
    let center = |uv| { let (x, y) = document_pixel_from_uv(83, 57, uv).unwrap(); [x as f64 + 0.5, y as f64 + 0.5] };
    let (start, end) = (center(start_uv), center(end_uv));
    let mapping = LayerMapping::new(&page.layers[0].transform, width, height).unwrap();
    page.apply_tool_at(end_uv, Some(start_uv)).unwrap();
    let mut expected_alpha = Vec::new();
    for y in 0..height {
      for x in 0..width {
        let distance = segment_distance(mapping.to_document([x as f64 + 0.5, y as f64 + 0.5]), start, end);
        let coverage = if distance < 2.75 { (1.0 - distance / 2.75).powf((1.0 - 0.65_f32 as f64).max(0.15)) * 128.0 / 255.0 } else { 0.0 };
        let alpha = (coverage * 255.0).round() as u8;
        expected_alpha.push(alpha);
        let index = (y as usize * width as usize + x as usize) * 4;
        assert_eq!(page.layers[0].pixels[index + 3], alpha, "transformed pixel {x},{y}");
        if alpha > 0 { assert_eq!(&page.layers[0].pixels[index..index + 3], &[80, 160, 240]); }
      }
    }
    let source = page.layers[0].pixels.clone();
    page.add_mask_to_active(255).unwrap();
    page.mask_value = 0;
    page.apply_tool_at(end_uv, Some(start_uv)).unwrap();
    let mask = page.layers[0].mask.as_ref().unwrap();
    for (actual, alpha) in mask.pixels.iter().zip(&expected_alpha) {
      assert!((*actual as i16 - (255 - *alpha) as i16).abs() <= 1, "mask uses the same capsule coverage");
    }
    assert!(Arc::ptr_eq(&source, &page.layers[0].pixels), "mask stroke must not copy or alter RGBA");
    assert!(page.undo());
    assert!(page.layers[0].mask.as_ref().unwrap().pixels.iter().all(|value| *value == 255));
    assert!(page.redo());
    let png = page.composite_png().unwrap();
    let decoded = image::load_from_memory(&png).unwrap().to_rgba8();
    assert_eq!(decoded.dimensions(), (83, 57));
    assert_eq!(decoded.into_raw(), page.composite_rgba().unwrap());
  }

  #[test]
  fn one_frame_press_and_release_keeps_both_endpoints_and_one_undo() {
    let mut page = page(101, 7, vec![0; 101 * 7 * 4]);
    page.brush_radius = 1.0;
    let original = page.layers[0].pixels.clone();
    let rect = Rect::from_min_size(Pos2::ZERO, vec2(101.0, 7.0));
    let undo_before = page.undo.len();
    brush_events(&mut page, &[primary(pos2(1.5, 3.5), true), egui::Event::PointerMoved(pos2(35.5, 3.5)), primary(pos2(99.5, 3.5), false)], rect);
    for x in 1..=99 { assert_eq!(page.layers[0].pixels[(3 * 101 + x) * 4 + 3], 255, "press/release endpoint omitted at {x}"); }
    assert_eq!(page.undo.len(), undo_before + 1);
    assert!(!page.brush_gesture.active);
    let painted = page.layers[0].pixels.clone();
    assert!(page.undo());
    assert_eq!(*page.layers[0].pixels, *original);
    assert!(page.redo());
    assert_eq!(*page.layers[0].pixels, *painted);
  }

  #[test]
  fn outside_samples_reentry_and_separate_clicks_never_bridge() {
    let mut page = page(101, 7, vec![0; 101 * 7 * 4]);
    page.brush_radius = 1.0;
    let rect = Rect::from_min_size(Pos2::ZERO, vec2(101.0, 7.0));
    brush_events(&mut page, &[
      primary(pos2(5.5, 3.5), true), egui::Event::PointerMoved(pos2(5.5, -4.0)),
      egui::Event::PointerMoved(pos2(95.5, 3.5)), primary(pos2(95.5, 3.5), false),
      primary(pos2(20.5, 3.5), true), primary(pos2(20.5, 3.5), false),
    ], rect);
    for x in 0..101 {
      assert_eq!(page.layers[0].pixels[(3 * 101 + x) * 4 + 3] > 0, [5, 20, 95].contains(&x), "unexpected bridging at {x}");
    }
    assert_eq!(page.undo.len(), 2);
    let before = page.snapshot();
    assert!(page.apply_tool_at(pos2(-0.1, 0.5), None).is_err());
    assert!(page.apply_tool_at(pos2(0.5, 0.5), Some(pos2(2.0, 0.5))).is_err());
    assert!(page.snapshot() == before);
  }

  #[test]
  fn brush_input_cancellation_topmost_and_viewport_edges() {
    let rect = Rect::from_min_size(pos2(20.0, 10.0), vec2(40.0, 30.0));
    let point = pos2(30.0, 20.0);
    let mut gesture = BrushGesture::default();
    assert!(!gesture.event(&primary(point, true), false, rect).begin, "covered canvas cannot claim press");
    assert!(gesture.event(&primary(point, true), true, rect).begin);
    assert!(gesture.event(&egui::Event::PointerMoved(pos2(61.0, 20.0)), true, rect).segment.is_none());
    assert_eq!(gesture.event(&egui::Event::PointerMoved(pos2(50.0, 20.0)), true, rect).segment, Some((None, pos2(50.0, 20.0))));
    assert!(gesture.event(&egui::Event::PointerMoved(point), false, rect).segment.is_none());
    assert_eq!(gesture.event(&egui::Event::PointerMoved(point), true, rect).segment, Some((None, point)));
    for cancel in [egui::Event::PointerGone, egui::Event::WindowFocused(false),
      egui::Event::PointerButton { pos: point, button: egui::PointerButton::Middle, pressed: true, modifiers: egui::Modifiers::NONE },
      egui::Event::Key { key: egui::Key::Space, physical_key: None, pressed: true, repeat: false, modifiers: egui::Modifiers::NONE }] {
      gesture.event(&primary(point, true), true, rect);
      assert!(gesture.event(&cancel, true, rect).finish_after);
      assert!(!gesture.active);
      assert!(gesture.event(&egui::Event::PointerMoved(pos2(50.0, 20.0)), true, rect).segment.is_none(), "cancel requires a fresh press");
    }
    gesture.event(&primary(pos2(19.0, 20.0), true), true, rect);
    assert!(gesture.event(&egui::Event::PointerMoved(point), true, rect).segment.is_none(), "a press outside cannot start on entry");
  }

  #[test]
  fn stationary_brush_is_a_single_dab_and_new_edit_clears_redo() {
    let mut page = page(5, 5, vec![0; 5 * 5 * 4]);
    page.brush_radius = 1.0;
    page.brush_color = [200, 60, 20, 128];
    let rect = Rect::from_min_size(Pos2::ZERO, vec2(5.0, 5.0));
    let point = pos2(2.5, 2.5);
    brush_events(&mut page, &[primary(point, true)], rect);
    let dab = page.layers[0].pixels.clone();
    brush_events(&mut page, &[egui::Event::PointerMoved(point), egui::Event::PointerMoved(point), primary(point, false)], rect);
    assert_eq!(*page.layers[0].pixels, *dab, "stationary input and release must not add opacity");
    assert_eq!(page.layers[0].pixels[(2 * 5 + 2) * 4 + 3], 128);
    assert!(page.undo());
    assert!(!page.redo.is_empty());
    brush_events(&mut page, &[primary(pos2(0.5, 0.5), true), primary(pos2(0.5, 0.5), false)], rect);
    assert!(page.redo.is_empty());
  }


  fn canvas_gate_frame(ctx: &egui::Context, page: &mut EditImagePage, events: Vec<egui::Event>, disabled: bool, other_widget: bool, document_modal: bool, panning: bool) -> bool {
    let mut allowed = false;
    let mut output = ctx.run_ui(egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(300.0, 200.0))), events, focused: true, ..Default::default() }, |_| {
      // This test container must not claim clicks intended for its canvas child.
      egui::Area::new(egui::Id::new("brush_gate_canvas")).sense(Sense::hover()).fixed_pos(Pos2::ZERO).order(egui::Order::Background).show(ctx, |ui| {
        if disabled { ui.disable(); }
        let canvas = Rect::from_min_size(pos2(20.0, 20.0), vec2(101.0, 7.0));
        let response = ui.allocate_rect(canvas, Sense::click_and_drag());
        if other_widget {
          ui.interact(canvas, egui::Id::new("covering_same_layer_widget"), Sense::click_and_drag());
        }
        allowed = brush_canvas_input_allowed(ui, &response, canvas, document_modal, panning);
        if allowed {
          let events = ctx.input(|input| input.events.clone());
          for event in &events {
            let topmost = match event {
              egui::Event::PointerMoved(point) | egui::Event::PointerButton { pos: point, .. } => ctx.layer_id_at(*point) == Some(response.layer_id),
              _ => true,
            };
            let transition = page.brush_gesture.event(event, topmost, canvas.intersect(ui.clip_rect()));
            page.apply_brush_transition(transition, canvas);
          }
        } else {
          page.finish_brush_gesture();
        }
      });
    });
    // This headless input test intentionally has no texture painter.
    output.textures_delta.clear();
    allowed
  }

  #[test]
  fn egui_canvas_gate_accepts_batched_press_release_even_without_final_hover() {
    for release in [pos2(19.75, 23.5), pos2(119.5, 23.5)] {
      let ctx = egui::Context::default();
      let mut page = page(101, 7, vec![0; 101 * 7 * 4]);
      page.brush_radius = 1.0;
      for _ in 0..2 { canvas_gate_frame(&ctx, &mut page, vec![], false, false, false, false); }
      assert!(canvas_gate_frame(&ctx, &mut page, vec![primary(pos2(20.25, 23.5), true), primary(release, false)], false, false, false, false));
      assert_eq!(page.layers[0].pixels[(3 * 101) * 4 + 3], 255, "short outside release must keep the initial dab");
      if release.x > 100.0 {
        for x in 0..100 { assert_eq!(page.layers[0].pixels[(3 * 101 + x) * 4 + 3], 255, "batched endpoint gap at {x}"); }
      } else {
        assert_eq!(page.layers[0].pixels.chunks_exact(4).filter(|pixel| pixel[3] != 0).count(), 1, "outside release cannot clamp/paint a segment on the edge");
      }
      assert_eq!(page.undo.len(), 1);
    }
  }

  #[test]
  fn egui_canvas_gate_blocks_disabled_popup_modal_pan_and_other_widget() {
    for mode in 0..5 {
      let ctx = egui::Context::default();
      let mut page = page(101, 7, vec![0; 101 * 7 * 4]);
      page.brush_radius = 1.0;
      let disabled = mode == 0;
      let other_widget = mode == 1;
      for _ in 0..2 { canvas_gate_frame(&ctx, &mut page, vec![], disabled, other_widget, mode == 3, mode == 4); }
      if mode == 2 { egui::Popup::open_id(&ctx, egui::Id::new("brush_covering_popup")); }
      assert!(!canvas_gate_frame(&ctx, &mut page, vec![primary(pos2(21.5, 23.5), true), primary(pos2(21.5, 23.5), false)], disabled, other_widget, mode == 3, mode == 4), "input gate mode {mode}");
      assert!(page.layers[0].pixels.iter().all(|value| *value == 0), "blocked UI must not paint underneath, mode {mode}");
      assert!(page.undo.is_empty());
    }
  }

}
