//! 3D Stage (local preview): block out a scene with a posable stick-figure mannequin, box props and
//! cutout cards in a small software-rendered viewport, then send a flat layout sketch of the framed
//! view to Create Image or Create Video as a reference. Everything runs on this computer.

use egui::{Align2, Color32, FontId, Pos2, Rect, Sense, Stroke, Ui, pos2, vec2};

use crate::pages::common::{self, AppRequest, Env};
use crate::pages::edit_image::fit_rect;
use crate::prompt_box::refs::ImageSlot;
use crate::theme;
use crate::ui::icons::Icon;
use crate::ui::widgets::{self, Kind};

/// The layout sketch's size in pixels; the viewport frames the same 16:9 view.
const SKETCH_W: u32 = 240;
const SKETCH_H: u32 = 135;
/// The stage floor's height in scene units.
const FLOOR_Y: f32 = -0.95;
/// How far the camera sits from the scene origin.
const CAMERA_DISTANCE: f32 = 4.2;

pub struct Stage3dPage {
  pub pose_preset: PosePreset,
  pub pose: MannequinPose,
  /// The scene; the first object is always the mannequin (its transform places the figure). Positive
  /// Z is away from the camera's starting view, so new props and cards land in front.
  pub objects: Vec<StageObject>,
  pub selected_object: usize,
  pub cam_yaw_deg: f32,
  pub cam_pitch_deg: f32,
  pub cam_zoom: f32,
  pub focal_length_mm: f32,
  pub shot_prompt: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PosePreset {
  Standing,
  Running,
  PowerStance,
  Crouched,
}

#[derive(Clone, Debug, PartialEq)]
pub struct MannequinPose {
  pub torso_lean_deg: f32,
  pub head_yaw_deg: f32,
  pub left_arm_raise_deg: f32,
  pub right_arm_raise_deg: f32,
  pub left_leg_stride_deg: f32,
  pub right_leg_stride_deg: f32,
}

#[derive(Clone, Debug)]
pub struct StageObject {
  pub name: String,
  pub visible: bool,
  pub pos: [f32; 3],
  pub scale: f32,
  pub color: [u8; 3],
  /// A cutout card's image (`width`, `height`, RGBA8); boxes have none.
  pub cutout_rgba: Option<(u32, u32, Vec<u8>)>,
}

impl Default for Stage3dPage {
  fn default() -> Self {
    Self {
      pose_preset: PosePreset::PowerStance,
      pose: MannequinPose::from_preset(PosePreset::PowerStance),
      objects: vec![
        StageObject { name: "Mannequin".into(), visible: true, pos: [0.0, 0.0, 0.0], scale: 1.0, color: [89, 167, 255], cutout_rgba: None },
        StageObject { name: "Pillar Left".into(), visible: true, pos: [-1.05, -0.2, 0.5], scale: 0.85, color: [140, 150, 175], cutout_rgba: None },
        StageObject { name: "Pillar Right".into(), visible: true, pos: [1.05, -0.2, 0.5], scale: 0.85, color: [140, 150, 175], cutout_rgba: None },
      ],
      selected_object: 0,
      cam_yaw_deg: 22.0,
      cam_pitch_deg: 16.0,
      cam_zoom: 1.0,
      focal_length_mm: 35.0,
      shot_prompt: String::new(),
    }
  }
}

impl Stage3dPage {
  pub fn add_box_prop(&mut self, name: String) {
    let offset = (self.objects.len() as f32 * 0.35).sin() * 0.8;
    self.objects.push(StageObject { name, visible: true, pos: [offset, -0.3, -0.2], scale: 0.7, color: [110, 210, 175], cutout_rgba: None });
    self.selected_object = self.objects.len() - 1;
  }

  /// Adds an RGBA cutout as a card; its average opaque color tints the card in the viewport.
  pub fn add_cutout_card(&mut self, name: String, width: u32, height: u32, pixels: Vec<u8>) {
    let opaque: Vec<&[u8]> = pixels.chunks_exact(4).filter(|px| px[3] > 16).collect();
    let color = if opaque.is_empty() {
      [255, 193, 77]
    } else {
      let n = opaque.len() as u64;
      [0, 1, 2].map(|c| (opaque.iter().map(|px| u64::from(px[c])).sum::<u64>() / n) as u8)
    };
    let offset = (self.objects.len() as f32 * 0.35).sin() * 0.65;
    let valid = width > 0 && height > 0 && pixels.len() >= width as usize * height as usize * 4;
    let cutout_rgba = valid.then_some((width, height, pixels));
    self.objects.push(StageObject { name, visible: true, pos: [offset, -0.25, -0.45], scale: 0.75, color, cutout_rgba });
    self.selected_object = self.objects.len() - 1;
  }

