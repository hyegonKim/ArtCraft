//! Dialogs for Express Suite in DeckCraft:
//! - `"brandKit"` (`express.applyBrandKit`): primary/secondary/accent hex swatches + heading/body fonts.
//! - `"bulkCsv"` (`express.bulkCreateCsv`): CSV paste box with `{{column}}` template expansion.
//! - `"qrCode"` (`express.insertQrCode`): pure-Rust vector QR code generator dialog.

use deckcraft_model::express::builtin_brand_kits;
use egui::{Ui, vec2};
use serde_json::json;

use super::{Dialog, SlideApp, buttons, check, choice, field};

pub(super) fn brand_kit_dialog(app: &mut SlideApp, ui: &mut Ui, d: &mut Dialog) -> bool {
    let active_kit = app.session.active().and_then(|s| s.doc.brand_kit.clone()).unwrap_or_default();
    if !d.fields.contains_key("seeded") {
        d.fields.insert("seeded".into(), "1".into());
        d.fields.insert("name".into(), active_kit.name);
        d.fields.insert("primaryHex".into(), active_kit.primary_hex);
        d.fields.insert("secondaryHex".into(), active_kit.secondary_hex);
        d.fields.insert("accentHex".into(), active_kit.accent_hex);
        d.fields.insert("headingFont".into(), active_kit.heading_font);
        d.fields.insert("bodyFont".into(), active_kit.body_font);
    }

    ui.horizontal_wrapped(|ui| {
        ui.label("Presets:");
        for preset in builtin_brand_kits() {
            if ui.button(&preset.name).clicked() {
                d.fields.insert("name".into(), preset.name);
                d.fields.insert("primaryHex".into(), preset.primary_hex);
                d.fields.insert("secondaryHex".into(), preset.secondary_hex);
                d.fields.insert("accentHex".into(), preset.accent_hex);
                d.fields.insert("headingFont".into(), preset.heading_font);
                d.fields.insert("bodyFont".into(), preset.body_font);
            }
        }
    });
    ui.separator();

    field(ui, d, "Kit name:", "name", "Studio Indigo");
    field(ui, d, "Primary (#HEX):", "primaryHex", "#0F172A");
    field(ui, d, "Secondary (#HEX):", "secondaryHex", "#F8FAFC");
    field(ui, d, "Accent (#HEX):", "accentHex", "#4F46E5");
    field(ui, d, "Heading font:", "headingFont", "Montserrat");
    field(ui, d, "Body font:", "bodyFont", "Inter");
    let all = check(ui, d, "Apply to entire presentation", "all", true);

    let (ok, cancel) = buttons(ui, "Apply");
    if ok {
        let _ = app.run(
            "express.applyBrandKit",
            json!({
                "name": d.get("name", "Studio Indigo"),
                "primaryHex": d.get("primaryHex", "#0F172A"),
                "secondaryHex": d.get("secondaryHex", "#F8FAFC"),
                "accentHex": d.get("accentHex", "#4F46E5"),
                "headingFont": d.get("headingFont", "Montserrat"),
                "bodyFont": d.get("bodyFont", "Inter"),
                "all": all,
            }),
        );
    }
    ok || cancel
}

pub(super) fn smart_resize_dialog(app: &mut SlideApp, ui: &mut Ui, d: &mut Dialog) -> bool {
    if !d.fields.contains_key("channel") {
        d.fields.insert("channel".into(), "instagramPost".into());
    }
    ui.horizontal(|ui| {
        ui.add_sized(vec2(120.0, 20.0), egui::Label::new("Social channel:"));
        choice(
            ui,
            d,
            "express-channel",
            "Social channel",
            "channel",
            &[
                ("instagramPost", "Instagram Post (1080×1080)"),
                ("instagramReel", "Instagram Reel (1080×1920)"),
                ("youtubeThumbnail", "YouTube Thumbnail (1920×1080)"),
                ("linkedinBanner", "LinkedIn Banner (1200×627)"),
                ("presentation16x9", "Presentation 16:9 (1920×1080)"),
            ],
        );
    });
    let (ok, cancel) = buttons(ui, "Resize");
    if ok {
        let ch = d.get("channel", "instagramPost");
        let _ = app.run("express.smartResize", json!({ "channel": ch }));
    }
    ok || cancel
}

pub(super) fn bulk_csv_dialog(app: &mut SlideApp, ui: &mut Ui, d: &mut Dialog) -> bool {
    ui.label("Expand {{column}} placeholders on the current slide from CSV rows:");
    let mut csv = d.get("csv", "title,subtitle\nLaunch Keynote,Q1 Highlights\nProduct Roadmap,Q2 Milestones");
    ui.add(egui::TextEdit::multiline(&mut csv).desired_rows(5).desired_width(360.0));
    d.fields.insert("csv".into(), csv.clone());
    let replace = check(ui, d, "Replace template slide", "replaceTemplate", false);

    let (ok, cancel) = buttons(ui, "Create Slides");
    if ok {
        let _ = app.run(
            "express.bulkCreateCsv",
            json!({
                "csv": csv,
                "replaceTemplate": replace,
            }),
        );
    }
    ok || cancel
}

pub(super) fn qr_code_dialog(app: &mut SlideApp, ui: &mut Ui, d: &mut Dialog) -> bool {
    field(ui, d, "URL or text:", "text", "https://getartcraft.com");
    field(ui, d, "Foreground (#HEX):", "fgColor", "#0F172A");
    field(ui, d, "Background (#HEX):", "bgColor", "#FFFFFF");
    let (ok, cancel) = buttons(ui, "Insert");
    if ok {
        let _ = app.run(
            "express.insertQrCode",
            json!({
                "text": d.get("text", "https://getartcraft.com"),
                "fgColor": d.get("fgColor", "#0F172A"),
                "bgColor": d.get("bgColor", "#FFFFFF"),
                "quietZone": 2,
            }),
        );
    }
    ok || cancel
}
