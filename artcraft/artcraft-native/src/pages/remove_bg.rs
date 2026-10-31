//! BG Remove (local preview): color-key matting (a corner-color key, a green-screen key and a luma
//! key) with feathering, spill suppression and edge refinement, all on this computer. There is no
//! AI segmentation here; that lives in the web app.

use std::fs;

use egui::{Color32, Rect, Sense, Stroke, TextureHandle, TextureOptions, Ui, pos2, vec2};
use rfd::FileDialog;

use crate::pages::common::{self, AppRequest, Env};
use crate::pages::edit_image::{fit_dims, fit_rect, refine_edges_rgba, resample_rgba};
use crate::prompt_box::refs::RefKind;
use crate::theme;
use crate::ui::icons::Icon;
use crate::ui::widgets::{self, Kind};

/// The largest matte size. Sources are scaled to fit inside it, keeping their aspect ratio.
pub const MATTE_MAX_W: u32 = 320;
pub const MATTE_MAX_H: u32 = 200;

pub struct RemoveBgPage {
  pub src_rgba: Vec<u8>,
  pub cutout_rgba: Vec<u8>,
  /// The matte size: the source fitted inside `MATTE_MAX_W x MATTE_MAX_H`.
  pub width: u32,
  pub height: u32,
  pub mode: KeyMode,
  pub backdrop: BackdropPreview,
  pub threshold: f32,
  pub feather_radius: f32,
  pub edge_decontaminate: f32,
  pub spill_suppression: f32,
  pub edge_sharpen: f32,
  pub edge_contrast: f32,
  pub edge_shift: f32,
  pub split_wipe: f32,
  texture: Option<TextureHandle>,
  dirty: bool,
}

/// How a pixel's "subject" score is computed before thresholding.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KeyMode {
  /// Distance from the average color of the four corners (a flat backdrop).
  CornerColor,
  /// How little green dominates the pixel (a green screen).
  GreenScreen,
  /// Brightness (bright subjects on dark backdrops).
  Luma,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BackdropPreview {
  Checkerboard,
  SolidDark,
  AlphaMatte,
}

impl Default for RemoveBgPage {
  fn default() -> Self {
    let mut page = Self {
      src_rgba: sample_image(MATTE_MAX_W, MATTE_MAX_H),
      cutout_rgba: Vec::new(),
      width: MATTE_MAX_W,
      height: MATTE_MAX_H,
      mode: KeyMode::CornerColor,
      backdrop: BackdropPreview::Checkerboard,
      threshold: 0.36,
      feather_radius: 1.5,
      edge_decontaminate: 0.65,
      spill_suppression: 0.55,
      edge_sharpen: 0.45,
      edge_contrast: 0.35,
      edge_shift: 0.0,
      split_wipe: 0.15,
      texture: None,
      dirty: true,
    };
    page.recompute_matte();
    page
  }
}

impl RemoveBgPage {
  /// Replaces the source with an RGBA buffer (fitted inside the matte bounds) and re-keys it.
  pub fn import_source_rgba(&mut self, width: u32, height: u32, pixels: Vec<u8>) {
    let (w, h) = fit_dims(width, height, MATTE_MAX_W, MATTE_MAX_H);
    self.src_rgba = resample_rgba(&pixels, width, height, w, h);
    self.width = w;
    self.height = h;
    self.recompute_matte();
  }

  /// Keys `src_rgba` into `cutout_rgba` with the current settings.
  pub fn recompute_matte(&mut self) {
    let mut out = compute_alpha_matte(&self.src_rgba, self.width, self.height, self.mode, self.threshold, self.feather_radius, self.edge_decontaminate, self.spill_suppression);
    if self.edge_sharpen > 1e-4 || self.edge_contrast > 1e-4 || self.edge_shift.abs() > 1e-4 {
      refine_edges_rgba(&mut out, self.width, self.height, self.edge_sharpen, self.edge_contrast, self.edge_shift);
    }
    self.cutout_rgba = out;
    self.dirty = true;
  }

  /// The share of the matte that is mostly opaque, in percent.
  pub fn foreground_coverage_percent(&self) -> f32 {
    let total = (self.width as usize * self.height as usize).max(1) as f32;
    let fg = self.cutout_rgba.chunks_exact(4).filter(|px| px[3] > 127).count() as f32;
    (fg / total) * 100.0
  }

