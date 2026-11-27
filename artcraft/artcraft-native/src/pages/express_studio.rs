//! Express Quick Actions & Brand Kit Studio integration for ArtCraft Native:
//! - Brand Kit palette/font preset applier (`ExpressBrandKit`, `BRAND_KIT_PRESETS`),
//! - Social Channel Smart Resize presets (`SocialResizePreset`: Instagram Post, Instagram Reel,
//!   YouTube Thumbnail, LinkedIn Banner, Presentation 16:9),
//! - 1-click `craft://` handoff URI & JSON metadata builder (`CraftHandoffMetadata`) validated
//!   against `SUITE_APPS`.

use std::io::Cursor;

use egui::{Color32, Sense, Ui, vec2};
use image::{ImageFormat, RgbaImage};
use serde_json::{Value, json};

use crate::pages::common::{AppRequest, Env};
use crate::pages::suite_registry::SUITE_APPS;
use crate::theme;
use crate::ui::widgets;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExpressBrandKit {
  pub name: String,
  pub primary_hex: String,
  pub secondary_hex: String,
  pub accent_hex: String,
  pub heading_font: String,
  pub body_font: String,
}

impl Default for ExpressBrandKit {
  fn default() -> Self {
    Self::from_preset(BrandKitPreset::StudioIndigo)
  }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BrandKitPreset {
  StudioIndigo,
  EditorialWarm,
  CyberNeon,
  BotanicalLuxe,
}

pub const BRAND_KIT_PRESETS: [BrandKitPreset; 4] = [BrandKitPreset::StudioIndigo, BrandKitPreset::EditorialWarm, BrandKitPreset::CyberNeon, BrandKitPreset::BotanicalLuxe];

impl BrandKitPreset {
  pub fn label(self) -> &'static str {
    match self {
      Self::StudioIndigo => "Studio Indigo",
      Self::EditorialWarm => "Editorial Warm",
      Self::CyberNeon => "Cyber Neon",
      Self::BotanicalLuxe => "Botanical Luxe",
    }
  }
}

pub fn parse_hex_rgb(hex: &str) -> Result<[u8; 3], String> {
  let trimmed = hex.trim();
  let raw = trimmed.strip_prefix('#').unwrap_or(trimmed);
  if raw.len() != 6 || !raw.bytes().all(|b| b.is_ascii_hexdigit()) {
    return Err(format!("Expected a 6-digit #RRGGBB hex color, got `{hex}`"));
  }
  let r = u8::from_str_radix(&raw[0..2], 16).map_err(|e| e.to_string())?;
  let g = u8::from_str_radix(&raw[2..4], 16).map_err(|e| e.to_string())?;
  let b = u8::from_str_radix(&raw[4..6], 16).map_err(|e| e.to_string())?;
  Ok([r, g, b])
}

pub fn normalize_hex_rgb(hex: &str) -> Result<String, String> {
  let [r, g, b] = parse_hex_rgb(hex)?;
  Ok(format!("#{r:02X}{g:02X}{b:02X}"))
}

pub type BrandRgbTriplet = ([u8; 3], [u8; 3], [u8; 3]);

impl ExpressBrandKit {
  pub fn from_preset(preset: BrandKitPreset) -> Self {
    match preset {
      BrandKitPreset::StudioIndigo => Self { name: String::from("Studio Indigo"), primary_hex: String::from("#0F172A"), secondary_hex: String::from("#F8FAFC"), accent_hex: String::from("#4F46E5"), heading_font: String::from("Montserrat"), body_font: String::from("Inter") },
      BrandKitPreset::EditorialWarm => Self { name: String::from("Editorial Warm"), primary_hex: String::from("#1C1917"), secondary_hex: String::from("#FFFBEB"), accent_hex: String::from("#EA580C"), heading_font: String::from("Playfair Display"), body_font: String::from("Source Sans 3") },
      BrandKitPreset::CyberNeon => Self { name: String::from("Cyber Neon"), primary_hex: String::from("#F1F5F9"), secondary_hex: String::from("#090D16"), accent_hex: String::from("#06B6D4"), heading_font: String::from("Space Grotesk"), body_font: String::from("IBM Plex Sans") },
      BrandKitPreset::BotanicalLuxe => Self { name: String::from("Botanical Luxe"), primary_hex: String::from("#143127"), secondary_hex: String::from("#F4F7F2"), accent_hex: String::from("#C28E36"), heading_font: String::from("Cormorant Garamond"), body_font: String::from("Plus Jakarta Sans") },
    }
  }