  /// Rasterizes the framed view (far objects first, then the mannequin) into a flat RGBA8 sketch.
  pub fn render_layout_sketch_rgba(&self, width: u32, height: u32) -> Vec<u8> {
    let mut out = vec![0u8; width as usize * height as usize * 4];
    if width == 0 || height == 0 {
      return out;
    }
    for (y, row) in out.chunks_exact_mut(width as usize * 4).enumerate() {
      let v = y as f32 / height as f32;
      let sky = [(20.0 + 38.0 * (1.0 - v)) as u8, (24.0 + 44.0 * (1.0 - v)) as u8, (38.0 + 70.0 * (1.0 - v)) as u8, 255];
      for px in row.chunks_exact_mut(4) {
        px.copy_from_slice(&sky);
      }
    }

    let frame = Rect::from_min_size(pos2(0.0, 0.0), vec2(width as f32, height as f32));
    let zoom = self.lens_zoom();
    let mut props: Vec<(Rect, f32, &StageObject)> = self.objects.iter().skip(1).filter(|o| o.visible).map(|o| {
      let (rect, depth) = object_rect(o, self.cam_yaw_deg, self.cam_pitch_deg, zoom, frame);
      (rect, depth, o)
    }).collect();
    props.sort_by(|a, b| b.1.total_cmp(&a.1));
    for (rect, _, obj) in props {
      fill_object(&mut out, width, height, rect, obj);
    }

    if let Some(mannequin) = self.objects.first().filter(|o| o.visible) {
      let color = [mannequin.color[0], mannequin.color[1], mannequin.color[2], 255];
      let radius = (height as f32 * 0.018 * zoom).max(1.0);
      for (a, b) in mannequin_bones(&self.pose, mannequin) {
        let (p0, _) = project_point_3d(a, self.cam_yaw_deg, self.cam_pitch_deg, zoom, frame);
        let (p1, _) = project_point_3d(b, self.cam_yaw_deg, self.cam_pitch_deg, zoom, frame);
        stamp_line(&mut out, width, height, p0, p1, radius, color);
      }
    }
    out
  }

  pub fn ui(&mut self, ui: &mut Ui, env: &mut Env<'_>) {
    let area = ui.max_rect().shrink2(vec2(16.0, 12.0));
    let mut root = ui.new_child(egui::UiBuilder::new().max_rect(area));

    root.columns(3, |cols| {
      cols[0].set_max_width(280.0);
      self.scene_column(&mut cols[0]);
      self.viewport_column(&mut cols[1], env);
      self.pose_column(&mut cols[2]);
    });
  }

  /// Left column: the outliner and the selected object's transform.
  fn scene_column(&mut self, ui: &mut Ui) {
    widgets::section_label(ui, "01 / Scene", theme::ACCENT_INK);
    if ui.button("+ Box Prop").clicked() {
      self.add_box_prop(format!("Box {}", self.objects.len()));
    }
    ui.add_space(4.0);
    for (i, obj) in self.objects.iter_mut().enumerate() {
      let selected = i == self.selected_object;
      ui.horizontal(|ui| {
        ui.checkbox(&mut obj.visible, "");
        if ui.selectable_label(selected, &obj.name).clicked() {
          self.selected_object = i;
        }
      });
    }

    if let Some(obj) = self.objects.get_mut(self.selected_object) {
      ui.add_space(8.0);
      widgets::section_label(ui, "02 / Transform", theme::ACCENT_INK);
      ui.add(egui::Slider::new(&mut obj.pos[0], -2.0..=2.0).text("Location X"));
      ui.add(egui::Slider::new(&mut obj.pos[1], -1.5..=1.5).text("Location Y"));
      ui.add(egui::Slider::new(&mut obj.pos[2], -2.0..=2.0).text("Location Z"));
      ui.add(egui::Slider::new(&mut obj.scale, 0.2..=2.5).text("Scale"));
    }
  }