  pub fn ui(&mut self, ui: &mut Ui, env: &mut Env<'_>) {
    self.refresh_texture(&env.ctx);
    let area = ui.max_rect().shrink2(vec2(16.0, 12.0));
    let mut root = ui.new_child(egui::UiBuilder::new().max_rect(area));

    root.columns(3, |cols| {
      cols[0].set_max_width(280.0);
      self.settings_column(&mut cols[0], env);
      self.preview_column(&mut cols[1], env);
      self.export_column(&mut cols[2], env);
    });
  }

  /// Left column: the key, its settings and the source.
  fn settings_column(&mut self, ui: &mut Ui, env: &mut Env<'_>) {
    let mut changed = false;
    widgets::section_label(ui, "01 / Color Key", theme::ACCENT_INK);
    for mode in KeyMode::ALL {
      let selected = self.mode == mode;
      if widgets::menu_row(ui, widgets::RowLead::Icon(Icon::Wand), mode.label(), None, selected, false, selected).clicked() {
        self.mode = mode;
        changed = true;
      }
    }

    ui.add_space(10.0);
    widgets::section_label(ui, "02 / Matte & Edges", theme::ACCENT_INK);
    changed |= ui.add(egui::Slider::new(&mut self.threshold, 0.05..=0.90).text("Threshold")).changed();
    changed |= ui.add(egui::Slider::new(&mut self.feather_radius, 0.0..=5.0).text("Feather Radius (px)")).changed();
    changed |= ui.add(egui::Slider::new(&mut self.edge_sharpen, 0.0..=1.0).text("Edge Sharpen")).changed();
    changed |= ui.add(egui::Slider::new(&mut self.edge_contrast, 0.0..=1.0).text("Edge Contrast")).changed();
    changed |= ui.add(egui::Slider::new(&mut self.edge_shift, -1.0..=1.0).text("Edge Shift")).changed();
    changed |= ui.add(egui::Slider::new(&mut self.edge_decontaminate, 0.0..=1.0).text("Decontaminate Colors")).changed();
    changed |= ui.add(egui::Slider::new(&mut self.spill_suppression, 0.0..=1.0).text("Green Spill Suppression")).changed();

    ui.add_space(10.0);
    widgets::section_label(ui, "03 / Source Image", theme::ACCENT_INK);
    if widgets::button(ui, Some(Icon::Upload), "Load Image", Kind::Secondary, ui.available_width(), 34.0).clicked() {
      self.load_image_file(env);
    }
    ui.add_space(4.0);
    if widgets::button(ui, Some(Icon::RotateCw), "Use Sample Image", Kind::Secondary, ui.available_width(), 32.0).clicked() {
      self.src_rgba = sample_image(MATTE_MAX_W, MATTE_MAX_H);
      self.width = MATTE_MAX_W;
      self.height = MATTE_MAX_H;
      changed = true;
    }
    if changed {
      self.recompute_matte();
    }
  }