  pub fn validate(&self) -> Result<BrandRgbTriplet, String> {
    let p = parse_hex_rgb(&self.primary_hex)?;
    let s = parse_hex_rgb(&self.secondary_hex)?;
    let a = parse_hex_rgb(&self.accent_hex)?;
    if self.heading_font.trim().is_empty() || self.body_font.trim().is_empty() {
      return Err(String::from("Heading and body font families must be non-empty"));
    }
    Ok((p, s, a))
  }

  /// Synthesize a structured Firefly / image-generation brand style directive.
  pub fn style_prompt_directive(&self, social: SocialResizePreset) -> Result<String, String> {
    let _ = self.validate()?;
    let (w, h) = social.dimensions_px();
    Ok(format!("Brand Kit \"{}\": palette primary {}, background {}, accent {}; typography {} headings with {} body; framed for {} ({}x{} px, {} aspect).", self.name.trim(), normalize_hex_rgb(&self.primary_hex)?, normalize_hex_rgb(&self.secondary_hex)?, normalize_hex_rgb(&self.accent_hex)?, self.heading_font.trim(), self.body_font.trim(), social.label(), w, h, social.aspect_label(),))
  }

  /// Rasterize a clean RGBA8 Brand Kit reference card (`width` x `height`) showing background,
  /// primary header band, and accent geometric callout for visual reference handoff.
  pub fn render_brand_card_rgba(&self, width: u32, height: u32) -> Result<Vec<u8>, String> {
    let (pri, sec, acc) = self.validate()?;
    let w = width.clamp(16, 1024) as usize;
    let h = height.clamp(16, 1024) as usize;
    let mut out = vec![0u8; w * h * 4];
    for y in 0..h {
      for x in 0..w {
        let idx = (y * w + x) * 4;
        let in_header = y < h / 4;
        let in_accent = x >= w / 8 && x < (w * 3) / 8 && y >= (h * 5) / 8 && y < (h * 7) / 8;
        let rgb = if in_accent {
          acc
        } else if in_header {
          pri
        } else {
          sec
        };
        out[idx] = rgb[0];
        out[idx + 1] = rgb[1];
        out[idx + 2] = rgb[2];
        out[idx + 3] = 255;
      }
    }
    Ok(out)
  }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SocialResizePreset {
  #[default]
  InstagramPost,
  InstagramReel,
  YouTubeThumbnail,
  LinkedInBanner,
  Presentation16x9,
}

pub const SOCIAL_RESIZE_PRESETS: [SocialResizePreset; 5] = [SocialResizePreset::InstagramPost, SocialResizePreset::InstagramReel, SocialResizePreset::YouTubeThumbnail, SocialResizePreset::LinkedInBanner, SocialResizePreset::Presentation16x9];

impl SocialResizePreset {
  pub fn id(self) -> &'static str {
    match self {
      Self::InstagramPost => "instagramPost",
      Self::InstagramReel => "instagramReel",
      Self::YouTubeThumbnail => "youtubeThumbnail",
      Self::LinkedInBanner => "linkedinBanner",
      Self::Presentation16x9 => "presentation16x9",
    }
  }