  /// Center column: the viewport (drag to orbit, scroll to zoom), the prompt and sending the sketch.
  fn viewport_column(&mut self, ui: &mut Ui, env: &mut Env<'_>) {
    widgets::hud(ui, "STAGE VIEWPORT · DRAG TO ORBIT · SCROLL TO ZOOM", theme::MUTED);
    ui.add_space(6.0);
    let viewport_h = (ui.available_height() - 105.0).max(220.0);
    let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), viewport_h), Sense::click_and_drag());
    if resp.dragged() {
      let d = resp.drag_delta();
      self.cam_yaw_deg = (self.cam_yaw_deg + d.x * 0.5) % 360.0;
      self.cam_pitch_deg = (self.cam_pitch_deg + d.y * 0.4).clamp(-75.0, 75.0);
    }
    if resp.hovered() {
      let scroll = ui.input(|i| i.smooth_scroll_delta.y);
      if scroll.abs() > 0.1 {
        self.cam_zoom = (self.cam_zoom + scroll * 0.003).clamp(0.4, 2.8);
      }
    }
    let painter = ui.painter();
    painter.rect(rect, 0.0, theme::SUNKEN, Stroke::new(1.0, theme::LINE_STRONG), egui::StrokeKind::Inside);
    widgets::corner_marks(painter, rect);
    self.paint_viewport(painter, rect.shrink(8.0));

    ui.add_space(6.0);
    ui.add(egui::TextEdit::singleline(&mut self.shot_prompt).hint_text("Prompt to send with the sketch (optional)").desired_width(f32::INFINITY));
    ui.add_space(6.0);
    ui.horizontal(|ui| {
      if widgets::button(ui, Some(Icon::Image), "Send Sketch to Image", Kind::Accent, 195.0, 34.0).clicked() {
        self.send_sketch(env, None);
      }
      if widgets::button(ui, Some(Icon::Video), "Send Sketch to Video", Kind::Secondary, 195.0, 34.0).clicked() {
        self.send_sketch(env, Some(ImageSlot::FirstFrame));
      }
    });
  }

  /// Right column: pose presets, joints and the camera.
  fn pose_column(&mut self, ui: &mut Ui) {
    widgets::section_label(ui, "03 / Pose Presets", theme::ACCENT_INK);
    for preset in PosePreset::ALL {
      let selected = self.pose_preset == preset;
      if widgets::menu_row(ui, widgets::RowLead::Icon(Icon::User), preset.label(), None, selected, false, selected).clicked() {
        self.pose_preset = preset;
        self.pose = MannequinPose::from_preset(preset);
      }
    }

    ui.add_space(8.0);
    widgets::section_label(ui, "04 / Joints", theme::ACCENT_INK);
    ui.add(egui::Slider::new(&mut self.pose.torso_lean_deg, -45.0..=45.0).text("Torso Lean (°)"));
    ui.add(egui::Slider::new(&mut self.pose.head_yaw_deg, -60.0..=60.0).text("Head Turn (°)"));
    ui.add(egui::Slider::new(&mut self.pose.left_arm_raise_deg, -60.0..=150.0).text("Left Arm (°)"));
    ui.add(egui::Slider::new(&mut self.pose.right_arm_raise_deg, -60.0..=150.0).text("Right Arm (°)"));
    ui.add(egui::Slider::new(&mut self.pose.left_leg_stride_deg, -70.0..=70.0).text("Left Leg Stride (°)"));
    ui.add(egui::Slider::new(&mut self.pose.right_leg_stride_deg, -70.0..=70.0).text("Right Leg Stride (°)"));

    ui.add_space(8.0);
    widgets::section_label(ui, "05 / Camera", theme::ACCENT_INK);
    ui.add(egui::Slider::new(&mut self.focal_length_mm, 18.0..=105.0).text("Focal Length (mm)"));
    ui.add(egui::Slider::new(&mut self.cam_zoom, 0.4..=2.8).text("Zoom"));
  }

  fn send_sketch(&self, env: &mut Env<'_>, video_slot: Option<ImageSlot>) {
    let rgba = self.render_layout_sketch_rgba(SKETCH_W, SKETCH_H);
    let Some(png) = common::encode_rgba_to_png(SKETCH_W, SKETCH_H, &rgba) else {
      env.toasts.error("Couldn't encode the sketch as a PNG");
      return;
    };
    let prompt = Some(self.shot_prompt.trim().to_owned()).filter(|p| !p.is_empty());
    env.requests.push(match video_slot {
      Some(slot) => AppRequest::SendPngToCreateVideo { png, slot, prompt },
      None => AppRequest::SendPngToCreateImage { png, prompt },
    });
  }

  /// The viewport: a floor grid, the props, the mannequin and the sketch frame. Everything is
  /// projected through the 16:9 sketch frame, so the frame shows exactly what the sketch holds.
  fn paint_viewport(&self, painter: &egui::Painter, rect: Rect) {
    let p = painter.with_clip_rect(rect);
    let frame = fit_rect(rect, SKETCH_W as f32 / SKETCH_H as f32);
    let zoom = self.lens_zoom();
    let (yaw, pitch) = (self.cam_yaw_deg, self.cam_pitch_deg);

    for i in -5..=5 {
      let v = i as f32 * 0.4;
      let grid = Stroke::new(1.0, Color32::from_white_alpha(24));
      let (a, _) = project_point_3d([v, FLOOR_Y, -2.0], yaw, pitch, zoom, frame);
      let (b, _) = project_point_3d([v, FLOOR_Y, 2.0], yaw, pitch, zoom, frame);
      p.line_segment([a, b], grid);
      let (c, _) = project_point_3d([-2.0, FLOOR_Y, v], yaw, pitch, zoom, frame);
      let (d, _) = project_point_3d([2.0, FLOOR_Y, v], yaw, pitch, zoom, frame);
      p.line_segment([c, d], grid);
    }

    let mut props: Vec<(Rect, f32, &StageObject)> = self.objects.iter().skip(1).filter(|o| o.visible).map(|o| {
      let (r, depth) = object_rect(o, yaw, pitch, zoom, frame);
      (r, depth, o)
    }).collect();
    props.sort_by(|a, b| b.1.total_cmp(&a.1));
    for (r, _, obj) in props {
      let color = Color32::from_rgb(obj.color[0], obj.color[1], obj.color[2]);
      p.rect(r, 2.0, color.gamma_multiply(0.35), Stroke::new(1.5, color), egui::StrokeKind::Inside);
    }

    if let Some(mannequin) = self.objects.first().filter(|o| o.visible) {
      for (b0, b1) in mannequin_bones(&self.pose, mannequin) {
        let (p0, _) = project_point_3d(b0, yaw, pitch, zoom, frame);
        let (p1, _) = project_point_3d(b1, yaw, pitch, zoom, frame);
        p.line_segment([p0, p1], Stroke::new(4.0, theme::ACCENT_400));
        p.circle_filled(p0, 4.5, Color32::WHITE);
        p.circle_filled(p1, 4.5, Color32::WHITE);
      }
    }

    p.rect_stroke(frame, 0.0, Stroke::new(1.0, Color32::from_white_alpha(45)), egui::StrokeKind::Inside);
    p.text(pos2(frame.left() + 8.0, frame.top() + 12.0), Align2::LEFT_CENTER, format!("SKETCH FRAME · {:.0}MM", self.focal_length_mm), FontId::new(11.5, theme::mono()), theme::ACCENT_INK);
  }

  /// The camera's magnification: the zoom times the focal length relative to 35 mm.
  fn lens_zoom(&self) -> f32 {
    self.cam_zoom * (self.focal_length_mm / 35.0).clamp(0.5, 3.0)
  }
}

