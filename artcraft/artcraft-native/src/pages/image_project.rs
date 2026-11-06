//! Versioned, lossless local image projects. Metadata is bounded JSON; unique pixel buffers are
//! stored once as raw bytes, preserving transparent RGB and shared layers without resampling.

use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::io::{Read, Write};
use std::path::Path;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tempfile::NamedTempFile;

use crate::pages::curves::Curves;
use crate::pages::edit_image::{BlendMode, CanvasLayer, LayerMask, LayerTransform, MAX_DOCUMENT_BYTES, checked_rgba_len, validate_layer};
use crate::pages::levels::Levels;

pub const PROJECT_EXTENSION: &str = "acraft";
pub const MAX_PROJECT_LAYERS: usize = 256;
const MAGIC: [u8; 8] = *b"ACRFTIMG";
const VERSION: u32 = 1;
const HEADER_BYTES: usize = 16;
const MAX_METADATA_BYTES: usize = 1024 * 1024;
const MAX_NAME_BYTES: usize = 4096;
const MAX_BLOBS: usize = MAX_PROJECT_LAYERS * 2;

#[derive(Clone, Debug, PartialEq)]
pub struct ImageProject {
  pub width: u32,
  pub height: u32,
  pub layers: Vec<CanvasLayer>,
  pub active_layer: usize,
  pub exposure: f32,
  pub contrast: f32,
  pub saturation: f32,
  pub levels: Levels,
  pub levels_preview: bool,
  pub curves: Curves,
  pub curves_preview: bool,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Metadata {
  width: u32,
  height: u32,
  layers: Vec<LayerMetadata>,
  blobs: Vec<BlobMetadata>,
  active_layer: usize,
  exposure: f32,
  contrast: f32,
  saturation: f32,
  levels: LevelsMetadata,
  levels_preview: bool,
  curves: Curves,
  curves_preview: bool,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct LayerMetadata {
  name: String,
  visible: bool,
  opacity: f32,
  blend: BlendMetadata,
  width: u32,
  height: u32,
  pixels: usize,
  mask: Option<MaskMetadata>,
  transform: TransformMetadata,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct BlobMetadata {
  bytes: u64,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct MaskMetadata {
  enabled: bool,
  pixels: usize,
}

#[derive(Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum BlendMetadata {
  Normal,
  Multiply,
  Screen,
  Overlay,
  Add,
}

#[derive(Clone, Copy, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct TransformMetadata {
  offset: [f64; 2],
  rotation_deg: f64,
  scale: [f64; 2],
  flip_x: bool,
  flip_y: bool,
}

#[derive(Clone, Copy, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct LevelsMetadata {
  input_black: u8,
  input_white: u8,
  gamma: f32,
  output_black: u8,
  output_white: u8,
}

impl ImageProject {
  /// Validate before publishing a project into the editor. The budget counts each shared pixel
  /// allocation once, including spare capacity, just like the editor's current-document budget.
  pub fn validate(&self) -> Result<(), String> {
    checked_rgba_len(self.width, self.height)?;
    validate_layer_count(self.layers.len(), self.active_layer)?;
    validate_adjustments(self.exposure, self.contrast, self.saturation, &self.levels, &self.curves)?;
    let mut seen = HashSet::new();
    let mut bytes = 0usize;
    for layer in &self.layers {
      validate_layer(layer)?;
      validate_name(&layer.name)?;
      for pixels in std::iter::once(&layer.pixels).chain(layer.mask.iter().map(|mask| &mask.pixels)) {
        if seen.insert(Arc::as_ptr(pixels) as usize) {
          bytes = bytes.checked_add(pixels.capacity()).ok_or("Project pixel memory size overflow")?;
          if bytes > MAX_DOCUMENT_BYTES {
            return Err("Project pixels and masks exceed the 512 MiB editing memory budget".into());
          }
        }
      }
    }
    Ok(())
  }