  pub fn label(self) -> &'static str {
    match self {
      Self::InstagramPost => "Instagram Post (1080×1080)",
      Self::InstagramReel => "Instagram Reel (1080×1920)",
      Self::YouTubeThumbnail => "YouTube Thumbnail (1920×1080)",
      Self::LinkedInBanner => "LinkedIn Banner (1200×627)",
      Self::Presentation16x9 => "Presentation 16:9 (1920×1080)",
    }
  }

  pub fn dimensions_px(self) -> (u32, u32) {
    match self {
      Self::InstagramPost => (1080, 1080),
      Self::InstagramReel => (1080, 1920),
      Self::YouTubeThumbnail | Self::Presentation16x9 => (1920, 1080),
      Self::LinkedInBanner => (1200, 627),
    }
  }

  pub fn aspect_label(self) -> &'static str {
    match self {
      Self::InstagramPost => "1:1",
      Self::InstagramReel => "9:16",
      Self::YouTubeThumbnail | Self::Presentation16x9 => "16:9",
      Self::LinkedInBanner => "1.91:1",
    }
  }

  /// Safe-margin inset `[x, y, width, height]` in pixels avoiding social platform UI overlays.
  pub fn safe_area_px(self) -> [u32; 4] {
    let (w, h) = self.dimensions_px();
    let (mx, my) = match self {
      Self::InstagramPost => (0.06_f32, 0.06_f32),
      Self::InstagramReel => (0.07, 0.12),
      Self::YouTubeThumbnail => (0.06, 0.07),
      Self::LinkedInBanner => (0.08, 0.10),
      Self::Presentation16x9 => (0.05, 0.05),
    };
    let px = (w as f32 * mx).round() as u32;
    let py = (h as f32 * my).round() as u32;
    [px, py, w.saturating_sub(px * 2), h.saturating_sub(py * 2)]
  }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ExpressQuickAction {
  #[default]
  ApplyBrandKit,
  SmartResize,
  BulkCreateCsv,
  InsertQrCode,
  LiveWatercolor,
  ExtractCaptureTheme,
  FrameIoReview,
}

pub const EXPRESS_QUICK_ACTIONS: [ExpressQuickAction; 7] = [ExpressQuickAction::ApplyBrandKit, ExpressQuickAction::SmartResize, ExpressQuickAction::BulkCreateCsv, ExpressQuickAction::InsertQrCode, ExpressQuickAction::LiveWatercolor, ExpressQuickAction::ExtractCaptureTheme, ExpressQuickAction::FrameIoReview];

impl ExpressQuickAction {
  pub fn label(self) -> &'static str {
    match self {
      Self::ApplyBrandKit => "Apply Brand Kit (DeckCraft)",
      Self::SmartResize => "Smart Social Resize (DeckCraft)",
      Self::BulkCreateCsv => "Bulk Create from CSV (DeckCraft)",
      Self::InsertQrCode => "Insert Vector QR Code (DeckCraft)",
      Self::LiveWatercolor => "Live Watercolor Wash (PhotoCraft)",
      Self::ExtractCaptureTheme => "Extract OKLab Theme (VectorCraft)",
      Self::FrameIoReview => "Frame.io Review & Telestrator (FilmCraft)",
    }
  }

  pub fn target_app_id(self) -> &'static str {
    match self {
      Self::ApplyBrandKit | Self::SmartResize | Self::BulkCreateCsv | Self::InsertQrCode => "deckcraft",
      Self::LiveWatercolor => "photocraft",
      Self::ExtractCaptureTheme => "vectorcraft",
      Self::FrameIoReview => "filmcraft",
    }
  }

  pub fn command_id(self) -> &'static str {
    match self {
      Self::ApplyBrandKit => "express.applyBrandKit",
      Self::SmartResize => "express.smartResize",
      Self::BulkCreateCsv => "express.bulkCreateCsv",
      Self::InsertQrCode => "express.insertQrCode",
      Self::LiveWatercolor => "fresco.applyLiveWatercolor",
      Self::ExtractCaptureTheme => "capture.extractColorTheme",
      Self::FrameIoReview => "review.addComment",
    }
  }
}

/// Validated 1-click `craft://` deep-link & JSON handoff payload across the Craft Suite.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CraftHandoffMetadata {
  pub target_app: String,
  pub command: String,
  pub brand_kit: ExpressBrandKit,
  pub social_preset: SocialResizePreset,
  pub payload_text: String,
}

fn percent_encode_component(input: &str) -> String {
  let mut out = String::with_capacity(input.len() + 8);
  for &b in input.as_bytes() {
    if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~') {
      out.push(char::from(b));
    } else {
      use std::fmt::Write as _;
      let _ = write!(out, "%{b:02X}");
    }
  }
  out
}

impl CraftHandoffMetadata {
  pub fn from_action(action: ExpressQuickAction, brand_kit: ExpressBrandKit, social_preset: SocialResizePreset, payload_text: String) -> Self {
    Self { target_app: String::from(action.target_app_id()), command: String::from(action.command_id()), brand_kit, social_preset, payload_text }
  }