impl PosePreset {
  pub const ALL: [Self; 4] = [Self::Standing, Self::Running, Self::PowerStance, Self::Crouched];

  pub fn label(self) -> &'static str {
    match self {
      Self::Standing => "Standing",
      Self::Running => "Running",
      Self::PowerStance => "Power Stance",
      Self::Crouched => "Crouched",
    }
  }
}

impl Default for MannequinPose {
  fn default() -> Self {
    Self::from_preset(PosePreset::PowerStance)
  }
}

impl MannequinPose {
  pub fn from_preset(preset: PosePreset) -> Self {
    let [torso_lean_deg, head_yaw_deg, left_arm_raise_deg, right_arm_raise_deg, left_leg_stride_deg, right_leg_stride_deg] = match preset {
      PosePreset::Standing => [0.0, 0.0, 15.0, 15.0, 0.0, 0.0],
      PosePreset::Running => [24.0, -12.0, 75.0, -45.0, -50.0, 55.0],
      PosePreset::PowerStance => [-8.0, 15.0, 42.0, 115.0, -18.0, 18.0],
      PosePreset::Crouched => [14.0, -8.0, 35.0, 35.0, 65.0, 65.0],
    };
    Self { torso_lean_deg, head_yaw_deg, left_arm_raise_deg, right_arm_raise_deg, left_leg_stride_deg, right_leg_stride_deg }
  }