  /// Center column: the before / after preview.
  fn preview_column(&mut self, ui: &mut Ui, env: &mut Env<'_>) {
    widgets::hud(ui, "BEFORE | AFTER · DRAG TO MOVE THE SPLIT", theme::MUTED);
    ui.add_space(6.0);
    let frame_size = vec2(ui.available_width(), (ui.available_height() - 120.0).max(220.0));
    let (frame_rect, resp) = ui.allocate_exact_size(frame_size, Sense::click_and_drag());
    ui.painter().rect(frame_rect, 0.0, theme::SUNKEN, Stroke::new(1.0, theme::LINE_STRONG), egui::StrokeKind::Inside);
    widgets::corner_marks(ui.painter(), frame_rect);

    let inner = fit_rect(frame_rect.shrink(8.0), self.width as f32 / self.height.max(1) as f32);
    if let Some(tex) = &self.texture {
      ui.painter().image(tex.id(), inner, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
    }
    let wipe_x = inner.left() + inner.width() * self.split_wipe.clamp(0.0, 1.0);
    ui.painter().vline(wipe_x, inner.y_range(), Stroke::new(2.0, theme::ACCENT));
    if resp.dragged() || resp.clicked() {
      if let Some(pos) = resp.interact_pointer_pos() {
        self.split_wipe = ((pos.x - inner.left()) / inner.width().max(1.0)).clamp(0.0, 1.0);
        self.dirty = true;
      }
    }

    ui.add_space(8.0);
    if ui.add(egui::Slider::new(&mut self.split_wipe, 0.0..=1.0).text("Before / After Split")).changed() {
      self.dirty = true;
    }
    ui.add_space(6.0);
    ui.horizontal(|ui| {
      if widgets::button(ui, Some(Icon::Pencil), "Send Cutout to Edit Image", Kind::Accent, 185.0, 34.0).clicked() {
        env.requests.push(AppRequest::SendRgbaToEditImage { width: self.width, height: self.height, pixels: self.cutout_rgba.clone(), layer_name: "Cutout".into() });
      }
      if widgets::button(ui, Some(Icon::Box), "Send Cutout to 3D Stage", Kind::Secondary, 180.0, 34.0).clicked() {
        env.requests.push(AppRequest::SendRgbaToStage3d { width: self.width, height: self.height, pixels: self.cutout_rgba.clone(), card_name: "Cutout Card".into() });
      }
    });
  }

  /// Right column: the preview backdrop and export.
  fn export_column(&mut self, ui: &mut Ui, env: &mut Env<'_>) {
    widgets::section_label(ui, "04 / Preview Backdrop", theme::ACCENT_INK);
    for backdrop in BackdropPreview::ALL {
      let selected = self.backdrop == backdrop;
      if widgets::menu_row(ui, widgets::RowLead::Icon(Icon::Image), backdrop.label(), None, selected, false, selected).clicked() {
        self.backdrop = backdrop;
        self.dirty = true;
      }
    }

    ui.add_space(12.0);
    widgets::section_label(ui, "05 / Export Cutout", theme::ACCENT_INK);
    let coverage = self.foreground_coverage_percent();
    ui.label(egui::RichText::new(format!("{} × {} px · {coverage:.1}% opaque", self.width, self.height)).size(12.5).color(theme::MUTED));
    ui.add_space(8.0);
    if widgets::button(ui, Some(Icon::Download), "Save Transparent PNG", Kind::Primary, ui.available_width(), 36.0).clicked() {
      self.save_cutout(env);
    }
    ui.add_space(6.0);
    if widgets::button(ui, Some(Icon::Image), "Send Cutout to Create Image", Kind::Secondary, ui.available_width(), 34.0).clicked() {
      match common::encode_rgba_to_png(self.width, self.height, &self.cutout_rgba) {
        Some(png) => env.requests.push(AppRequest::SendPngToCreateImage { png, prompt: None }),
        None => env.toasts.error("Couldn't encode the cutout as a PNG"),
      }
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
        self.import_source_rgba(rgba.width(), rgba.height(), rgba.into_raw());
        env.toasts.success(format!("Loaded {name}"));
      },
      Err(err) => env.toasts.error(format!("Couldn't open {name}: {err}")),
    }
  }

  fn save_cutout(&self, env: &mut Env<'_>) {
    let Some(png) = common::encode_rgba_to_png(self.width, self.height, &self.cutout_rgba) else {
      env.toasts.error("Couldn't encode the cutout as a PNG");
      return;
    };
    let Some(path) = FileDialog::new().set_file_name("artcraft_cutout.png").save_file() else {
      return;
    };
    match fs::write(&path, png) {
      Ok(()) => env.toasts.success(format!("Saved the cutout to {}", common::file_name(&path))),
      Err(err) => env.toasts.error(format!("Failed to save the cutout: {err}")),
    }
  }

  fn refresh_texture(&mut self, ctx: &egui::Context) {
    if !self.dirty && self.texture.is_some() {
      return;
    }
    let preview = render_backdrop_preview(&self.src_rgba, &self.cutout_rgba, self.width, self.height, self.backdrop, self.split_wipe);
    let image = egui::ColorImage::from_rgba_unmultiplied([self.width as usize, self.height as usize], &preview);
    match &mut self.texture {
      Some(tex) => tex.set(image, TextureOptions::LINEAR),
      None => self.texture = Some(ctx.load_texture("remove_bg_preview", image, TextureOptions::LINEAR)),
    }
    self.dirty = false;
  }
}

impl KeyMode {
  pub const ALL: [Self; 3] = [Self::CornerColor, Self::GreenScreen, Self::Luma];