  pub fn validate(&self) -> Result<(), String> {
    if !SUITE_APPS.iter().any(|app| app.id == self.target_app) {
      return Err(format!("Unknown Craft Suite target `{}`", self.target_app));
    }
    if self.command.trim().is_empty() || !self.command.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'.' || b == b'_') {
      return Err(format!("Invalid handoff command `{}`", self.command));
    }
    let _ = self.brand_kit.validate()?;
    Ok(())
  }

  /// Build a canonical RFC 3986 `craft://<app>/<command>?...` deep-link URI.
  pub fn build_handoff_uri(&self) -> Result<String, String> {
    self.validate()?;
    let pri = normalize_hex_rgb(&self.brand_kit.primary_hex)?;
    let sec = normalize_hex_rgb(&self.brand_kit.secondary_hex)?;
    let acc = normalize_hex_rgb(&self.brand_kit.accent_hex)?;
    let (w, h) = self.social_preset.dimensions_px();
    let mut uri = format!("craft://{}/{}?channel={}&width={}&height={}&primary={}&secondary={}&accent={}&headingFont={}&bodyFont={}", self.target_app, self.command, self.social_preset.id(), w, h, percent_encode_component(&pri), percent_encode_component(&sec), percent_encode_component(&acc), percent_encode_component(self.brand_kit.heading_font.trim()), percent_encode_component(self.brand_kit.body_font.trim()),);
    if !self.payload_text.trim().is_empty() {
      uri.push_str("&payload=");
      uri.push_str(&percent_encode_component(self.payload_text.trim()));
    }
    Ok(uri)
  }

  /// Structured JSON handoff metadata envelope.
  pub fn to_handoff_json(&self) -> Result<Value, String> {
    let uri = self.build_handoff_uri()?;
    let (w, h) = self.social_preset.dimensions_px();
    let safe = self.social_preset.safe_area_px();
    Ok(json!({
      "schema": "craft.handoff.v1",
      "uri": uri,
      "targetApp": self.target_app,
      "command": self.command,
      "socialChannel": {
        "id": self.social_preset.id(),
        "widthPx": w,
        "heightPx": h,
        "aspect": self.social_preset.aspect_label(),
        "safeAreaPx": safe,
      },
      "brandKit": {
        "name": self.brand_kit.name.trim(),
        "primaryHex": normalize_hex_rgb(&self.brand_kit.primary_hex)?,
        "secondaryHex": normalize_hex_rgb(&self.brand_kit.secondary_hex)?,
        "accentHex": normalize_hex_rgb(&self.brand_kit.accent_hex)?,
        "headingFont": self.brand_kit.heading_font.trim(),
        "bodyFont": self.brand_kit.body_font.trim(),
      },
      "payload": self.payload_text.trim(),
    }))
  }
}

pub struct ExpressStudioState {
  pub preset: BrandKitPreset,
  pub brand_kit: ExpressBrandKit,
  pub social_preset: SocialResizePreset,
  pub quick_action: ExpressQuickAction,
  pub payload_text: String,
}

impl Default for ExpressStudioState {
  fn default() -> Self {
    Self { preset: BrandKitPreset::StudioIndigo, brand_kit: ExpressBrandKit::default(), social_preset: SocialResizePreset::InstagramPost, quick_action: ExpressQuickAction::ApplyBrandKit, payload_text: String::from("https://getartcraft.com") }
  }
}

fn swatch_chip(ui: &mut Ui, hex: &str, label: &str) {
  let color = match parse_hex_rgb(hex) {
    Ok([r, g, b]) => Color32::from_rgb(r, g, b),
    Err(_) => Color32::from_gray(90),
  };
  let (rect, resp) = ui.allocate_exact_size(vec2(20.0, 20.0), Sense::hover());
  ui.painter().rect_filled(rect, 4.0, color);
  ui.painter().rect_stroke(rect, 4.0, theme::hairline(), egui::StrokeKind::Inside);
  resp.on_hover_text(format!("{label}: {hex}"));
}

