//! Piecewise-linear, 8-bit RGB curves for the local editor. The master curve is applied before
//! individual red, green and blue curves; alpha and source layer pixels are never changed.

use egui::{Color32, Pos2, Rect, Sense, Stroke, Ui, pos2, vec2};
use serde::{Deserialize, Serialize};

use crate::theme;

pub const MAX_CONTROL_POINTS: usize = 32;
const CHANNEL_NAMES: [&str; 4] = ["RGB", "Red", "Green", "Blue"];

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Curves {
  /// Master RGB, red, green and blue, in that order.
  pub channels: [Curve; 4],
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Curve {
  pub points: Vec<CurvePoint>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CurvePoint {
  pub input: u8,
  pub output: u8,
}

#[derive(Clone, Default)]
struct CurveUiState {
  channel: usize,
  selected: usize,
}

impl Default for Curves {
  fn default() -> Self {
    Self { channels: std::array::from_fn(|_| Curve::default()) }
  }
}

impl Default for Curve {
  fn default() -> Self {
    Self { points: vec![CurvePoint { input: 0, output: 0 }, CurvePoint { input: 255, output: 255 }] }
  }
}

impl Curves {
  /// Validate persisted or programmatically supplied parameters before accepting a document.
  /// Integer coordinates cannot carry NaN/Infinity; x must increase strictly from 0 to 255.
  pub fn validate(&self) -> Result<(), String> {
    for (index, curve) in self.channels.iter().enumerate() {
      curve.validate().map_err(|error| format!("{} curve: {error}", CHANNEL_NAMES[index]))?;
    }
    Ok(())
  }

  /// Invalid programmatic values are a no-op as a final panic guard. Document loading and
  /// compositing must call `validate` to report invalid parameters to the user.
  pub fn apply(&self, rgba: &mut [u8]) {
    if self.validate().is_err() {
      return;
    }
    let tables: [[u8; 256]; 4] = std::array::from_fn(|index| self.channels[index].lookup());
    for pixel in rgba.chunks_exact_mut(4) {
      for channel in 0..3 {
        pixel[channel] = tables[channel + 1][tables[0][pixel[channel] as usize] as usize];
      }
    }
  }

  /// Pixel history accounts for heap allocations, including spare point capacity.
  pub fn heap_bytes(&self) -> usize {
    self.channels.iter().fold(0usize, |bytes, curve| bytes.saturating_add(curve.points.capacity().saturating_mul(std::mem::size_of::<CurvePoint>())))
  }

  /// Returns true only when curve data changes. Channel/node selection is egui view state,
  /// outside the model, so selection alone never changes document equality or adds history.
  pub fn ui(&mut self, ui: &mut Ui) -> bool {
    let id = ui.make_persistent_id("image_curves_ui");
    let mut state = ui.ctx().data_mut(|data| data.get_temp::<CurveUiState>(id).unwrap_or_default());
    state.channel = state.channel.min(3);
    let mut changed = false;
    if let Err(error) = self.validate() {
      ui.colored_label(theme::MUTED, error);
      if ui.button("Reset Invalid Curves").clicked() {
        *self = Self::default();
        return true;
      }
      return false;
    }
    ui.horizontal(|ui| {
      ui.label("Curves");
      egui::ComboBox::from_label("Curve Channel").selected_text(CHANNEL_NAMES[state.channel]).show_ui(ui, |ui| {
        for (channel, name) in CHANNEL_NAMES.iter().enumerate() {
          if ui.selectable_value(&mut state.channel, channel, *name).changed() {
            state.selected = 0;
          }
        }
      });
    });
    let curve = &mut self.channels[state.channel];
    state.selected = state.selected.min(curve.points.len().saturating_sub(1));
    let size = vec2(ui.available_width().clamp(120.0, 280.0), 180.0);
    let (outer, response) = ui.allocate_exact_size(size, Sense::click_and_drag());
    let graph = outer.shrink(8.0);
    if response.clicked() || response.drag_started() {
      // egui reports drag_started only after the drag threshold is crossed. Hit-test the
      // press location, not the already-moved pointer, or dragging a node inserts another one.
      let origin = if response.drag_started() { ui.input(|input| input.pointer.press_origin()).or_else(|| response.interact_pointer_pos()) } else { response.interact_pointer_pos() };
      if let Some(pointer) = origin {
        if let Some(index) = nearest_point(curve, graph, pointer) {
          state.selected = index;
        } else {
          let point = point_from_position(graph, pointer);
          if let Some(index) = curve.points.iter().position(|existing| existing.input == point.input) {
            state.selected = index;
          } else if let Ok(index) = curve.insert_point(point) {
            state.selected = index;
            changed = true;
          }
        }
      }
    }
    if response.dragged() {
      if let Some(pointer) = response.interact_pointer_pos() {
        let point = point_from_position(graph, pointer);
        changed |= curve.move_point(state.selected, point);
      }
    }
    paint_graph(ui, graph, curve, state.channel, state.selected);
    ui.small("Piecewise linear · click to add · drag a point");
    let count = curve.points.len();
    ui.horizontal(|ui| {
      ui.label("Point");
      let mut number = state.selected + 1;
      if ui.add(egui::DragValue::new(&mut number).range(1..=count)).changed() {
        state.selected = number.saturating_sub(1).min(count.saturating_sub(1));
      }
      ui.label(format!("/ {count}"));
    });
    if let Some(selected) = curve.points.get(state.selected).copied() {
      let mut point = selected;
      ui.horizontal(|ui| {
        let input_label = ui.label("Input");
        let interior = state.selected > 0 && state.selected + 1 < count;
        ui.add_enabled(interior, egui::DragValue::new(&mut point.input).range(0..=255).speed(1.0)).labelled_by(input_label.id);
        let output_label = ui.label("Output");
        ui.add(egui::DragValue::new(&mut point.output).range(0..=255).speed(1.0)).labelled_by(output_label.id);
      });
      changed |= curve.move_point(state.selected, point);
    }
    ui.horizontal_wrapped(|ui| {
      let removable = state.selected > 0 && state.selected + 1 < count;
      if ui.add_enabled(removable, egui::Button::new("Delete Point")).clicked() && curve.remove_point(state.selected) {
        state.selected = state.selected.saturating_sub(1);
        changed = true;
      }
      if ui.button("Reset Channel").clicked() && *curve != Curve::default() {
        *curve = Curve::default();
        state.selected = 0;
        changed = true;
      }
    });
    if count >= MAX_CONTROL_POINTS {
      ui.small("32-point channel limit; delete an interior point to add another.");
    }
    if ui.button("Reset All Curves").clicked() && *self != Self::default() {
      *self = Self::default();
      state.selected = 0;
      changed = true;
    }
    ui.ctx().data_mut(|data| data.insert_temp(id, state));
    changed
  }
}

impl Curve {
  pub fn validate(&self) -> Result<(), String> {
    if !(2..=MAX_CONTROL_POINTS).contains(&self.points.len()) {
      return Err("expected 2 to 32 control points".into());
    }
    if self.points.first().is_none_or(|point| point.input != 0) || self.points.last().is_none_or(|point| point.input != 255) {
      return Err("the first and last input must be 0 and 255".into());
    }
    if self.points.windows(2).any(|pair| pair[0].input >= pair[1].input) {
      return Err("control-point inputs must increase strictly".into());
    }
    Ok(())
  }

  /// Adds a bounded interior point. Endpoints and duplicate inputs cannot be inserted.
  pub fn insert_point(&mut self, point: CurvePoint) -> Result<usize, String> {
    self.validate()?;
    if self.points.len() >= MAX_CONTROL_POINTS {
      return Err("the curve already has 32 points".into());
    }
    let index = match self.points.binary_search_by_key(&point.input, |point| point.input) {
      Ok(_) => return Err("a point already has this input".into()),
      Err(index) => index,
    };
    if index == 0 || index >= self.points.len() {
      return Err("only interior points can be added".into());
    }
    self.points.try_reserve(1).map_err(|_| "Couldn't allocate another curve point")?;
    self.points.insert(index, point);
    Ok(index)
  }

  /// Moves a node without crossing its neighbors. Endpoint x positions remain fixed.
  pub fn move_point(&mut self, index: usize, mut point: CurvePoint) -> bool {
    if self.validate().is_err() || index >= self.points.len() {
      return false;
    }
    if index == 0 {
      point.input = 0;
    } else if index + 1 == self.points.len() {
      point.input = 255;
    } else {
      let lower = self.points[index - 1].input.saturating_add(1);
      let upper = self.points[index + 1].input.saturating_sub(1);
      point.input = point.input.clamp(lower, upper);
    }
    if self.points[index] == point {
      return false;
    }
    self.points[index] = point;
    true
  }

  pub fn remove_point(&mut self, index: usize) -> bool {
    if self.validate().is_err() || index == 0 || index >= self.points.len().saturating_sub(1) {
      return false;
    }
    self.points.remove(index);
    true
  }

  fn lookup(&self) -> [u8; 256] {
    std::array::from_fn(|value| {
      for pair in self.points.windows(2) {
        let (left, right) = (pair[0], pair[1]);
        if value <= right.input as usize && right.input > left.input {
          let t = (value as f64 - left.input as f64) / (right.input as f64 - left.input as f64);
          return (left.output as f64 + (right.output as f64 - left.output as f64) * t).clamp(0.0, 255.0).round() as u8;
        }
      }
      value as u8
    })
  }
}

fn paint_graph(ui: &Ui, graph: Rect, curve: &Curve, channel: usize, selected: usize) {
  let painter = ui.painter();
  painter.rect_filled(graph.expand(8.0), 2.0, theme::SUNKEN);
  for division in 0..=4 {
    let t = division as f32 / 4.0;
    let x = egui::lerp(graph.x_range(), t);
    let y = egui::lerp(graph.y_range(), t);
    painter.line_segment([pos2(x, graph.top()), pos2(x, graph.bottom())], Stroke::new(1.0, theme::LINE));
    painter.line_segment([pos2(graph.left(), y), pos2(graph.right(), y)], Stroke::new(1.0, theme::LINE));
  }
  painter.line_segment([graph.left_bottom(), graph.right_top()], Stroke::new(1.0, theme::LINE_STRONG));
  let color = match channel {
    1 => Color32::from_rgb(246, 113, 113),
    2 => Color32::from_rgb(112, 211, 142),
    3 => Color32::from_rgb(116, 170, 255),
    _ => theme::ACCENT_INK,
  };
  for pair in curve.points.windows(2) {
    painter.line_segment([position_from_point(graph, pair[0]), position_from_point(graph, pair[1])], Stroke::new(2.0, color));
  }
  for (index, point) in curve.points.iter().enumerate() {
    let center = position_from_point(graph, *point);
    painter.circle_filled(center, if index == selected { 4.5 } else { 3.0 }, color);
    if index == selected {
      painter.circle_stroke(center, 6.0, Stroke::new(1.0, Color32::WHITE));
    }
  }
}

fn nearest_point(curve: &Curve, graph: Rect, pointer: Pos2) -> Option<usize> {
  curve.points.iter().enumerate().map(|(index, point)| (index, position_from_point(graph, *point).distance_sq(pointer))).filter(|(_, distance)| *distance <= 12.0 * 12.0).min_by(|left, right| left.1.total_cmp(&right.1)).map(|(index, _)| index)
}

fn position_from_point(graph: Rect, point: CurvePoint) -> Pos2 {
  pos2(graph.left() + graph.width() * point.input as f32 / 255.0, graph.bottom() - graph.height() * point.output as f32 / 255.0)
}

fn point_from_position(graph: Rect, point: Pos2) -> CurvePoint {
  let x = ((point.x - graph.left()) / graph.width().max(1.0)).clamp(0.0, 1.0);
  let y = ((graph.bottom() - point.y) / graph.height().max(1.0)).clamp(0.0, 1.0);
  CurvePoint { input: (x * 255.0).round() as u8, output: (y * 255.0).round() as u8 }
}

#[cfg(test)]
mod tests {
  use super::*;
  use egui_kittest::{Harness, kittest::Queryable};

  #[test]
  fn identity_preserves_all_256_values_alpha_and_trailing_bytes() {
    let mut rgba: Vec<u8> = (0..=255).flat_map(|value| [value, 255 - value, value, value]).collect();
    rgba.extend_from_slice(&[11, 22, 33]);
    let original = rgba.clone();
    Curves::default().apply(&mut rgba);
    assert_eq!(rgba, original);
    Curves::default().apply(&mut []);
  }

  #[test]
  fn master_interpolates_arbitrary_points_and_preserves_alpha() {
    let mut curves = Curves::default();
    curves.channels[0].insert_point(CurvePoint { input: 64, output: 128 }).unwrap();
    let mut rgba = [0, 32, 64, 7, 128, 192, 255, 199];
    curves.apply(&mut rgba);
    assert_eq!(rgba, [0, 64, 128, 7, 171, 213, 255, 199]);
  }

  #[test]
  fn individual_channels_are_independent_and_follow_master() {
    let mut curves = Curves::default();
    curves.channels[0].insert_point(CurvePoint { input: 128, output: 64 }).unwrap();
    curves.channels[1].insert_point(CurvePoint { input: 64, output: 192 }).unwrap();
    curves.channels[2].points[0].output = 255;
    curves.channels[2].points[1].output = 0;
    let mut rgba = [128, 128, 128, 85];
    curves.apply(&mut rgba);
    assert_eq!(rgba, [192, 191, 64, 85]);
  }

  #[test]
  fn descending_segments_and_changed_endpoints_work() {
    let mut curves = Curves::default();
    curves.channels[0].points = vec![CurvePoint { input: 0, output: 240 }, CurvePoint { input: 128, output: 80 }, CurvePoint { input: 255, output: 20 }];
    let mut rgba = [0, 64, 128, 0, 192, 254, 255, 255];
    curves.apply(&mut rgba);
    assert_eq!(rgba, [240, 160, 80, 0, 50, 20, 20, 255]);
  }

  #[test]
  fn malformed_order_duplicates_endpoints_and_point_counts_are_rejected() {
    let mut curves = Curves::default();
    for points in [vec![], vec![CurvePoint { input: 0, output: 0 }], vec![CurvePoint { input: 1, output: 0 }, CurvePoint { input: 255, output: 255 }], vec![CurvePoint { input: 0, output: 0 }, CurvePoint { input: 0, output: 255 }, CurvePoint { input: 255, output: 255 }], vec![CurvePoint { input: 0, output: 0 }, CurvePoint { input: 128, output: 0 }, CurvePoint { input: 64, output: 255 }, CurvePoint { input: 255, output: 255 }]] {
      curves.channels[2].points = points;
      assert!(curves.validate().is_err());
      let mut rgba = [1, 2, 3, 4];
      curves.apply(&mut rgba);
      assert_eq!(rgba, [1, 2, 3, 4]);
    }
    curves.channels[2].points = (0..=32).map(|value| CurvePoint { input: if value == 32 { 255 } else { value }, output: value }).collect();
    assert!(curves.validate().is_err());
  }

  #[test]
  fn moving_and_deleting_nodes_preserves_order_and_endpoints() {
    let mut curve = Curve::default();
    let first = curve.insert_point(CurvePoint { input: 64, output: 80 }).unwrap();
    let second = curve.insert_point(CurvePoint { input: 128, output: 100 }).unwrap();
    assert!(curve.move_point(first, CurvePoint { input: 255, output: 90 }));
    assert_eq!(curve.points[first], CurvePoint { input: 127, output: 90 });
    assert!(curve.move_point(second, CurvePoint { input: 0, output: 120 }));
    assert_eq!(curve.points[second], CurvePoint { input: 128, output: 120 });
    assert!(curve.move_point(0, CurvePoint { input: 30, output: 20 }));
    assert_eq!(curve.points[0], CurvePoint { input: 0, output: 20 });
    assert!(!curve.remove_point(0));
    assert!(!curve.remove_point(curve.points.len() - 1));
    assert!(!curve.remove_point(usize::MAX));
    assert!(curve.remove_point(1));
    assert!(curve.validate().is_ok());
    assert!(!curve.move_point(usize::MAX, CurvePoint { input: 0, output: 0 }));
  }

  #[test]
  fn insertions_are_bounded_without_replacing_existing_points() {
    let mut curve = Curve::default();
    for input in 1..=30 {
      curve.insert_point(CurvePoint { input, output: input }).unwrap();
    }
    let before = curve.clone();
    assert!(curve.insert_point(CurvePoint { input: 31, output: 31 }).is_err());
    assert!(curve.insert_point(CurvePoint { input: 0, output: 123 }).is_err());
    assert_eq!(curve, before);
  }

  #[test]
  fn serialized_curves_round_trip_and_reject_invalid_scalar_values() {
    let mut curves = Curves::default();
    curves.channels[3].insert_point(CurvePoint { input: 100, output: 200 }).unwrap();
    let json = serde_json::to_vec(&curves).unwrap();
    let restored: Curves = serde_json::from_slice(&json).unwrap();
    assert_eq!(restored, curves);
    assert!(restored.validate().is_ok());
    for json in [r#"{"input":256,"output":0}"#, r#"{"input":-1,"output":0}"#, r#"{"input":0.5,"output":0}"#, r#"{"input":0,"output":0,"unknown":1}"#] {
      assert!(serde_json::from_str::<CurvePoint>(json).is_err());
    }
    let minimum = 4 * 2 * std::mem::size_of::<CurvePoint>();
    assert!(curves.heap_bytes() >= minimum);
  }

  #[test]
  fn graph_coordinates_round_trip_and_clamp_outside_pointer_positions() {
    let graph = Rect::from_min_size(pos2(10.0, 20.0), vec2(260.0, 180.0));
    for point in [CurvePoint { input: 0, output: 0 }, CurvePoint { input: 255, output: 255 }, CurvePoint { input: 64, output: 192 }] {
      assert_eq!(point_from_position(graph, position_from_point(graph, point)), point);
    }
    assert_eq!(point_from_position(graph, graph.left_top() - vec2(100.0, 100.0)), CurvePoint { input: 0, output: 255 });
    assert_eq!(point_from_position(graph, graph.right_bottom() + vec2(100.0, 100.0)), CurvePoint { input: 255, output: 0 });
  }

  #[test]
  fn selecting_a_channel_changes_only_view_state() {
    let mut harness = curves_harness();
    assert_eq!(harness.get_by_label("Curve Channel").value().as_deref(), Some("RGB"));
    harness.get_by_label("Curve Channel").click();
    harness.run();
    harness.get_by_label("Red").click();
    harness.run();
    assert_eq!(harness.get_by_label("Curve Channel").value().as_deref(), Some("Red"));
    assert_eq!(harness.state().0, Curves::default());
    assert!(!harness.state().1, "channel selection must not create an edit");
  }

  #[test]
  fn graph_click_adds_and_drag_moves_a_real_control_point() {
    let mut harness = curves_harness();
    let graph = harness
      .output()
      .shapes
      .iter()
      .find_map(|clipped| match &clipped.shape {
        egui::epaint::Shape::Rect(shape) if shape.fill == theme::SUNKEN && (shape.rect.height() - 180.0).abs() < 0.5 => Some(shape.rect.shrink(8.0)),
        _ => None,
      })
      .expect("curves graph drawn");
    let added = CurvePoint { input: 128, output: 200 };
    let start = position_from_point(graph, added);
    harness.hover_at(start);
    harness.drag_at(start);
    harness.run();
    harness.drop_at(start);
    harness.run();
    assert_eq!(harness.state().0.channels[0].points.len(), 3);
    assert_eq!(harness.state().0.channels[0].points[1], added);
    assert!(harness.state().1);
    let moved = CurvePoint { input: 160, output: 90 };
    let end = position_from_point(graph, moved);
    harness.hover_at(start);
    harness.drag_at(start);
    harness.run();
    harness.hover_at(end);
    harness.run();
    harness.drop_at(end);
    harness.run();
    assert_eq!(harness.state().0.channels[0].points.len(), 3, "dragging an existing node must not add a new one");
    assert_eq!(harness.state().0.channels[0].points[1], moved);
    assert!(harness.state().0.validate().is_ok());
  }

  fn curves_harness() -> Harness<'static, (Curves, bool)> {
    let mut harness = Harness::builder().with_size(vec2(400.0, 600.0)).build_ui_state(
      |ui, state: &mut (Curves, bool)| {
        state.1 |= state.0.ui(ui);
      },
      (Curves::default(), false),
    );
    harness.run();
    harness
  }
}