  pub fn label(self) -> &'static str {
    match self {
      Self::CornerColor => "Corner Color Key",
      Self::GreenScreen => "Green Screen Key",
      Self::Luma => "Luma Key (bright subject)",
    }
  }
}

impl BackdropPreview {
  pub const ALL: [Self; 3] = [Self::Checkerboard, Self::SolidDark, Self::AlphaMatte];

  pub fn label(self) -> &'static str {
    match self {
      Self::Checkerboard => "Checkerboard",
      Self::SolidDark => "Solid Dark",
      Self::AlphaMatte => "Alpha Matte (B&W)",
    }
  }
}

/// A synthetic sample (a figure on a flat teal backdrop) so the page shows something before an
/// image is loaded.
pub fn sample_image(width: u32, height: u32) -> Vec<u8> {
  let mut out = vec![0u8; width as usize * height as usize * 4];
  let cx = width as f32 * 0.5;
  let cy = height as f32 * 0.52;
  for y in 0..height {
    for x in 0..width {
      let idx = ((y * width + x) * 4) as usize;
      let mut rgb = [34u8, 148, 96];
      let dx = (x as f32 - cx) / (width as f32 * 0.16);
      let dy_head = (y as f32 - (cy - height as f32 * 0.12)) / (height as f32 * 0.18);
      let dy_body = (y as f32 - (cy + height as f32 * 0.24)) / (height as f32 * 0.26);
      let in_head = dx * dx + dy_head * dy_head <= 1.0;
      let in_body = (dx * 0.65).powi(2) + dy_body * dy_body <= 1.0 && y as f32 > cy;
      if in_head || in_body {
        rgb = [(218.0 - dx.abs() * 35.0) as u8, (142.0 - dy_head.abs() * 20.0) as u8, (118.0 + dx * 15.0).clamp(60.0, 200.0) as u8];
      }
      out[idx..idx + 3].copy_from_slice(&rgb);
      out[idx + 3] = 255;
    }
  }
  out
}

/// Keys an RGBA8 image: scores each pixel by `mode`, thresholds it into a soft alpha, box-feathers
/// the alpha, then suppresses green spill and pulls the backdrop color out of soft edges.
#[allow(clippy::too_many_arguments)]
pub fn compute_alpha_matte(src: &[u8], width: u32, height: u32, mode: KeyMode, threshold: f32, feather_radius: f32, edge_decontaminate: f32, spill_suppression: f32) -> Vec<u8> {
  let count = width as usize * height as usize;
  if width == 0 || height == 0 || src.len() < count * 4 {
    return vec![0u8; count * 4];
  }
  // The backdrop reference: the average of the four corner pixels.
  let corner = |x: u32, y: u32| {
    let p = ((y * width + x) * 4) as usize;
    [src[p] as f32, src[p + 1] as f32, src[p + 2] as f32]
  };
  let corners = [corner(0, 0), corner(width - 1, 0), corner(0, height - 1), corner(width - 1, height - 1)];
  let [bg_r, bg_g, bg_b] = [0, 1, 2].map(|c| corners.iter().map(|k| k[c]).sum::<f32>() / 4.0);

  let thresh = threshold.clamp(0.05, 0.95);
  let mut alpha: Vec<f32> = src
    .chunks_exact(4)
    .take(count)
    .map(|px| {
      let (r, g, b) = (px[0] as f32, px[1] as f32, px[2] as f32);
      let score = match mode {
        KeyMode::CornerColor => ((r - bg_r).abs() + (g - bg_g).abs() + (b - bg_b).abs()) / 765.0 * 1.8,
        KeyMode::GreenScreen => 1.0 - ((g - r.max(b)).max(0.0) / 255.0 * 2.4).clamp(0.0, 1.0),
        KeyMode::Luma => (0.2126 * r + 0.7152 * g + 0.0722 * b) / 255.0,
      };
      ((score - thresh) / 0.14).clamp(0.0, 1.0)
    })
    .collect();

  let rad = feather_radius.round() as i32;
  if rad >= 1 {
    let prev = alpha.clone();
    let (w, h) = (width as i32, height as i32);
    for y in 0..h {
      for x in 0..w {
        let mut sum = 0.0;
        let mut n = 0.0;
        for dy in -rad..=rad {
          for dx in -rad..=rad {
            let nx = (x + dx).clamp(0, w - 1);
            let ny = (y + dy).clamp(0, h - 1);
            sum += prev[(ny * w + nx) as usize];
            n += 1.0;
          }
        }
        alpha[(y * w + x) as usize] = sum / n;
      }
    }
  }

  let mut out = src[..count * 4].to_vec();
  let spill = spill_suppression.clamp(0.0, 1.0);
  for (px, &a) in out.chunks_exact_mut(4).zip(&alpha) {
    let a = a.clamp(0.0, 1.0);
    let (mut r, mut g, mut b) = (px[0] as f32, px[1] as f32, px[2] as f32);
    let rb_avg = 0.5 * (r + b);
    if spill > 0.0 && g > rb_avg {
      g -= (g - rb_avg) * spill;
    }
    if edge_decontaminate > 0.0 && (0.05..0.95).contains(&a) {
      let dec = edge_decontaminate * (1.0 - a);
      let k = (1.0 - dec * 0.5).max(0.2);
      r = (r - bg_r * dec) / k;
      g = (g - bg_g * dec) / k;
      b = (b - bg_b * dec) / k;
    }
    px[0] = r.clamp(0.0, 255.0).round() as u8;
    px[1] = g.clamp(0.0, 255.0).round() as u8;
    px[2] = b.clamp(0.0, 255.0).round() as u8;
    px[3] = (a * 255.0).round() as u8;
  }
  out
}