impl ExpressStudioState {
  pub fn ui(&mut self, ui: &mut Ui, env: &mut Env<'_>) {
    egui::Frame::new().fill(theme::CONTROLS).inner_margin(egui::Margin::same(14)).corner_radius(theme::RADIUS).show(ui, |ui| {
      ui.set_min_width(ui.available_width());
      widgets::section_label(ui, "Express Quick Actions & Brand Kit Studio", theme::ACCENT_INK);
      ui.add_space(6.0);

      ui.horizontal_wrapped(|ui| {
        ui.label("Brand Preset:");
        for p in BRAND_KIT_PRESETS {
          if ui.selectable_label(self.preset == p, p.label()).clicked() {
            self.preset = p;
            self.brand_kit = ExpressBrandKit::from_preset(p);
          }
        }
        swatch_chip(ui, &self.brand_kit.primary_hex, "Primary");
        swatch_chip(ui, &self.brand_kit.secondary_hex, "Secondary");
        swatch_chip(ui, &self.brand_kit.accent_hex, "Accent");
      });

      ui.add_space(4.0);
      ui.horizontal_wrapped(|ui| {
        ui.label("Primary:");
        ui.add(egui::TextEdit::singleline(&mut self.brand_kit.primary_hex).desired_width(76.0));
        ui.label("Secondary:");
        ui.add(egui::TextEdit::singleline(&mut self.brand_kit.secondary_hex).desired_width(76.0));
        ui.label("Accent:");
        ui.add(egui::TextEdit::singleline(&mut self.brand_kit.accent_hex).desired_width(76.0));
        ui.label("Heading:");
        ui.add(egui::TextEdit::singleline(&mut self.brand_kit.heading_font).desired_width(115.0));
        ui.label("Body:");
        ui.add(egui::TextEdit::singleline(&mut self.brand_kit.body_font).desired_width(115.0));
      });

      ui.add_space(4.0);
      ui.horizontal_wrapped(|ui| {
        ui.label("Social Resize:");
        for sp in SOCIAL_RESIZE_PRESETS {
          if ui.selectable_label(self.social_preset == sp, sp.label()).clicked() {
            self.social_preset = sp;
          }
        }
        let [sx, sy, sw, sh] = self.social_preset.safe_area_px();
        ui.small(format!("Safe frame: {sw}×{sh} px (+{sx},+{sy})"));
      });

      ui.add_space(4.0);
      ui.horizontal_wrapped(|ui| {
        ui.label("Quick Action:");
        egui::ComboBox::from_id_salt("express_quick_action").selected_text(self.quick_action.label()).show_ui(ui, |ui| {
          for qa in EXPRESS_QUICK_ACTIONS {
            if ui.selectable_label(self.quick_action == qa, qa.label()).clicked() {
              self.quick_action = qa;
            }
          }
        });
        ui.label("Payload / URL:");
        ui.add(egui::TextEdit::singleline(&mut self.payload_text).desired_width(180.0));
      });

      ui.add_space(8.0);
      ui.horizontal_wrapped(|ui| {
        if ui.button("Copy 1-Click craft:// Handoff").clicked() {
          let meta = CraftHandoffMetadata::from_action(self.quick_action, self.brand_kit.clone(), self.social_preset, self.payload_text.clone());
          match meta.build_handoff_uri() {
            Ok(uri) => {
              env.requests.push(AppRequest::CopyText(uri));
              env.toasts.success("Copied craft:// handoff link to clipboard");
            },
            Err(err) => env.toasts.error(err),
          }
        }
        if ui.button("Copy Handoff JSON").clicked() {
          let meta = CraftHandoffMetadata::from_action(self.quick_action, self.brand_kit.clone(), self.social_preset, self.payload_text.clone());
          match meta.to_handoff_json() {
            Ok(val) => {
              let text = serde_json::to_string_pretty(&val).unwrap_or_else(|_| val.to_string());
              env.requests.push(AppRequest::CopyText(text));
              env.toasts.success("Copied Craft handoff JSON metadata");
            },
            Err(err) => env.toasts.error(err),
          }
        }
        if ui.button("Copy Brand Style Prompt").clicked() {
          match self.brand_kit.style_prompt_directive(self.social_preset) {
            Ok(prompt) => {
              env.requests.push(AppRequest::CopyText(prompt));
              env.toasts.success("Copied Brand Kit style prompt");
            },
            Err(err) => env.toasts.error(err),
          }
        }
        if ui.button("Send Brand Card to Create Image").clicked() {
          match self.encode_brand_card_png() {
            Ok((png, prompt)) => {
              env.requests.push(AppRequest::SendPngToCreateImage { png, prompt: Some(prompt) });
            },
            Err(err) => env.toasts.error(err),
          }
        }
      });
    });
  }