  /// Load into an independent, validated value. Callers replace their current document only
  /// after this returns successfully; failed loads never expose partially decoded layers.
  pub fn load(path: &Path) -> Result<Self, String> {
    let file_info = std::fs::metadata(path).map_err(|error| format!("Couldn't inspect image project: {error}"))?;
    if !file_info.is_file() {
      return Err("The image project is not a regular file".into());
    }
    let mut file = File::open(path).map_err(|error| format!("Couldn't open image project: {error}"))?;
    let info = file.metadata().map_err(|error| format!("Couldn't inspect open image project: {error}"))?;
    let max_file_bytes = (HEADER_BYTES + MAX_METADATA_BYTES + MAX_DOCUMENT_BYTES) as u64;
    if !info.is_file() || !(HEADER_BYTES as u64..=max_file_bytes).contains(&info.len()) {
      return Err("Image project file size is outside the supported bounds".into());
    }
    let mut header = [0u8; HEADER_BYTES];
    file.read_exact(&mut header).map_err(|error| format!("Couldn't read image project header: {error}"))?;
    if header[..8] != MAGIC {
      return Err("This is not an ArtCraft image project".into());
    }
    let version = u32::from_le_bytes([header[8], header[9], header[10], header[11]]);
    if version != VERSION {
      return Err(format!("Image project version {version} is not supported by this app"));
    }
    let metadata_length = u32::from_le_bytes([header[12], header[13], header[14], header[15]]) as usize;
    if metadata_length == 0 || metadata_length > MAX_METADATA_BYTES {
      return Err("Image project metadata is outside the 1 MiB limit".into());
    }
    let mut metadata_bytes = allocate_bytes(metadata_length)?;
    file.read_exact(&mut metadata_bytes).map_err(|error| format!("Couldn't read image project metadata: {error}"))?;
    let metadata: Metadata = serde_json::from_slice(&metadata_bytes).map_err(|error| format!("Invalid image project metadata: {error}"))?;
    let payload_length = metadata.validate()?;
    let expected = (HEADER_BYTES as u64).checked_add(metadata_length as u64).and_then(|length| length.checked_add(payload_length)).ok_or("Image project file size overflow")?;
    if expected != info.len() {
      return Err("Image project payload is truncated or has unexpected trailing bytes".into());
    }
    let mut blobs = Vec::new();
    blobs.try_reserve_exact(metadata.blobs.len()).map_err(|_| "Not enough memory for image project buffers")?;
    for blob in &metadata.blobs {
      let length = usize::try_from(blob.bytes).map_err(|_| "Image project buffer cannot be addressed")?;
      let mut bytes = allocate_bytes(length)?;
      file.read_exact(&mut bytes).map_err(|error| format!("Couldn't read image project pixels: {error}"))?;
      blobs.push(Arc::new(bytes));
    }
    let mut trailing = [0u8; 1];
    if file.read(&mut trailing).map_err(|error| format!("Couldn't finish reading image project: {error}"))? != 0 {
      return Err("Image project changed while reading or contains trailing data".into());
    }
    let project = metadata.into_project(&blobs)?;
    project.validate()?;
    Ok(project)
  }

  /// Write to a temporary file in the destination folder, sync it, then atomically replace the
  /// destination. No in-place truncation occurs, so a failed save retains the previous project.
  pub fn save(&self, path: &Path) -> Result<(), String> {
    self.save_via(path, write_project)
  }

  fn save_via(&self, path: &Path, write: impl FnOnce(&mut File, &Metadata, &[Arc<Vec<u8>>]) -> Result<(), String>) -> Result<(), String> {
    self.validate()?;
    let (metadata, blobs) = self.metadata();
    metadata.validate()?;
    if path.file_name().is_none() {
      return Err("Choose an image project filename".into());
    }
    let directory = path.parent().filter(|parent| !parent.as_os_str().is_empty()).unwrap_or_else(|| Path::new("."));
    let mut temporary = NamedTempFile::new_in(directory).map_err(|error| format!("Couldn't prepare image project save: {error}"))?;
    write(temporary.as_file_mut(), &metadata, &blobs)?;
    temporary.as_file_mut().flush().map_err(|error| format!("Couldn't finish writing image project: {error}"))?;
    temporary.as_file().sync_all().map_err(|error| format!("Couldn't sync image project: {error}"))?;
    temporary.persist(path).map_err(|error| format!("Couldn't replace image project; the previous file was kept: {}", error.error))?;
    Ok(())
  }