/// The preview: the source left of `split_wipe`, the cutout over `backdrop` to its right.
pub fn render_backdrop_preview(src: &[u8], cutout: &[u8], width: u32, height: u32, backdrop: BackdropPreview, split_wipe: f32) -> Vec<u8> {
  let len = width as usize * height as usize * 4;
  let mut out = vec![0u8; len];
  if src.len() < len || cutout.len() < len {
    return out;
  }
  let wipe_x = (split_wipe.clamp(0.0, 1.0) * width as f32).round() as u32;
  for y in 0..height {
    for x in 0..width {
      let idx = ((y * width + x) * 4) as usize;
      if x < wipe_x {
        out[idx..idx + 4].copy_from_slice(&src[idx..idx + 4]);
        continue;
      }
      let a = cutout[idx + 3] as f32 / 255.0;
      let bg = match backdrop {
        BackdropPreview::AlphaMatte => {
          let v = cutout[idx + 3];
          out[idx..idx + 4].copy_from_slice(&[v, v, v, 255]);
          continue;
        },
        BackdropPreview::Checkerboard => {
          if ((x / 8) + (y / 8)) % 2 == 0 { 48.0 } else { 28.0 }
        },
        BackdropPreview::SolidDark => 20.0,
      };
      for c in 0..3 {
        out[idx + c] = (cutout[idx + c] as f32 * a + bg * (1.0 - a)).clamp(0.0, 255.0).round() as u8;
      }
      out[idx + 3] = 255;
    }
  }
  out
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn corner_color_key_separates_the_sample_subject_from_its_backdrop() {
    let page = RemoveBgPage::default();
    assert_eq!(page.cutout_rgba[3], 0, "the backdrop corner is transparent");
    let center = (((page.height / 2) * page.width + page.width / 2) * 4 + 3) as usize;
    assert!(page.cutout_rgba[center] > 200, "the subject is opaque");
    let coverage = page.foreground_coverage_percent();
    assert!((5.0..65.0).contains(&coverage), "unexpected coverage: {coverage}");
  }

  #[test]
  fn green_screen_key_drops_green_and_keeps_other_colors() {
    let mut page = RemoveBgPage::default();
    page.import_source_rgba(2, 1, [20, 220, 20, 255, 220, 40, 40, 255].to_vec());
    page.mode = KeyMode::GreenScreen;
    page.feather_radius = 0.0;
    page.recompute_matte();
    let alpha_at = |x: u32| page.cutout_rgba[((page.height / 2 * page.width + x) * 4 + 3) as usize];
    assert_eq!(alpha_at(10), 0, "green is keyed out");
    assert_eq!(alpha_at(page.width - 10), 255, "red is kept");
  }

  #[test]
  fn imported_sources_keep_their_aspect_ratio() {
    let mut page = RemoveBgPage::default();
    page.import_source_rgba(100, 50, [10, 200, 10, 255].repeat(100 * 50));
    assert_eq!((page.width, page.height), (320, 160));
    assert_eq!(page.cutout_rgba.len(), 320 * 160 * 4);
  }
}