  fn encode_brand_card_png(&self) -> Result<(Vec<u8>, String), String> {
    let prompt = self.brand_kit.style_prompt_directive(self.social_preset)?;
    let (pw, ph) = self.social_preset.dimensions_px();
    let card_w = (pw / 6).clamp(160, 360);
    let card_h = (ph / 6).clamp(90, 360);
    let rgba = self.brand_kit.render_brand_card_rgba(card_w, card_h)?;
    let image = RgbaImage::from_raw(card_w, card_h, rgba).ok_or_else(|| String::from("Failed to construct RGBA brand card image"))?;
    let mut png = Vec::new();
    image.write_to(&mut Cursor::new(&mut png), ImageFormat::Png).map_err(|err| format!("PNG encoding failed: {err}"))?;
    Ok((png, prompt))
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn brand_kit_presets_validate_and_generate_style_prompts() {
    for preset in BRAND_KIT_PRESETS {
      let kit = ExpressBrandKit::from_preset(preset);
      assert!(kit.validate().is_ok());
      let directive = kit.style_prompt_directive(SocialResizePreset::InstagramReel).unwrap();
      assert!(directive.contains("1080x1920 px"));
      assert!(directive.contains(&kit.heading_font));
    }
    let bad = ExpressBrandKit { primary_hex: String::from("#GGGGGG"), ..ExpressBrandKit::default() };
    assert!(bad.validate().is_err());
  }

  #[test]
  fn social_resize_presets_match_channel_dimensions_and_safe_margins() {
    assert_eq!(SocialResizePreset::InstagramPost.dimensions_px(), (1080, 1080));
    assert_eq!(SocialResizePreset::InstagramReel.dimensions_px(), (1080, 1920));
    assert_eq!(SocialResizePreset::YouTubeThumbnail.dimensions_px(), (1920, 1080));
    assert_eq!(SocialResizePreset::LinkedInBanner.dimensions_px(), (1200, 627));
    assert_eq!(SocialResizePreset::Presentation16x9.dimensions_px(), (1920, 1080));
    for sp in SOCIAL_RESIZE_PRESETS {
      let (w, h) = sp.dimensions_px();
      let [sx, sy, sw, sh] = sp.safe_area_px();
      assert!(sx > 0 && sy > 0);
      assert_eq!(sx * 2 + sw, w);
      assert_eq!(sy * 2 + sh, h);
    }
  }

  #[test]
  fn craft_handoff_uri_and_json_enforce_registered_apps_and_encode_params() {
    let kit = ExpressBrandKit::from_preset(BrandKitPreset::CyberNeon);
    let handoff = CraftHandoffMetadata::from_action(ExpressQuickAction::InsertQrCode, kit, SocialResizePreset::LinkedInBanner, String::from("https://getartcraft.com/launch?utm=q1"));
    let uri = handoff.build_handoff_uri().unwrap();
    assert!(uri.starts_with("craft://deckcraft/express.insertQrCode?"));
    assert!(uri.contains("channel=linkedinBanner&width=1200&height=627"));
    assert!(uri.contains("accent=%2306B6D4"));
    assert!(uri.contains("headingFont=Space%20Grotesk"));

    let meta_json = handoff.to_handoff_json().unwrap();
    assert_eq!(meta_json["schema"], json!("craft.handoff.v1"));
    assert_eq!(meta_json["targetApp"], json!("deckcraft"));
    assert_eq!(meta_json["socialChannel"]["widthPx"], json!(1200));

    let untrusted = CraftHandoffMetadata { target_app: String::from("../evil-app"), ..handoff };
    assert!(untrusted.build_handoff_uri().is_err());

    let state = ExpressStudioState::default();
    let (png, _) = state.encode_brand_card_png().unwrap();
    assert!(png.starts_with(b"\x89PNG\r\n\x1a\n"));
  }
}