  fn metadata(&self) -> (Metadata, Vec<Arc<Vec<u8>>>) {
    let mut ids = HashMap::new();
    let mut blobs = Vec::new();
    let layers = self.layers.iter().map(|layer| LayerMetadata { name: layer.name.clone(), visible: layer.visible, opacity: layer.opacity, blend: layer.blend.into(), width: layer.width, height: layer.height, pixels: register_blob(&layer.pixels, &mut ids, &mut blobs), mask: layer.mask.as_ref().map(|mask| MaskMetadata { enabled: mask.enabled, pixels: register_blob(&mask.pixels, &mut ids, &mut blobs) }), transform: layer.transform.into() }).collect();
    let metadata = Metadata { width: self.width, height: self.height, layers, blobs: blobs.iter().map(|blob| BlobMetadata { bytes: blob.len() as u64 }).collect(), active_layer: self.active_layer, exposure: self.exposure, contrast: self.contrast, saturation: self.saturation, levels: self.levels.into(), levels_preview: self.levels_preview, curves: self.curves.clone(), curves_preview: self.curves_preview };
    (metadata, blobs)
  }
}

impl Metadata {
  /// Validate lengths and every reference before allocating any image payload.
  fn validate(&self) -> Result<u64, String> {
    checked_rgba_len(self.width, self.height)?;
    validate_layer_count(self.layers.len(), self.active_layer)?;
    validate_adjustments(self.exposure, self.contrast, self.saturation, &self.levels.into(), &self.curves)?;
    if self.blobs.is_empty() || self.blobs.len() > MAX_BLOBS {
      return Err("Image project contains an invalid number of pixel buffers".into());
    }
    let mut payload = 0u64;
    for blob in &self.blobs {
      if blob.bytes == 0 || blob.bytes > MAX_DOCUMENT_BYTES as u64 {
        return Err("Image project pixel buffer is outside the supported bounds".into());
      }
      payload = payload.checked_add(blob.bytes).ok_or("Image project payload size overflow")?;
      if payload > MAX_DOCUMENT_BYTES as u64 {
        return Err("Image project pixels and masks exceed the 512 MiB limit".into());
      }
    }
    let mut used = vec![false; self.blobs.len()];
    for layer in &self.layers {
      validate_name(&layer.name)?;
      let rgba_length = checked_rgba_len(layer.width, layer.height)?;
      let transform: LayerTransform = layer.transform.into();
      transform.validate()?;
      if !layer.opacity.is_finite() || !(0.0..=1.0).contains(&layer.opacity) {
        return Err("Image project layer opacity is invalid".into());
      }
      validate_blob_reference(&self.blobs, &mut used, layer.pixels, rgba_length)?;
      if let Some(mask) = &layer.mask {
        validate_blob_reference(&self.blobs, &mut used, mask.pixels, rgba_length / 4)?;
      }
    }
    if used.iter().any(|used| !used) {
      return Err("Image project contains unreferenced pixel buffers".into());
    }
    Ok(payload)
  }

