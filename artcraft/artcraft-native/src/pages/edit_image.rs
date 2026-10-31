//! Edit Image (local preview): a small multi-layer canvas with a brush, an eraser, liquify brushes,
//! filters and tone adjustments. Everything runs on this computer; there is no AI editing here.
//! The composite can be saved as a PNG or attached to Create Image / Create Video as a reference.

use std::fs;

use egui::{Color32, Pos2, Rect, Sense, Stroke, TextureHandle, TextureOptions, Ui, Vec2, pos2, vec2};
use rfd::FileDialog;

use crate::overlays::lightbox::fit_inside;
use crate::pages::common::{self, AppRequest, Env};
use crate::prompt_box::refs::{ImageSlot, RefKind};
use crate::theme;
use crate::ui::icons::Icon;
use crate::ui::widgets::{self, Kind};

/// The canvas size in pixels. Imported images are scaled to fit inside it, keeping their aspect.
pub const CANVAS_W: u32 = 320;
pub const CANVAS_H: u32 = 200;

pub struct EditImagePage {
  pub layers: Vec<CanvasLayer>,
  pub active_layer: usize,
  pub tool: DrawTool,
  pub brush_radius: f32,
  pub brush_hardness: f32,
  pub brush_color: [u8; 4],
  /// Edge refinement: color sharpening, plus the contrast and shift of soft alpha edges.
  pub sharpen: f32,
  pub edge_contrast: f32,
  pub edge_shift: f32,
  pub filter: FilterPreset,
  pub filter_intensity: f32,
  pub exposure: f32,
  pub contrast: f32,
  pub saturation: f32,
  texture: Option<TextureHandle>,
  dirty: bool,
  last_drag_uv: Option<Pos2>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DrawTool {
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

#[derive(Clone, Debug)]
pub struct CanvasLayer {
  pub name: String,
  pub visible: bool,
  pub opacity: f32,
  pub blend: BlendMode,
  pub width: u32,
  pub height: u32,
  pub pixels: Vec<u8>,
}

impl Default for EditImagePage {
  fn default() -> Self {
    Self {
      layers: vec![CanvasLayer::new_solid("Background", CANVAS_W, CANVAS_H, [255, 255, 255, 255]), CanvasLayer::new_solid("Layer 1", CANVAS_W, CANVAS_H, [0, 0, 0, 0])],
      active_layer: 1,
      tool: DrawTool::Brush,
      brush_radius: 10.0,
      brush_hardness: 0.75,
      brush_color: [45, 129, 255, 255],
      sharpen: 0.55,
      edge_contrast: 0.40,
      edge_shift: 0.0,
      filter: FilterPreset::EdgeGlow,
      filter_intensity: 0.65,
      exposure: 0.0,
      contrast: 0.0,
      saturation: 0.0,
      texture: None,
      dirty: true,
      last_drag_uv: None,
    }
  }
}

impl EditImagePage {
  /// Adds an RGBA buffer from another tool (or a file) as a new layer, fitted inside the canvas.
  pub fn import_rgba_layer(&mut self, width: u32, height: u32, pixels: Vec<u8>, name: String) {
    let fitted = fit_rgba(&pixels, width, height, CANVAS_W, CANVAS_H);
    self.layers.push(CanvasLayer { name, visible: true, opacity: 1.0, blend: BlendMode::Normal, width: CANVAS_W, height: CANVAS_H, pixels: fitted });
    self.active_layer = self.layers.len() - 1;
    self.dirty = true;
  }

  /// Composites the visible layers (bottom to top) into one RGBA8 buffer (`CANVAS_W x CANVAS_H`),
  /// then applies the tone adjustments.
  pub fn composite_rgba(&self) -> Vec<u8> {
    let len = rgba_len(CANVAS_W, CANVAS_H);
    let mut out = vec![0u8; len];
    for layer in &self.layers {
      if !layer.visible || layer.opacity <= 0.0 || layer.pixels.len() != len {
        continue;
      }
      let opacity = layer.opacity.clamp(0.0, 1.0);
      for i in (0..len).step_by(4) {
        let src_a = (layer.pixels[i + 3] as f32 / 255.0) * opacity;
        if src_a <= 0.0 {
          continue;
        }
        let dst_a = out[i + 3] as f32 / 255.0;
        let out_a = src_a + dst_a * (1.0 - src_a);
        for c in 0..3 {
          let backdrop = out[i + c];
          let source = layer.pixels[i + c];
          // A blend mode only applies where there is a backdrop: (1 - αb)·Cs + αb·B(Cb, Cs).
          let blended = (1.0 - dst_a) * source as f32 + dst_a * layer.blend.blend_channel(backdrop, source) as f32;
          out[i + c] = ((blended * src_a + backdrop as f32 * dst_a * (1.0 - src_a)) / out_a).round() as u8;
        }
        out[i + 3] = (out_a * 255.0).round() as u8;
      }
    }
    apply_adjustments(&mut out, self.exposure, self.contrast, self.saturation);
    out
  }

  /// Applies the current tool at `uv` (normalized `[0, 1]` canvas coordinates).
  pub fn apply_tool_at(&mut self, uv: Pos2, prev_uv: Option<Pos2>) {
    let px = (uv.x.clamp(0.0, 1.0) * (CANVAS_W - 1) as f32).round() as i32;
    let py = (uv.y.clamp(0.0, 1.0) * (CANVAS_H - 1) as f32).round() as i32;
    let r = self.brush_radius.max(1.0);
    let r_i = r.ceil() as i32;

    match self.tool {
      DrawTool::ColorPicker => {
        let comp = self.composite_rgba();
        let idx = ((py as u32 * CANVAS_W + px as u32) * 4) as usize;
        if idx + 3 < comp.len() {
          self.brush_color = [comp[idx], comp[idx + 1], comp[idx + 2], 255];
        }
      },
      DrawTool::Brush | DrawTool::Eraser => {
        let Some(layer) = self.layers.get_mut(self.active_layer) else {
          return;
        };
        for dy in -r_i..=r_i {
          for dx in -r_i..=r_i {
            let x = px + dx;
            let y = py + dy;
            if !(0..CANVAS_W as i32).contains(&x) || !(0..CANVAS_H as i32).contains(&y) {
              continue;
            }
            let dist = ((dx * dx + dy * dy) as f32).sqrt();
            if dist > r {
              continue;
            }
            let falloff = (1.0 - dist / r).powf((1.0 - self.brush_hardness).max(0.15)).clamp(0.0, 1.0);
            let idx = ((y as u32 * CANVAS_W + x as u32) * 4) as usize;
            if self.tool == DrawTool::Eraser {
              let a = layer.pixels[idx + 3] as f32 * (1.0 - falloff);
              layer.pixels[idx + 3] = a.round() as u8;
            } else {
              let a = (self.brush_color[3] as f32 / 255.0) * falloff;
              for (c, &target) in self.brush_color[..3].iter().enumerate() {
                let cur = layer.pixels[idx + c] as f32;
                layer.pixels[idx + c] = (cur * (1.0 - a) + target as f32 * a).round() as u8;
              }
              layer.pixels[idx + 3] = layer.pixels[idx + 3].max((a * 255.0).round() as u8);
            }
          }
        }
        self.dirty = true;
      },
      DrawTool::LiquifyPush | DrawTool::LiquifyBloat | DrawTool::LiquifyPucker => {
        let Some(layer) = self.layers.get_mut(self.active_layer) else {
          return;
        };
        let delta = prev_uv.map_or(vec2(0.02, 0.0), |p| uv - p);
        apply_liquify_brush(&mut layer.pixels, CANVAS_W, CANVAS_H, px as f32, py as f32, r * 1.4, self.tool, delta * vec2(CANVAS_W as f32, CANVAS_H as f32));
        self.dirty = true;
      },
    }
  }

  /// Applies the selected filter to the active layer.
  pub fn apply_filter_to_active(&mut self) {
    if let Some(layer) = self.layers.get_mut(self.active_layer) {
      apply_filter(&mut layer.pixels, layer.width, layer.height, self.filter, self.filter_intensity);
      self.dirty = true;
    }
  }

  /// Sharpens the active layer and refines its soft alpha edges.
  pub fn refine_edges_of_active(&mut self) {
    if let Some(layer) = self.layers.get_mut(self.active_layer) {
      refine_edges_rgba(&mut layer.pixels, layer.width, layer.height, self.sharpen, self.edge_contrast, self.edge_shift);
      self.dirty = true;
    }
  }

  pub fn ui(&mut self, ui: &mut Ui, env: &mut Env<'_>) {
    self.refresh_texture(&env.ctx);
    let area = ui.max_rect().shrink2(vec2(16.0, 12.0));
    let mut root = ui.new_child(egui::UiBuilder::new().max_rect(area));

    root.columns(3, |cols| {
      cols[0].set_max_width(270.0);
      egui::ScrollArea::vertical().id_salt("edit_img_left").show(&mut cols[0], |ui| self.tools_column(ui, env));
      self.canvas_column(&mut cols[1], env);
      egui::ScrollArea::vertical().id_salt("edit_img_right").show(&mut cols[2], |ui| self.layers_column(ui, env));
    });
  }

  /// Left column: tools, edge refinement and filters.
  fn tools_column(&mut self, ui: &mut Ui, env: &mut Env<'_>) {
    widgets::section_label(ui, "01 / Tools", theme::ACCENT_INK);
    for tool in DrawTool::ALL {
      let selected = self.tool == tool;
      if widgets::menu_row(ui, widgets::RowLead::Icon(Icon::Pencil), tool.label(), None, selected, false, selected).clicked() {
        self.tool = tool;
      }
    }
    ui.add_space(6.0);
    ui.add(egui::Slider::new(&mut self.brush_radius, 1.0..=42.0).text("Brush Size"));
    ui.add(egui::Slider::new(&mut self.brush_hardness, 0.1..=1.0).text("Hardness"));
    let mut rgb = [self.brush_color[0] as f32 / 255.0, self.brush_color[1] as f32 / 255.0, self.brush_color[2] as f32 / 255.0];
    ui.horizontal(|ui| {
      ui.label("Brush Color");
      if ui.color_edit_button_rgb(&mut rgb).changed() {
        self.brush_color = [(rgb[0] * 255.0) as u8, (rgb[1] * 255.0) as u8, (rgb[2] * 255.0) as u8, 255];
      }
    });

    ui.add_space(10.0);
    widgets::section_label(ui, "02 / Sharpen & Edges", theme::ACCENT_INK);
    ui.add(egui::Slider::new(&mut self.sharpen, 0.0..=1.0).text("Sharpen"));
    ui.add(egui::Slider::new(&mut self.edge_contrast, 0.0..=1.0).text("Edge Contrast"));
    ui.add(egui::Slider::new(&mut self.edge_shift, -1.0..=1.0).text("Edge Shift"));
    if widgets::button(ui, Some(Icon::Sparkles), "Apply to Layer", Kind::Secondary, ui.available_width(), 32.0).clicked() {
      self.refine_edges_of_active();
      env.toasts.success("Sharpened the active layer");
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
      self.apply_filter_to_active();
      env.toasts.success(format!("Applied {} to the active layer", self.filter.label()));
    }
  }

  /// Center column: the canvas, and loading or sending the composite.
  fn canvas_column(&mut self, ui: &mut Ui, env: &mut Env<'_>) {
    widgets::hud(ui, &format!("CANVAS · {CANVAS_W} × {CANVAS_H} PX"), theme::MUTED);
    ui.add_space(6.0);
    let frame_size = vec2(ui.available_width(), (ui.available_height() - 56.0).max(210.0));
    let (frame_rect, resp) = ui.allocate_exact_size(frame_size, Sense::click_and_drag());
    ui.painter().rect(frame_rect, 0.0, theme::SUNKEN, Stroke::new(1.0, theme::LINE_STRONG), egui::StrokeKind::Inside);
    widgets::corner_marks(ui.painter(), frame_rect);

    let canvas = fit_rect(frame_rect.shrink(8.0), CANVAS_W as f32 / CANVAS_H as f32);
    if let Some(tex) = &self.texture {
      ui.painter().image(tex.id(), canvas, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
    }

    // Strokes start on the canvas and keep going (clamped to its edges) while dragging.
    let pointer = resp.interact_pointer_pos().filter(|p| self.last_drag_uv.is_some() || canvas.contains(*p));
    match pointer {
      Some(pos) if (resp.dragged() || resp.clicked()) && canvas.width() > 1.0 && canvas.height() > 1.0 => {
        let uv = pos2(((pos.x - canvas.left()) / canvas.width()).clamp(0.0, 1.0), ((pos.y - canvas.top()) / canvas.height()).clamp(0.0, 1.0));
        let prev = self.last_drag_uv;
        self.apply_tool_at(uv, prev);
        self.last_drag_uv = Some(uv);
      },
      _ => self.last_drag_uv = None,
    }

    ui.add_space(8.0);
    ui.horizontal(|ui| {
      if widgets::button(ui, Some(Icon::Upload), "Load Image", Kind::Secondary, 130.0, 34.0).clicked() {
        self.load_image_file(env);
      }
      if widgets::button(ui, Some(Icon::Image), "Send to Create Image", Kind::Secondary, 170.0, 34.0).clicked() {
        self.send_composite(env, None);
      }
      if widgets::button(ui, Some(Icon::Video), "Send to Create Video", Kind::Secondary, 170.0, 34.0).clicked() {
        self.send_composite(env, Some(ImageSlot::FirstFrame));
      }
    });
  }

  /// Right column: the layer stack, tone adjustments and export.
  fn layers_column(&mut self, ui: &mut Ui, env: &mut Env<'_>) {
    widgets::section_label(ui, "04 / Layers", theme::ACCENT_INK);
    ui.horizontal(|ui| {
      if ui.button("+ Layer").clicked() {
        let n = self.layers.len() + 1;
        self.layers.push(CanvasLayer::new_solid(format!("Layer {n}"), CANVAS_W, CANVAS_H, [0, 0, 0, 0]));
        self.active_layer = self.layers.len() - 1;
        self.dirty = true;
      }
      if ui.button("Duplicate").clicked() {
        if let Some(mut dup) = self.layers.get(self.active_layer).cloned() {
          dup.name = format!("{} Copy", dup.name);
          self.layers.push(dup);
          self.active_layer = self.layers.len() - 1;
          self.dirty = true;
        }
      }
      if self.layers.len() > 1 && ui.button("Delete").clicked() {
        self.layers.remove(self.active_layer);
        self.active_layer = self.active_layer.min(self.layers.len() - 1);
        self.dirty = true;
      }
    });

    ui.add_space(6.0);
    let mut dirty = false;
    for (i, layer) in self.layers.iter_mut().enumerate().rev() {
      let is_active = i == self.active_layer;
      egui::Frame::new()
        .fill(if is_active { theme::WASH_SELECTED } else { theme::CONTROLS })
        .stroke(Stroke::new(1.0, if is_active { theme::ACCENT } else { theme::LINE }))
        .corner_radius(theme::RADIUS)
        .inner_margin(egui::Margin::same(8))
        .show(ui, |ui| {
          ui.horizontal(|ui| {
            dirty |= ui.checkbox(&mut layer.visible, "").changed();
            if ui.selectable_label(is_active, &layer.name).clicked() {
              self.active_layer = i;
            }
          });
          dirty |= ui.add(egui::Slider::new(&mut layer.opacity, 0.0..=1.0).text("Opacity")).changed();
          egui::ComboBox::from_id_salt(("edit_img_blend", i)).selected_text(layer.blend.label()).show_ui(ui, |ui| {
            for mode in BlendMode::ALL {
              dirty |= ui.selectable_value(&mut layer.blend, mode, mode.label()).changed();
            }
          });
        });
      ui.add_space(4.0);
    }

    ui.add_space(10.0);
    widgets::section_label(ui, "05 / Adjustments", theme::ACCENT_INK);
    dirty |= ui.add(egui::Slider::new(&mut self.exposure, -1.0..=1.0).text("Exposure")).changed();
    dirty |= ui.add(egui::Slider::new(&mut self.contrast, -1.0..=1.0).text("Contrast")).changed();
    dirty |= ui.add(egui::Slider::new(&mut self.saturation, -1.0..=1.0).text("Saturation")).changed();
    if dirty {
      self.dirty = true;
    }

    ui.add_space(10.0);
    widgets::section_label(ui, "06 / Export", theme::ACCENT_INK);
    if widgets::button(ui, Some(Icon::Wand), "Send Layer to BG Remove", Kind::Secondary, ui.available_width(), 34.0).clicked() {
      if let Some(layer) = self.layers.get(self.active_layer) {
        env.requests.push(AppRequest::SendRgbaToRemoveBg { width: layer.width, height: layer.height, pixels: layer.pixels.clone() });
      }
    }
    ui.add_space(6.0);
    if widgets::button(ui, Some(Icon::Download), "Save Composite PNG", Kind::Primary, ui.available_width(), 36.0).clicked() {
      self.save_composite(env);
    }
  }

  fn load_image_file(&mut self, env: &mut Env<'_>) {
    let Some(path) = common::pick_files(&[RefKind::Image], false).into_iter().next() else {
      return;
    };
    let name = common::file_name(&path);
    match image::open(&path) {
      Ok(img) => {
        let rgba = img.to_rgba8();
        self.import_rgba_layer(rgba.width(), rgba.height(), rgba.into_raw(), name.clone());
        env.toasts.success(format!("Added {name} as a new layer"));
      },
      Err(err) => env.toasts.error(format!("Couldn't open {name}: {err}")),
    }
  }

  /// Attaches the composite to Create Image (`slot` = `None`) or to a Create Video slot.
  fn send_composite(&self, env: &mut Env<'_>, video_slot: Option<ImageSlot>) {
    let Some(png) = common::encode_rgba_to_png(CANVAS_W, CANVAS_H, &self.composite_rgba()) else {
      env.toasts.error("Couldn't encode the canvas as a PNG");
      return;
    };
    env.requests.push(match video_slot {
      Some(slot) => AppRequest::SendPngToCreateVideo { png, slot, prompt: None },
      None => AppRequest::SendPngToCreateImage { png, prompt: None },
    });
  }

  fn save_composite(&self, env: &mut Env<'_>) {
    let Some(png) = common::encode_rgba_to_png(CANVAS_W, CANVAS_H, &self.composite_rgba()) else {
      env.toasts.error("Couldn't encode the canvas as a PNG");
      return;
    };
    let Some(path) = FileDialog::new().set_file_name("artcraft_composite.png").save_file() else {
      return;
    };
    match fs::write(&path, png) {
      Ok(()) => env.toasts.success(format!("Saved {}", common::file_name(&path))),
      Err(err) => env.toasts.error(format!("Failed to save PNG: {err}")),
    }
  }

  fn refresh_texture(&mut self, ctx: &egui::Context) {
    if !self.dirty && self.texture.is_some() {
      return;
    }
    let image = egui::ColorImage::from_rgba_unmultiplied([CANVAS_W as usize, CANVAS_H as usize], &self.composite_rgba());
    match &mut self.texture {
      Some(tex) => tex.set(image, TextureOptions::LINEAR),
      None => self.texture = Some(ctx.load_texture("edit_image_canvas", image, TextureOptions::LINEAR)),
    }
    self.dirty = false;
  }
}

impl DrawTool {
  pub const ALL: [Self; 6] = [Self::Brush, Self::Eraser, Self::LiquifyPush, Self::LiquifyBloat, Self::LiquifyPucker, Self::ColorPicker];

  pub fn label(self) -> &'static str {
    match self {
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
    Self { name: name.into(), visible: true, opacity: 1.0, blend: BlendMode::Normal, width, height, pixels: rgba.repeat(width as usize * height as usize) }
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
  let mut out = vec![0u8; rgba_len(dst_w, dst_h)];
  if src_w == 0 || src_h == 0 || dst_w == 0 || dst_h == 0 || src.len() < rgba_len(src_w, src_h) {
    return out;
  }
  let (fit_w, fit_h) = fit_dims(src_w, src_h, dst_w, dst_h);
  let scaled = resample_rgba(src, src_w, src_h, fit_w, fit_h);
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
  let mut out = vec![0u8; rgba_len(dst_w, dst_h)];
  if src_w == 0 || src_h == 0 || src.len() < rgba_len(src_w, src_h) {
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
pub fn refine_edges_rgba(pixels: &mut [u8], width: u32, height: u32, sharpen: f32, edge_contrast: f32, edge_shift: f32) {
  if width < 2 || height < 2 || pixels.len() < rgba_len(width, height) {
    return;
  }
  let src = pixels.to_vec();
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
pub fn apply_liquify_brush(pixels: &mut [u8], width: u32, height: u32, cx: f32, cy: f32, radius: f32, mode: DrawTool, delta: Vec2) {
  if width == 0 || height == 0 || pixels.len() < rgba_len(width, height) {
    return;
  }
  let src = pixels.to_vec();
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
}

/// Applies a filter to an RGBA8 buffer, mixed with the original by `intensity`.
pub fn apply_filter(pixels: &mut [u8], width: u32, height: u32, preset: FilterPreset, intensity: f32) {
  if width == 0 || height == 0 || pixels.len() < rgba_len(width, height) {
    return;
  }
  let src = pixels.to_vec();
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
}

fn rgba_len(width: u32, height: u32) -> usize {
  width as usize * height as usize * 4
}

#[cfg(test)]
mod tests {
  use super::*;

  mod compositing {
    use super::*;

    #[test]
    fn blend_modes_show_the_layer_as_is_over_a_transparent_backdrop() {
      let mut page = EditImagePage::default();
      page.layers.clear();
      page.layers.push(CanvasLayer { blend: BlendMode::Multiply, ..CanvasLayer::new_solid("Red", CANVAS_W, CANVAS_H, [200, 40, 40, 255]) });
      assert_eq!(page.composite_rgba()[..4], [200, 40, 40, 255]);
    }

    #[test]
    fn multiply_and_screen_blend_with_an_opaque_backdrop() {
      let mut page = EditImagePage::default();
      page.layers.clear();
      page.layers.push(CanvasLayer::new_solid("Grey", CANVAS_W, CANVAS_H, [128, 128, 128, 255]));
      page.layers.push(CanvasLayer { blend: BlendMode::Multiply, ..CanvasLayer::new_solid("Top", CANVAS_W, CANVAS_H, [128, 255, 0, 255]) });
      assert_eq!(page.composite_rgba()[..4], [64, 128, 0, 255]);
      page.layers[1].blend = BlendMode::Screen;
      assert_eq!(page.composite_rgba()[..4], [192, 255, 128, 255]);
    }

    #[test]
    fn half_opacity_mixes_with_the_backdrop() {
      let mut page = EditImagePage::default();
      page.layers.truncate(1);
      page.layers.push(CanvasLayer { opacity: 0.5, ..CanvasLayer::new_solid("Red", CANVAS_W, CANVAS_H, [255, 0, 0, 255]) });
      assert_eq!(page.composite_rgba()[..4], [255, 128, 128, 255]);
    }
  }

  #[test]
  fn imported_images_keep_their_aspect_ratio() {
    let mut page = EditImagePage::default();
    page.import_rgba_layer(100, 100, [255, 0, 0, 255].repeat(100 * 100), "Square".into());
    let layer = page.layers.last().expect("imported layer");
    let at = |x: u32, y: u32| {
      let i = ((y * CANVAS_W + x) * 4) as usize;
      [layer.pixels[i], layer.pixels[i + 1], layer.pixels[i + 2], layer.pixels[i + 3]]
    };
    // 100 x 100 fits as 200 x 200, centered: columns 60..260 hold the image.
    assert_eq!(at(30, 100), [0, 0, 0, 0]);
    assert_eq!(at(160, 100), [255, 0, 0, 255]);
    assert_eq!(at(290, 100), [0, 0, 0, 0]);
  }

  #[test]
  fn edge_glow_lights_edges_and_darkens_flat_areas() {
    let mut page = EditImagePage::default();
    page.layers.clear();
    let mut layer = CanvasLayer::new_solid("Split", CANVAS_W, CANVAS_H, [0, 0, 0, 255]);
    for px in layer.pixels.chunks_exact_mut(4).enumerate().filter(|(i, _)| *i as u32 % CANVAS_W >= CANVAS_W / 2).map(|(_, px)| px) {
      px.copy_from_slice(&[255, 255, 255, 255]);
    }
    page.layers.push(layer);
    page.active_layer = 0;
    page.filter = FilterPreset::EdgeGlow;
    page.filter_intensity = 1.0;
    page.apply_filter_to_active();
    let pixels = &page.layers[0].pixels;
    let at = |x: u32| ((10 * CANVAS_W + x) * 4) as usize;
    assert_eq!(pixels[at(CANVAS_W / 2 - 1) + 2], 255, "the pixel next to the edge glows");
    assert_eq!(pixels[at(300)..at(300) + 3], [0, 0, 0], "a flat area goes dark");
  }
}