  /// The stick figure's bones `(start, end)` in the mannequin's own space (unit scale, at the origin).
  pub fn compute_skeleton_bones(&self) -> Vec<([f32; 3], [f32; 3])> {
    let lean = self.torso_lean_deg.to_radians();
    let pelvis = [0.0, -0.25, 0.0];
    let neck = [lean.sin() * 0.55, -0.25 + lean.cos() * 0.55, 0.0];
    let head_top = [neck[0] + self.head_yaw_deg.to_radians().sin() * 0.08, neck[1] + 0.26, 0.0];

    let shoulder_l = [neck[0] - 0.26, neck[1] - 0.05, 0.0];
    let shoulder_r = [neck[0] + 0.26, neck[1] - 0.05, 0.0];
    let la = self.left_arm_raise_deg.to_radians();
    let ra = self.right_arm_raise_deg.to_radians();
    let hand_l = [shoulder_l[0] - la.cos() * 0.48, shoulder_l[1] + la.sin() * 0.48, 0.12];
    let hand_r = [shoulder_r[0] + ra.cos() * 0.48, shoulder_r[1] + ra.sin() * 0.48, 0.12];

    let hip_l = [-0.16, -0.25, 0.0];
    let hip_r = [0.16, -0.25, 0.0];
    let foot_l = [hip_l[0] - 0.05, FLOOR_Y, self.left_leg_stride_deg.to_radians().sin() * 0.45];
    let foot_r = [hip_r[0] + 0.05, FLOOR_Y, self.right_leg_stride_deg.to_radians().sin() * 0.45];

    vec![(pelvis, neck), (neck, head_top), (neck, shoulder_l), (neck, shoulder_r), (shoulder_l, hand_l), (shoulder_r, hand_r), (pelvis, hip_l), (pelvis, hip_r), (hip_l, foot_l), (hip_r, foot_r)]
  }
}

/// The mannequin's bones placed by its stage object (scaled about the floor, then moved).
fn mannequin_bones(pose: &MannequinPose, obj: &StageObject) -> Vec<([f32; 3], [f32; 3])> {
  let place = |p: [f32; 3]| [p[0] * obj.scale + obj.pos[0], (p[1] - FLOOR_Y) * obj.scale + FLOOR_Y + obj.pos[1], p[2] * obj.scale + obj.pos[2]];
  pose.compute_skeleton_bones().into_iter().map(|(a, b)| (place(a), place(b))).collect()
}

/// An object's screen rect (floor to top, a quarter-unit either side) and its depth.
fn object_rect(obj: &StageObject, yaw: f32, pitch: f32, zoom: f32, frame: Rect) -> (Rect, f32) {
  let half = 0.25 * obj.scale;
  let [x, y, z] = obj.pos;
  let (bottom, depth) = project_point_3d([x, FLOOR_Y, z], yaw, pitch, zoom, frame);
  let (top, _) = project_point_3d([x, y + obj.scale, z], yaw, pitch, zoom, frame);
  let (left, _) = project_point_3d([x - half, FLOOR_Y, z], yaw, pitch, zoom, frame);
  let (right, _) = project_point_3d([x + half, FLOOR_Y, z], yaw, pitch, zoom, frame);
  let half_w = ((right - left).length() / 2.0).max(2.0);
  let rect = Rect::from_min_max(pos2(top.x.min(bottom.x) - half_w, top.y.min(bottom.y)), pos2(top.x.max(bottom.x) + half_w, top.y.max(bottom.y)));
  (rect, depth)
}

/// A perspective projection of `pt` (orbiting the origin by yaw, then pitch) into `rect`. Returns
/// the screen position and the depth.
fn project_point_3d(pt: [f32; 3], yaw_deg: f32, pitch_deg: f32, zoom: f32, rect: Rect) -> (Pos2, f32) {
  let (sy, cy) = yaw_deg.to_radians().sin_cos();
  let (sp, cp) = pitch_deg.to_radians().sin_cos();
  let x1 = pt[0] * cy + pt[2] * sy;
  let z1 = -pt[0] * sy + pt[2] * cy;
  let y2 = pt[1] * cp - z1 * sp;
  let depth = (pt[1] * sp + z1 * cp + CAMERA_DISTANCE).max(0.5);
  let scale = (rect.width().min(rect.height()) * 0.42 * zoom) / depth;
  (pos2(rect.center().x + x1 * scale, rect.center().y - y2 * scale), depth)
}