  fn into_project(self, blobs: &[Arc<Vec<u8>>]) -> Result<ImageProject, String> {
    let mut layers = Vec::new();
    layers.try_reserve_exact(self.layers.len()).map_err(|_| "Not enough memory for image project layers")?;
    for layer in self.layers {
      let pixels = blobs.get(layer.pixels).ok_or("Image project pixel reference is missing")?.clone();
      let mask = layer.mask.map(|mask| Ok::<_, String>(LayerMask { enabled: mask.enabled, pixels: blobs.get(mask.pixels).ok_or("Image project mask reference is missing")?.clone() })).transpose()?;
      layers.push(CanvasLayer { name: layer.name, visible: layer.visible, opacity: layer.opacity, blend: layer.blend.into(), width: layer.width, height: layer.height, pixels, mask, transform: layer.transform.into() });
    }
    Ok(ImageProject { width: self.width, height: self.height, layers, active_layer: self.active_layer, exposure: self.exposure, contrast: self.contrast, saturation: self.saturation, levels: self.levels.into(), levels_preview: self.levels_preview, curves: self.curves, curves_preview: self.curves_preview })
  }
}

fn write_project(file: &mut File, metadata: &Metadata, blobs: &[Arc<Vec<u8>>]) -> Result<(), String> {
  let json = serde_json::to_vec(metadata).map_err(|error| format!("Couldn't encode image project metadata: {error}"))?;
  if json.is_empty() || json.len() > MAX_METADATA_BYTES {
    return Err("Image project metadata exceeds the 1 MiB limit".into());
  }
  file.write_all(&MAGIC).and_then(|()| file.write_all(&VERSION.to_le_bytes())).and_then(|()| file.write_all(&(json.len() as u32).to_le_bytes())).and_then(|()| file.write_all(&json)).map_err(|error| format!("Couldn't write image project metadata: {error}"))?;
  for blob in blobs {
    file.write_all(blob).map_err(|error| format!("Couldn't write image project pixels: {error}"))?;
  }
  Ok(())
}

fn register_blob(pixels: &Arc<Vec<u8>>, ids: &mut HashMap<usize, usize>, blobs: &mut Vec<Arc<Vec<u8>>>) -> usize {
  let identity = Arc::as_ptr(pixels) as usize;
  if let Some(id) = ids.get(&identity) {
    return *id;
  }
  let id = blobs.len();
  ids.insert(identity, id);
  blobs.push(pixels.clone());
  id
}

fn validate_layer_count(count: usize, active: usize) -> Result<(), String> {
  if count == 0 || count > MAX_PROJECT_LAYERS {
    return Err(format!("Image projects support between 1 and {MAX_PROJECT_LAYERS} layers"));
  }
  if active >= count {
    return Err("Image project active layer is outside the layer list".into());
  }
  Ok(())
}

fn validate_name(name: &str) -> Result<(), String> {
  if name.len() > MAX_NAME_BYTES {
    return Err("Image project layer name exceeds 4096 UTF-8 bytes".into());
  }
  Ok(())
}

fn validate_adjustments(exposure: f32, contrast: f32, saturation: f32, levels: &Levels, curves: &Curves) -> Result<(), String> {
  if [exposure, contrast, saturation].iter().any(|value| !value.is_finite() || !(-1.0..=1.0).contains(value)) {
    return Err("Image project adjustments must be finite and between -1 and 1".into());
  }
  if levels.input_black >= levels.input_white || !levels.gamma.is_finite() || !(0.1..=9.99).contains(&levels.gamma) {
    return Err("Image project Levels inputs or gamma are invalid".into());
  }
  curves.validate()
}

fn validate_blob_reference(blobs: &[BlobMetadata], used: &mut [bool], id: usize, expected_length: usize) -> Result<(), String> {
  let blob = blobs.get(id).ok_or("Image project pixel reference is outside its buffer table")?;
  if blob.bytes != expected_length as u64 {
    return Err("Image project dimensions do not match its pixel or mask buffer".into());
  }
  let referenced = used.get_mut(id).ok_or("Image project pixel reference is invalid")?;
  *referenced = true;
  Ok(())
}

fn allocate_bytes(length: usize) -> Result<Vec<u8>, String> {
  if length > MAX_DOCUMENT_BYTES {
    return Err("Image project buffer exceeds the editing memory budget".into());
  }
  let mut bytes = Vec::new();
  bytes.try_reserve_exact(length).map_err(|_| "Not enough memory to load image project")?;
  bytes.resize(length, 0);
  Ok(bytes)
}

impl From<BlendMode> for BlendMetadata {
  fn from(blend: BlendMode) -> Self {
    match blend {
      BlendMode::Normal => Self::Normal,
      BlendMode::Multiply => Self::Multiply,
      BlendMode::Screen => Self::Screen,
      BlendMode::Overlay => Self::Overlay,
      BlendMode::Add => Self::Add,
    }
  }
}

impl From<BlendMetadata> for BlendMode {
  fn from(blend: BlendMetadata) -> Self {
    match blend {
      BlendMetadata::Normal => Self::Normal,
      BlendMetadata::Multiply => Self::Multiply,
      BlendMetadata::Screen => Self::Screen,
      BlendMetadata::Overlay => Self::Overlay,
      BlendMetadata::Add => Self::Add,
    }
  }
}

impl From<LayerTransform> for TransformMetadata {
  fn from(value: LayerTransform) -> Self {
    Self { offset: value.offset, rotation_deg: value.rotation_deg, scale: value.scale, flip_x: value.flip_x, flip_y: value.flip_y }
  }
}

impl From<TransformMetadata> for LayerTransform {
  fn from(value: TransformMetadata) -> Self {
    Self { offset: value.offset, rotation_deg: value.rotation_deg, scale: value.scale, flip_x: value.flip_x, flip_y: value.flip_y }
  }
}

impl From<Levels> for LevelsMetadata {
  fn from(value: Levels) -> Self {
    Self { input_black: value.input_black, input_white: value.input_white, gamma: value.gamma, output_black: value.output_black, output_white: value.output_white }
  }
}

impl From<LevelsMetadata> for Levels {
  fn from(value: LevelsMetadata) -> Self {
    Self { input_black: value.input_black, input_white: value.input_white, gamma: value.gamma, output_black: value.output_black, output_white: value.output_white }
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::pages::curves::CurvePoint;

  #[test]
  fn roundtrip_preserves_nonuniform_rgba_masks_transforms_and_adjustments() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("full.acraft");
    let expected = fixture();
    expected.save(&path).unwrap();
    let actual = ImageProject::load(&path).unwrap();
    assert_eq!(actual, expected);
    assert_eq!(&actual.layers[0].pixels[0..4], &[23, 51, 197, 0], "hidden RGB under zero alpha is lossless");
    assert!(actual.layers[1].mask.as_ref().is_some_and(|mask| !mask.enabled));
  }