/// Fills an object's rect in the sketch: its cutout image (alpha-blended) or its flat color.
fn fill_object(out: &mut [u8], width: u32, height: u32, rect: Rect, obj: &StageObject) {
  let min_x = rect.left().floor().clamp(0.0, width as f32) as u32;
  let max_x = rect.right().ceil().clamp(0.0, width as f32) as u32;
  let min_y = rect.top().floor().clamp(0.0, height as f32) as u32;
  let max_y = rect.bottom().ceil().clamp(0.0, height as f32) as u32;
  // The cutout maps onto the whole rect, even where the rect runs off the sketch.
  let (x0, y0) = (rect.left().floor(), rect.top().floor());
  let span_x = rect.width().ceil().max(1.0);
  let span_y = rect.height().ceil().max(1.0);
  for y in min_y..max_y {
    for x in min_x..max_x {
      let dst = ((y * width + x) * 4) as usize;
      match &obj.cutout_rgba {
        Some((cw, ch, cpx)) => {
          let sx = (((x as f32 - x0) / span_x * *cw as f32) as u32).min(cw.saturating_sub(1));
          let sy = (((y as f32 - y0) / span_y * *ch as f32) as u32).min(ch.saturating_sub(1));
          let src = ((sy * cw + sx) * 4) as usize;
          let alpha = f32::from(cpx[src + 3]) / 255.0;
          for c in 0..3 {
            out[dst + c] = (f32::from(cpx[src + c]) * alpha + f32::from(out[dst + c]) * (1.0 - alpha)).round() as u8;
          }
        },
        None => out[dst..dst + 3].copy_from_slice(&obj.color),
      }
    }
  }
}

/// Draws a round-capped line of `radius` from `a` to `b` into an RGBA8 buffer.
fn stamp_line(out: &mut [u8], width: u32, height: u32, a: Pos2, b: Pos2, radius: f32, color: [u8; 4]) {
  let steps = ((b - a).length() / (radius * 0.5).max(0.5)).ceil().max(1.0) as u32;
  for step in 0..=steps {
    let center = a + (b - a) * (step as f32 / steps as f32);
    let min_x = (center.x - radius).floor().max(0.0) as u32;
    let min_y = (center.y - radius).floor().max(0.0) as u32;
    let max_x = ((center.x + radius).ceil().max(0.0) as u32).min(width);
    let max_y = ((center.y + radius).ceil().max(0.0) as u32).min(height);
    for y in min_y..max_y {
      for x in min_x..max_x {
        if (pos2(x as f32 + 0.5, y as f32 + 0.5) - center).length() <= radius {
          let i = ((y * width + x) * 4) as usize;
          out[i..i + 4].copy_from_slice(&color);
        }
      }
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn pose_presets_change_the_skeleton() {
    let hero = MannequinPose::from_preset(PosePreset::PowerStance).compute_skeleton_bones();
    let run = MannequinPose::from_preset(PosePreset::Running).compute_skeleton_bones();
    assert_eq!(hero.len(), 10);
    assert_ne!(hero, run);
  }

  mod sketch {
    use super::*;

    #[test]
    fn cutout_cards_keep_their_image_and_show_in_the_sketch() {
      let mut page = Stage3dPage::default();
      let before = page.render_layout_sketch_rgba(64, 36);
      page.add_cutout_card("Red Cutout".into(), 4, 4, [250, 20, 30, 255].repeat(4 * 4));
      let added = page.objects.last().expect("cutout object");
      assert_eq!(added.color, [250, 20, 30]);
      assert!(added.cutout_rgba.is_some());
      assert_ne!(before, page.render_layout_sketch_rgba(64, 36));
    }

    #[test]
    fn moving_the_mannequin_moves_it_in_the_sketch() {
      let mut page = Stage3dPage::default();
      let before = page.render_layout_sketch_rgba(SKETCH_W, SKETCH_H);
      page.objects[0].pos[0] = 1.5;
      assert_ne!(before, page.render_layout_sketch_rgba(SKETCH_W, SKETCH_H));
    }

    #[test]
    fn hidden_objects_are_left_out_of_the_sketch() {
      let mut page = Stage3dPage::default();
      for obj in &mut page.objects {
        obj.visible = false;
      }
      let empty = page.render_layout_sketch_rgba(SKETCH_W, SKETCH_H);
      for row in empty.chunks_exact(SKETCH_W as usize * 4) {
        assert!(row.chunks_exact(4).all(|px| px == &row[..4]), "only the backdrop gradient is left");
      }
    }
  }
}