  #[test]
  fn shared_buffers_are_written_once_and_arc_sharing_is_restored() {
    let mut project = fixture();
    project.layers.push(project.layers[0].clone());
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("shared.acraft");
    let (metadata, blobs) = project.metadata();
    assert_eq!(metadata.layers[0].pixels, metadata.layers[2].pixels);
    assert_eq!(blobs.len(), 4, "two source RGBA buffers and two masks");
    project.save(&path).unwrap();
    let loaded = ImageProject::load(&path).unwrap();
    assert_eq!(loaded, project);
    assert!(Arc::ptr_eq(&loaded.layers[0].pixels, &loaded.layers[2].pixels));
    assert!(Arc::ptr_eq(&loaded.layers[0].mask.as_ref().unwrap().pixels, &loaded.layers[2].mask.as_ref().unwrap().pixels));
    let payload: u64 = metadata.blobs.iter().map(|blob| blob.bytes).sum();
    let json_length = serde_json::to_vec(&metadata).unwrap().len();
    assert_eq!(std::fs::metadata(path).unwrap().len(), HEADER_BYTES as u64 + json_length as u64 + payload);
  }

  #[test]
  fn every_blend_mode_and_unicode_names_roundtrip() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("혼합.acraft");
    for mode in [BlendMode::Normal, BlendMode::Multiply, BlendMode::Screen, BlendMode::Overlay, BlendMode::Add] {
      let mut project = fixture();
      project.layers[0].blend = mode;
      project.layers[0].name = "레이어 🎨 e\u{301}".into();
      project.save(&path).unwrap();
      assert_eq!(ImageProject::load(&path).unwrap(), project);
    }
  }

  #[test]
  fn invalid_model_does_not_replace_an_existing_file() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("kept.acraft");
    fixture().save(&path).unwrap();
    let before = std::fs::read(&path).unwrap();
    let mut invalid = fixture();
    invalid.active_layer = invalid.layers.len();
    assert!(invalid.save(&path).is_err());
    assert_eq!(std::fs::read(&path).unwrap(), before);
    assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 1);
  }

  #[test]
  fn a_partial_write_failure_keeps_the_old_project_and_removes_the_temporary_file() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("kept.acraft");
    fixture().save(&path).unwrap();
    let before = std::fs::read(&path).unwrap();
    let result = fixture().save_via(&path, |file, _, _| {
      file.write_all(b"partial replacement").unwrap();
      Err("injected write failure".into())
    });
    assert!(result.unwrap_err().contains("injected write failure"));
    assert_eq!(std::fs::read(&path).unwrap(), before);
    assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 1);
    assert_eq!(ImageProject::load(&path).unwrap(), fixture());
  }

  #[test]
  fn failed_atomic_replacement_preserves_the_existing_directory_and_cleans_up() {
    let directory = tempfile::tempdir().unwrap();
    let destination = directory.path().join("kept.acraft");
    std::fs::create_dir(&destination).unwrap();
    std::fs::write(destination.join("keep"), b"user data").unwrap();
    assert!(fixture().save(&destination).is_err());
    assert_eq!(std::fs::read(destination.join("keep")).unwrap(), b"user data");
    assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 1);
  }

  #[test]
  fn header_version_metadata_limits_and_trailing_bytes_are_rejected() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("invalid.acraft");
    fixture().save(&path).unwrap();
    let valid = std::fs::read(&path).unwrap();
    let mut cases = vec![vec![], valid[..15].to_vec(), valid[..valid.len() - 1].to_vec()];
    let mut bad_magic = valid.clone();
    bad_magic[0] = 0;
    cases.push(bad_magic);
    let mut bad_version = valid.clone();
    bad_version[8..12].copy_from_slice(&2u32.to_le_bytes());
    cases.push(bad_version);
    let mut huge_metadata = valid.clone();
    huge_metadata[12..16].copy_from_slice(&((MAX_METADATA_BYTES + 1) as u32).to_le_bytes());
    cases.push(huge_metadata);
    let mut empty_metadata = valid.clone();
    empty_metadata[12..16].copy_from_slice(&0u32.to_le_bytes());
    cases.push(empty_metadata);
    let mut trailing = valid.clone();
    trailing.push(123);
    cases.push(trailing);
    for invalid in cases {
      std::fs::write(&path, invalid).unwrap();
      assert!(ImageProject::load(&path).is_err());
    }
  }

  #[test]
  fn hostile_metadata_is_rejected_before_allocating_pixel_payloads() {
    let mut mutations: Vec<Box<dyn Fn(&mut serde_json::Value)>> = vec![
      Box::new(|value| value["width"] = 0.into()),
      Box::new(|value| value["height"] = u32::MAX.into()),
      Box::new(|value| value["active_layer"] = 900.into()),
      Box::new(|value| value["layers"] = serde_json::json!([])),
      Box::new(|value| value["layers"][0]["pixels"] = 500.into()),
      Box::new(|value| value["layers"][0]["mask"]["pixels"] = 500.into()),
      Box::new(|value| value["layers"][0]["width"] = 9.into()),
      Box::new(|value| value["layers"][0]["opacity"] = 2.into()),
      Box::new(|value| value["layers"][0]["blend"] = "unknown".into()),
      Box::new(|value| value["layers"][0]["transform"]["scale"] = serde_json::json!([0, 1])),
      Box::new(|value| value["layers"][0]["transform"]["offset"] = serde_json::json!([1e30, 0])),
      Box::new(|value| value["layers"][0]["transform"]["rotation_deg"] = serde_json::Value::Null),
      Box::new(|value| value["levels"]["input_black"] = 255.into()),
      Box::new(|value| value["levels"]["gamma"] = 0.into()),
      Box::new(|value| value["exposure"] = 3.into()),
      Box::new(|value| value["curves"]["channels"][0]["points"][1]["input"] = 0.into()),
      Box::new(|value| value["unexpected"] = true.into()),
      Box::new(|value| value["blobs"][0]["bytes"] = (MAX_DOCUMENT_BYTES as u64 + 1).into()),
      Box::new(|value| value["blobs"] = serde_json::json!([])),
    ];
    mutations.push(Box::new(|value| value["layers"][0]["name"] = "x".repeat(MAX_NAME_BYTES + 1).into()));
    for mutate in mutations {
      let (metadata, _) = fixture().metadata();
      let mut value = serde_json::to_value(metadata).unwrap();
      mutate(&mut value);
      assert_invalid_metadata(value);
    }
  }

  #[test]
  fn unreferenced_buffers_and_mask_length_aliases_are_rejected() {
    let (mut metadata, _) = fixture().metadata();
    metadata.blobs.push(BlobMetadata { bytes: 1 });
    assert!(metadata.validate().unwrap_err().contains("unreferenced"));
    let (mut metadata, _) = fixture().metadata();
    metadata.layers[0].mask.as_mut().unwrap().pixels = metadata.layers[0].pixels;
    assert!(metadata.validate().unwrap_err().contains("dimensions"));
  }

  #[test]
  fn nonfinite_and_out_of_range_in_memory_parameters_are_rejected() {
    let mut project = fixture();
    project.layers[0].transform.rotation_deg = f64::NAN;
    assert!(project.validate().is_err());
    project = fixture();
    project.layers[0].opacity = f32::INFINITY;
    assert!(project.validate().is_err());
    project = fixture();
    project.levels.gamma = f32::NAN;
    assert!(project.validate().is_err());
    project = fixture();
    project.contrast = f32::NEG_INFINITY;
    assert!(project.validate().is_err());
  }

  #[test]
  fn layer_and_blob_counts_are_bounded_without_large_allocations() {
    let mut project = fixture();
    project.layers.resize(MAX_PROJECT_LAYERS + 1, project.layers[0].clone());
    assert!(project.validate().is_err());
    let (mut metadata, _) = fixture().metadata();
    metadata.blobs.resize(MAX_BLOBS + 1, BlobMetadata { bytes: 1 });
    assert!(metadata.validate().is_err());
    let (mut metadata, _) = fixture().metadata();
    metadata.blobs[0].bytes = MAX_DOCUMENT_BYTES as u64;
    assert!(metadata.validate().unwrap_err().contains("512 MiB"));
  }

  fn fixture() -> ImageProject {
    let mut curves = Curves::default();
    curves.channels[0].points.insert(1, CurvePoint { input: 121, output: 173 });
    curves.channels[2].points[0].output = 11;
    let first = CanvasLayer { name: "原本 레이어".into(), visible: true, opacity: 0.73, blend: BlendMode::Overlay, width: 3, height: 2, pixels: Arc::new(vec![23, 51, 197, 0, 255, 1, 7, 23, 1, 2, 3, 255, 190, 44, 13, 128, 11, 245, 76, 200, 99, 49, 199, 77]), mask: Some(LayerMask { enabled: true, pixels: Arc::new(vec![0, 64, 128, 200, 254, 255]) }), transform: LayerTransform { offset: [-23.25, 41.5], rotation_deg: 32.5, scale: [0.8, 1.2], flip_x: true, flip_y: false } };
    let second = CanvasLayer { name: "hidden".into(), visible: false, opacity: 0.5, blend: BlendMode::Multiply, width: 1, height: 1, pixels: Arc::new(vec![91, 87, 34, 27]), mask: Some(LayerMask { enabled: false, pixels: Arc::new(vec![71]) }), transform: LayerTransform::default() };
    ImageProject { width: 803, height: 601, layers: vec![first, second], active_layer: 1, exposure: -0.2, contrast: 0.4, saturation: 0.7, levels: Levels { input_black: 12, input_white: 241, gamma: 1.4, output_black: 200, output_white: 20 }, levels_preview: false, curves, curves_preview: true }
  }

  fn assert_invalid_metadata(value: serde_json::Value) {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("invalid.acraft");
    let json = serde_json::to_vec(&value).unwrap();
    let mut file = File::create(&path).unwrap();
    file.write_all(&MAGIC).unwrap();
    file.write_all(&VERSION.to_le_bytes()).unwrap();
    file.write_all(&(json.len() as u32).to_le_bytes()).unwrap();
    file.write_all(&json).unwrap();
    drop(file);
    assert!(ImageProject::load(&path).is_err(), "hostile metadata was accepted: {value}");
  }
}
