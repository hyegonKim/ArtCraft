//! Interchange: camera-raw-settings (`crs:`) develop fields from XMP → our develop settings.
//!
//! Other raw developers store their edits as `crs:` properties in XMP sidecars, in DNG-embedded
//! XMP and in XMP preset files. This module maps the commonly used, documented-by-observation
//! fields to a *partial* [`DevelopSettings`](lightcraft_develop::DevelopSettings) JSON object
//! (only the fields present in the packet), which is then merged like a preset. Our pipeline is
//! not theirs, so the result is a best-effort approximation of the look, not a pixel match.
//! The full table is in `docs/xmp-interop.md`.
//!
//! Implemented from the public XMP specification (ISO 16684-1) and black-box observation of what
//! each field means; no third-party code or preset content was used.

use std::collections::BTreeMap;

use lightcraft_develop::{MIXER_BANDS, Preset};
use serde_json::{Map, Value, json};

/// Properties keyed `prefix:name` (as produced by [`lightcraft_meta::parse_xmp`]).
pub type Props = BTreeMap<String, Vec<String>>;

/// What the mapped settings will develop. It decides how a `crs:` white balance is read: a raw
/// with a Kelvin scale (colour matrices in the file, or measured for its camera) takes `Temperature`
/// as Kelvin; everything developed relative to its as-shot look (rendered files, and raws without
/// matrices anywhere, [`lightcraft_catalog::relative_wb_camera`]) needs a shift from the other app's
/// as-shot white instead, because on that scale 6500 K / 0 means *as shot* (issue #510).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Target {
    /// A preset: raw or rendered unknown. Absolute Kelvin first, else the relative scale.
    Any,
    /// A rendered photo (or a raw shown from its embedded preview): `Incremental*` fields on our
    /// relative scale.
    Rendered,
    /// A raw developed with an absolute white balance (DNG and other files with matrices, and raws
    /// whose camera has measured matrices): Kelvin as written.
    RawAbsolute,
    /// A raw developed relative to its as-shot look ([`lightcraft_catalog::Photo::relative_wb`]).
    RawRelative,
}

impl Target {
    /// The target for a file of this media kind and format (its extension or Lightroom's
    /// `fileFormat` name) shot on `camera` (Exif make and model as the catalog keeps them, or
    /// empty when unknown); `preview_only` for a raw that can't be decoded yet.
    pub fn for_file(kind: lightcraft_catalog::MediaKind, format: &str, preview_only: bool, camera: &str) -> Target {
        if kind != lightcraft_catalog::MediaKind::Raw || preview_only {
            Target::Rendered
        } else if lightcraft_catalog::relative_wb_camera(format, camera) {
            Target::RawRelative
        } else {
            Target::RawAbsolute
        }
    }
    /// The target for a photo in the catalog.
    pub fn for_photo(p: &lightcraft_catalog::Photo) -> Target {
        Target::for_file(p.kind, &p.format, p.preview_only.is_some(), &p.meta.camera)
    }
}

/// Band names as they appear in `crs:` field names, in our mixer order.
const CRS_BANDS: [&str; 8] = ["Red", "Orange", "Yellow", "Green", "Aqua", "Blue", "Purple", "Magenta"];

/// `crs:` fields that describe the packet rather than an adjustment.
const NON_ADJUSTMENT: &[&str] = &[
    "crs:Version",
    "crs:ProcessVersion",
    "crs:Name",
    "crs:ShortName",
    "crs:Group",
    "crs:UUID",
    "crs:PresetType",
    "crs:SupportsAmount",
    "crs:SupportsColor",
    "crs:SupportsMonochrome",
    "crs:SupportsHighDynamicRange",
    "crs:SupportsNormalDynamicRange",
    "crs:SupportsSceneReferred",
    "crs:SupportsOutputReferred",
    "crs:CameraModelRestriction",
    "crs:Copyright",
    "crs:ContactInfo",
    "crs:HasSettings",
    "crs:AlreadyApplied",
    "crs:RawFileName",
    "crs:HasCrop",
    // preset/library bookkeeping and tool options that don't change the rendering
    "crs:Cluster",
    "crs:SupportsAmount2",
    "crs:RequiresRGBTables",
    "crs:SortName",
    "crs:Description",
    "crs:CropConstrainToWarp",
    "crs:OverrideLookVignette",
    // the file's as-shot white, recorded next to the edit (ours comes from the file itself)
    "crs:AsShotTemperature",
    "crs:AsShotTint",
    // which Lightroom version may read the edit, and checksums of the profiles it names
    "crs:CompatibleVersion",
    "crs:CameraProfileDigest",
    "crs:LensProfileDigest",
    // the lens profile Lightroom picked: ours is the file's own, only `LensProfileEnable` carries over
    "crs:LensProfileName",
    "crs:LensProfileFilename",
    "crs:LensProfileIsEmbedded",
    "crs:LensProfileSetup",
    // a crop tool option (keep the crop inside the photo), not the crop itself
    "crs:CropConstrainToUnitSquare",
];

/// True if the packet carries any `crs:` adjustment (not just bookkeeping fields).
pub fn has_adjustments(props: &Props) -> bool {
    if props.get("crs:AlreadyApplied").and_then(|v| v.first()).is_some_and(|s| s.eq_ignore_ascii_case("true")) {
        // The pixels already contain these settings (e.g. an exported/rendered file).
        return false;
    }
    props.keys().any(|k| k.starts_with("crs:") && !NON_ADJUSTMENT.contains(&k.split('/').next().unwrap_or(k)))
}

thread_local! {
    /// Keys read by [`to_partial`] while [`to_partial_report`] runs.
    static READ: std::cell::RefCell<Option<std::collections::BTreeSet<String>>> = const { std::cell::RefCell::new(None) };
    /// Adjustments [`to_partial`] read but deliberately left out, for the report.
    static SKIPPED: std::cell::RefCell<Option<Vec<String>>> = const { std::cell::RefCell::new(None) };
}

fn note(k: &str) {
    READ.with(|r| {
        if let Some(s) = r.borrow_mut().as_mut() {
            s.insert(k.to_string());
        }
    });
}

fn skipped(what: &str) {
    SKIPPED.with(|r| {
        if let Some(s) = r.borrow_mut().as_mut() {
            s.push(what.to_string());
        }
    });
}

fn first<'a>(props: &'a Props, k: &str) -> Option<&'a str> {
    note(k);
    props.get(k).and_then(|v| v.first()).map(|s| s.trim()).filter(|s| !s.is_empty())
}

fn num(props: &Props, k: &str) -> Option<f64> {
    let s = first(props, k)?;
    let s = s.strip_prefix('+').unwrap_or(s);
    if let Some((n, d)) = s.split_once('/') {
        let (n, d) = (n.trim().parse::<f64>().ok()?, d.trim().parse::<f64>().ok()?);
        return (d != 0.0).then_some(n / d).filter(|v| v.is_finite());
    }
    s.parse::<f64>().ok().filter(|v| v.is_finite())
}

fn boolean(props: &Props, k: &str) -> Option<bool> {
    first(props, k).map(|s| s.eq_ignore_ascii_case("true") || s == "1")
}

/// Insert `v` at the dotted `path` of a JSON object, creating intermediate objects.
fn put(out: &mut Value, path: &str, v: Value) {
    let mut cur = out;
    let mut parts = path.split('.').peekable();
    while let Some(p) = parts.next() {
        let Value::Object(m) = cur else { return };
        if parts.peek().is_none() {
            m.insert(p.to_string(), v);
            return;
        }
        cur = m.entry(p.to_string()).or_insert_with(|| Value::Object(Map::new()));
    }
}

/// Relative temperature (−100..100, used for rendered files) → Kelvin, matching the UI's scale
/// (a mired shift around 6500 K).
pub fn rel_to_kelvin(r: f64) -> f64 {
    1e6 / (1e6 / 6500.0 - r.clamp(-100.0, 100.0) * 0.8)
}

/// Inverse of [`rel_to_kelvin`]: Kelvin → relative temperature (−100..100 around 6500 K).
pub fn kelvin_to_rel(k: f64) -> f64 {
    ((1e6 / 6500.0 - 1e6 / k.max(1000.0)) / 0.8).clamp(-100.0, 100.0)
}

/// Parse a point curve stored as an `rdf:Seq` of `"x, y"` strings in 0..255.
fn curve(props: &Props, k: &str) -> Option<Value> {
    note(k);
    let items = props.get(k)?;
    let mut pts = Vec::new();
    for it in items {
        let (x, y) = it.split_once(',')?;
        // (`parse` reads "NaN" and "inf" too: a curve with such a point is not a curve)
        let (x, y) = (x.trim().parse::<f64>().ok().filter(|v| v.is_finite())?, y.trim().parse::<f64>().ok().filter(|v| v.is_finite())?);
        pts.push(json!({"x": (x / 255.0).clamp(0.0, 1.0), "y": (y / 255.0).clamp(0.0, 1.0)}));
    }
    if pts.len() < 2 {
        return None;
    }
    // A straight 0,0 → 255,255 line is the identity: store it as "no curve".
    let identity = pts.len() == 2 && items.iter().all(|it| it.split(',').map(|v| v.trim()).collect::<Vec<_>>().windows(2).all(|w| w[0] == w[1]));
    Some(if identity { json!([]) } else { Value::Array(pts) })
}

/// Map the `crs:` fields of an XMP packet to a partial develop-settings JSON object.
///
/// `target` decides how the white balance is read (see [`Target`]).
pub fn to_partial(props: &Props, target: Target) -> Value {
    let mut out = Value::Object(Map::new());
    let o = &mut out;
    let n = |o: &mut Value, crs: &str, path: &str| {
        if let Some(v) = num(props, &format!("crs:{crs}")) {
            put(o, path, json!(v));
        }
    };
    // ---- Light (process version 2012+ names)
    n(o, "Exposure2012", "light.exposure");
    n(o, "Contrast2012", "light.contrast");
    n(o, "Highlights2012", "light.highlights");
    n(o, "Shadows2012", "light.shadows");
    n(o, "Whites2012", "light.whites");
    n(o, "Blacks2012", "light.blacks");
    // Lightroom's Light panel "HDR" button
    if let Some(on) = boolean(props, "crs:HDREditMode") {
        put(o, "hdr.enabled", json!(on));
    }
    // older process versions (most `.lrtemplate` presets): approximate the 2012 sliders from the
    // earlier ones, relative to their defaults (contrast 25, blacks 5, brightness 50)
    let has = |o: &Value, path: &str| path.split('.').try_fold(o, |v, k| v.get(k)).is_some();
    // presets written by newer versions keep the old fields at their defaults: ignore them there
    let pv2012 = props.keys().any(|k| k.ends_with("2012"));
    let old = |o: &mut Value, crs: &str, path: &str, f: &dyn Fn(f64) -> f64| {
        let v = num(props, &format!("crs:{crs}"));
        if !pv2012
            && !has(o, path)
            && let Some(v) = v
        {
            put(o, path, json!(f(v).clamp(-100.0, 100.0)));
        }
    };
    old(o, "Exposure", "light.exposure", &|v| v);
    old(o, "Contrast", "light.contrast", &|v| v - 25.0);
    old(o, "HighlightRecovery", "light.highlights", &|v| -v);
    old(o, "FillLight", "light.shadows", &|v| v);
    old(o, "Shadows", "light.blacks", &|v| -(v - 5.0) * 4.0);
    old(o, "Brightness", "light.whites", &|v| (v - 50.0) * 0.6);
    // ---- Presence
    n(o, "Texture", "effects.texture");
    n(o, "Clarity2012", "effects.clarity");
    old(o, "Clarity", "effects.clarity", &|v| v);
    n(o, "Dehaze", "effects.dehaze");
    n(o, "Vibrance", "color.vibrance");
    n(o, "Saturation", "color.saturation");

    // ---- White balance
    let mode = first(props, "crs:WhiteBalance").map(|m| match m.to_ascii_lowercase().replace(' ', "").as_str() {
        "asshot" => "asShot",
        "auto" => "auto",
        "daylight" => "daylight",
        "cloudy" => "cloudy",
        "shade" => "shade",
        "tungsten" => "tungsten",
        "fluorescent" => "fluorescent",
        "flash" => "flash",
        _ => "custom",
    });
    let abs = num(props, "crs:Temperature").map(|t| (t, num(props, "crs:Tint")));
    let rel = num(props, "crs:IncrementalTemperature")
        .or(num(props, "crs:IncrementalTint").map(|_| 0.0))
        .map(|t| (rel_to_kelvin(t), num(props, "crs:IncrementalTint")));
    // A Kelvin value for something we develop relative to its as-shot look: the other app's
    // as-shot white (`AsShotTemperature` / `AsShotTint`, written next to the edit) is the
    // reference, so the edit is the same mired shift from ours (issue #510).
    let shifted = || {
        let (t, tint) = abs?;
        let as_shot = num(props, "crs:AsShotTemperature").filter(|k| *k > 0.0)?;
        if t <= 0.0 {
            return None;
        }
        let r = (1e6 / as_shot - 1e6 / t) / 0.8;
        let tint = tint.map(|v| v - num(props, "crs:AsShotTint").unwrap_or(0.0));
        Some((rel_to_kelvin(r), tint))
    };
    let wb = match target {
        Target::RawAbsolute => abs,
        Target::Any => abs.or(rel),
        Target::Rendered => rel.or_else(shifted).or(abs),
        Target::RawRelative => rel.or_else(shifted),
    };
    match (mode, wb) {
        (Some(m), _) if m != "custom" => put(o, "wb.mode", json!(m)),
        (m, Some((t, tint))) => {
            put(o, "wb.mode", json!(m.unwrap_or("custom")));
            put(o, "wb.temp", json!(t));
            if let Some(tint) = tint {
                put(o, "wb.tint", json!(tint));
            }
            // Kelvin as the other app wrote it: say so, so the value stays Kelvin on a raw with that scale (a
            // preset's Kelvin is read as the relative scale on the files that have only that, as before)
            if wb == abs && matches!(target, Target::RawAbsolute | Target::Any) {
                put(o, "wb.scale", json!("kelvin"));
            }
        }
        // a custom Kelvin with nothing to shift it from: read on the relative scale it would be a
        // large colour shift (ΔE 22 against Lightroom's export on the photos of issue #510), so
        // the photo stays As Shot (ΔE 12 there) and the report says so
        (_, None) if target == Target::RawRelative && abs.is_some() => {
            put(o, "wb.mode", json!("asShot"));
            skipped("Temperature, Tint (custom white balance without AsShotTemperature: kept As Shot)");
        }
        (Some(m), None) => put(o, "wb.mode", json!(m)),
        (None, None) => {}
    }

    // ---- Treatment + B&W mix
    if let Some(bw) = boolean(props, "crs:ConvertToGrayscale") {
        put(o, "treatment", json!(if bw { "bw" } else { "color" }));
    }
    for (band, crs) in MIXER_BANDS.iter().zip(CRS_BANDS) {
        n(o, &format!("HueAdjustment{crs}"), &format!("mixer.{band}.hue"));
        n(o, &format!("SaturationAdjustment{crs}"), &format!("mixer.{band}.sat"));
        n(o, &format!("LuminanceAdjustment{crs}"), &format!("mixer.{band}.lum"));
        n(o, &format!("GrayMixer{crs}"), &format!("bw_mix.{band}"));
    }

    // ---- Tone curve: parametric regions + point curves
    n(o, "ParametricShadows", "curve.shadows");
    n(o, "ParametricDarks", "curve.darks");
    n(o, "ParametricLights", "curve.lights");
    n(o, "ParametricHighlights", "curve.highlights");
    n(o, "ParametricShadowSplit", "curve.split_shadows");
    n(o, "ParametricMidtoneSplit", "curve.split_mid");
    n(o, "ParametricHighlightSplit", "curve.split_highlights");
    n(o, "CurveRefineSaturation", "curve.refine_saturation");
    if let Some(c) = curve(props, "crs:ToneCurve").filter(|c| c.as_array().is_some_and(|a| !a.is_empty())) {
        // the older single curve, used when no 2012 curve is present
        if !props.contains_key("crs:ToneCurvePV2012") {
            put(o, "curve.master", c);
        }
    }
    // Lightroom Classic reads the red / green / blue curves only when the packet holds the master
    // 2012 curve and all three channel curves (as its own packets always do): a packet missing any
    // of them imports without channel curves (master + red, master + red + green, or red / green /
    // blue without the master all leave the photo's channels unchanged there)
    if let Some(c) = curve(props, "crs:ToneCurvePV2012") {
        put(o, "curve.master", c);
        let channels = [("ToneCurvePV2012Red", "red"), ("ToneCurvePV2012Green", "green"), ("ToneCurvePV2012Blue", "blue")]
            .map(|(crs, ch)| (curve(props, &format!("crs:{crs}")), ch));
        if channels.iter().all(|(c, _)| c.is_some()) {
            for (c, ch) in channels {
                if let Some(c) = c {
                    put(o, &format!("curve.{ch}"), c);
                }
            }
        }
    }

    // ---- Color grading (split toning fields are shared with the older split-toning panel)
    n(o, "SplitToningShadowHue", "grading.shadows.hue");
    n(o, "SplitToningShadowSaturation", "grading.shadows.sat");
    n(o, "ColorGradeShadowLum", "grading.shadows.lum");
    n(o, "SplitToningHighlightHue", "grading.highlights.hue");
    n(o, "SplitToningHighlightSaturation", "grading.highlights.sat");
    n(o, "ColorGradeHighlightLum", "grading.highlights.lum");
    n(o, "ColorGradeMidtoneHue", "grading.midtones.hue");
    n(o, "ColorGradeMidtoneSat", "grading.midtones.sat");
    n(o, "ColorGradeMidtoneLum", "grading.midtones.lum");
    n(o, "ColorGradeGlobalHue", "grading.global.hue");
    n(o, "ColorGradeGlobalSat", "grading.global.sat");
    n(o, "ColorGradeGlobalLum", "grading.global.lum");
    n(o, "ColorGradeBlending", "grading.blending");
    n(o, "SplitToningBalance", "grading.balance");

    // ---- Detail
    n(o, "Sharpness", "detail.sharpen_amount");
    n(o, "SharpenRadius", "detail.sharpen_radius");
    n(o, "SharpenDetail", "detail.sharpen_detail");
    n(o, "SharpenEdgeMasking", "detail.sharpen_masking");
    n(o, "LuminanceSmoothing", "detail.nr_luminance");
    n(o, "LuminanceNoiseReductionDetail", "detail.nr_detail");
    n(o, "LuminanceNoiseReductionContrast", "detail.nr_contrast");
    n(o, "ColorNoiseReduction", "detail.nr_color");
    n(o, "ColorNoiseReductionDetail", "detail.nr_color_detail");
    n(o, "ColorNoiseReductionSmoothness", "detail.nr_color_smoothness");

    // ---- Effects: post-crop vignette + grain
    n(o, "PostCropVignetteAmount", "vignette.amount");
    n(o, "PostCropVignetteMidpoint", "vignette.midpoint");
    n(o, "PostCropVignetteRoundness", "vignette.roundness");
    n(o, "PostCropVignetteFeather", "vignette.feather");
    n(o, "PostCropVignetteHighlightContrast", "vignette.highlights");
    if let Some(st) = num(props, "crs:PostCropVignetteStyle") {
        let style = match st as i64 {
            2 => "colorPriority",
            3 => "paintOverlay",
            _ => "highlightPriority",
        };
        put(o, "vignette.style", json!(style));
    }
    n(o, "GrainAmount", "grain.amount");
    n(o, "GrainSize", "grain.size");
    n(o, "GrainFrequency", "grain.roughness");
    n(o, "EyeglassReflections", "reflections.eyeglass");
    n(o, "WindowReflections", "reflections.window");

    // ---- Optics (manual corrections; lens profiles are ours, only the switch carries over).
    // Our profile corrections are the file's own (DNG-embedded) ones, which Lightroom applies
    // whatever its "Enable Profile Corrections" box says [inferred]: so only an enabling switch
    // carries over, and `LensProfileEnable=0` (the default in most presets) leaves ours as it is.
    if boolean(props, "crs:LensProfileEnable") == Some(true) {
        put(o, "optics.lens_profile", json!(true));
    }
    if let Some(b) = boolean(props, "crs:AutoLateralCA") {
        put(o, "optics.remove_ca", json!(b));
    }
    n(o, "LensManualDistortionAmount", "optics.distortion");
    n(o, "VignetteAmount", "optics.vignetting");
    n(o, "VignetteMidpoint", "optics.vignetting_midpoint");
    n(o, "DefringePurpleAmount", "optics.defringe_purple_amount");
    n(o, "DefringePurpleHueLo", "optics.defringe_purple_hue_lo");
    n(o, "DefringePurpleHueHi", "optics.defringe_purple_hue_hi");
    n(o, "DefringeGreenAmount", "optics.defringe_green_amount");
    n(o, "DefringeGreenHueLo", "optics.defringe_green_hue_lo");
    n(o, "DefringeGreenHueHi", "optics.defringe_green_hue_hi");

    // ---- Calibration
    n(o, "ShadowTint", "calibration.shadows_tint");
    n(o, "RedHue", "calibration.red_hue");
    n(o, "RedSaturation", "calibration.red_sat");
    n(o, "GreenHue", "calibration.green_hue");
    n(o, "GreenSaturation", "calibration.green_sat");
    n(o, "BlueHue", "calibration.blue_hue");
    n(o, "BlueSaturation", "calibration.blue_sat");

    // ---- Geometry
    n(o, "PerspectiveVertical", "geometry.vertical");
    n(o, "PerspectiveHorizontal", "geometry.horizontal");
    n(o, "PerspectiveRotate", "geometry.rotate");
    n(o, "PerspectiveScale", "geometry.scale");
    n(o, "PerspectiveAspect", "geometry.aspect");
    n(o, "PerspectiveX", "geometry.offset_x");
    n(o, "PerspectiveY", "geometry.offset_y");
    if let Some(u) = num(props, "crs:PerspectiveUpright") {
        let mode = match u as i64 {
            1 => "auto",
            2 => "level",
            3 => "vertical",
            4 => "full",
            5 => "guided",
            _ => "off",
        };
        put(o, "geometry.upright", json!(mode));
    }

    // ---- Crop (normalized edges of the unrotated image + straighten angle)
    match boolean(props, "crs:HasCrop") {
        Some(true) => {
            let e = |k: &str, d: f64| num(props, &format!("crs:Crop{k}")).unwrap_or(d).clamp(0.0, 1.0);
            let (l, t, r, b) = (e("Left", 0.0), e("Top", 0.0), e("Right", 1.0), e("Bottom", 1.0));
            if r > l && b > t {
                put(
                    o,
                    "crop.geometry",
                    json!({"rect": {"x0": l, "y0": t, "x1": r, "y1": b}, "angle": num(props, "crs:CropAngle").unwrap_or(0.0).clamp(-45.0, 45.0)}),
                );
            }
        }
        Some(false) => put(o, "crop.geometry", json!({"rect": {"x0": 0.0, "y0": 0.0, "x1": 1.0, "y1": 1.0}, "angle": 0.0})),
        None => {}
    }
    out
}

/// [`to_partial`], plus the adjustments in the packet it could not carry over (field names
/// without the `crs:` prefix, grouped: `CameraProfile`, `Look`, `MaskGroupBasedCorrections`…).
///
/// With `values` (the packet's structured properties), local corrections become masks too
/// ([`crate::crs_masks`]; radial masks fitted to `aspect` = width / height); components that
/// can't be carried over are reported as `Mask: <kind>`.
pub fn to_partial_report(props: &Props, values: Option<&crate::crs_masks::Values>, target: Target, aspect: f64) -> (Value, Vec<String>) {
    READ.with(|r| *r.borrow_mut() = Some(Default::default()));
    SKIPPED.with(|r| *r.borrow_mut() = Some(Vec::new()));
    let mut out = to_partial(props, target);
    let mut read = READ.with(|r| r.borrow_mut().take()).unwrap_or_default();
    let mut mask_skips = SKIPPED.with(|r| r.borrow_mut().take()).unwrap_or_default();
    if let Some(values) = values {
        let (masks, skipped) = crate::crs_masks::masks(values, aspect);
        if !masks.is_empty() {
            put(&mut out, "masks", Value::Array(masks));
        }
        if values.keys().any(|k| crate::crs_masks::CONTAINERS.contains(&k.as_str())) {
            read.extend(crate::crs_masks::CONTAINERS.iter().map(|c| c.to_string()));
            mask_skips.extend(skipped.into_iter().map(|k| format!("Mask: {k}")));
        }
        let (spots, skipped) = crate::crs_masks::spots(values, aspect);
        if !spots.is_empty() {
            put(&mut out, "spots", Value::Array(spots));
        }
        if values.keys().any(|k| crate::crs_masks::SPOT_CONTAINERS.contains(&k.as_str())) {
            read.extend(crate::crs_masks::SPOT_CONTAINERS.iter().map(|c| c.to_string()));
            mask_skips.extend(skipped.into_iter().map(|k| format!("Spot: {k}")));
        }
    }
    // fields that only switch a panel on/off or name things: not adjustments by themselves; and
    // Point Color slots that are all empty (-1)
    let value_is = |k: &str, f: &dyn Fn(&[String]) -> bool| props.get(&format!("crs:{k}")).is_some_and(|v| f(v));
    let empty_points = |v: &[String]| v.iter().all(|s| s.split(',').all(|n| n.trim().parse::<f64>().is_ok_and(|x| x == -1.0) || n.trim().is_empty()));
    let quiet = |k: &str| {
        k.starts_with("Enable")
            || k.starts_with("ToneCurveName")
            || k == "AutoTone"
            || k == "AutoGrayscaleMix"
            || (k == "PointColors" && value_is(k, &empty_points))
            // the lens profile's strength, written at its default (100 %) next to every profile
            || (matches!(k, "LensProfileDistortionScale" | "LensProfileVignettingScale") && value_is(k, &|v| v.iter().all(|s| s.trim() == "100")))
    };
    let mut unmapped: Vec<String> = props
        .keys()
        .filter(|k| k.starts_with("crs:"))
        .map(|k| k.split('/').next().unwrap_or(k))
        .filter(|k| !NON_ADJUSTMENT.contains(k) && !read.contains(*k))
        .map(|k| k.trim_start_matches("crs:").to_string())
        .filter(|k| !quiet(k))
        .collect();
    unmapped.dedup();
    unmapped.extend(mask_skips);
    (out, unmapped)
}

/// The selected camera profile name from an XMP packet. The name is metadata only: it does not
/// identify or carry the profile's rendering data.
pub fn camera_profile_name(props: &Props) -> Option<&str> {
    first(props, "crs:CameraProfile")
}

/// Whether an XMP packet contains a saved Lightroom Upright matrix. Lightroom's matrix uses a
/// coordinate convention that LightCraft does not currently import.
pub fn has_saved_upright_transform(props: &Props) -> bool {
    props.keys().any(|key| {
        key.strip_prefix("crs:").is_some_and(|name| {
            name.starts_with("UprightTransform_") && props.get(key).is_some_and(|values| values.iter().any(|v| !v.trim().is_empty()))
        })
    })
}

/// Read an XMP preset (`crs:` fields + `crs:Name` / `crs:Group`) into one of our presets.
/// Returns `None` when the packet has no adjustments we understand.
pub fn preset_from_xmp(xmp: &str, fallback_name: &str) -> Option<Preset> {
    let d = lightcraft_meta::parse_xmp(xmp).ok()?;
    let props = &d.properties;
    let settings = to_partial(props, Target::Any);
    if settings.as_object().is_none_or(Map::is_empty) {
        return None;
    }
    let name = first(props, "crs:Name").unwrap_or(fallback_name).to_string();
    let group = first(props, "crs:Group").unwrap_or("Imported Presets").to_string();
    let uuid = first(props, "crs:UUID").map(|u| u.to_ascii_lowercase());
    let id = format!("user.xmp.{}", uuid.unwrap_or_else(|| crate::presets::slug(&format!("{group}-{name}"))));
    Some(Preset { id, name, group, settings, favorite: false, builtin: false })
}

fn xml_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            _ => out.push(c),
        }
    }
    out
}

fn fmt_crs_num(v: f64, signed: bool) -> String {
    if !v.is_finite() {
        return "0".to_string();
    }
    let rounded = v.round();
    if (v - rounded).abs() < 1e-6 {
        let i = rounded as i64;
        if signed && i > 0 { format!("+{i}") } else { i.to_string() }
    } else {
        let s = format!("{v:.4}");
        let s = s.trim_end_matches('0').trim_end_matches('.');
        if signed && v > 0.0 { format!("+{s}") } else { s.to_string() }
    }
}

fn get_f64(v: &Value, path: &str) -> Option<f64> {
    path.split('.').try_fold(v, |cur, part| cur.get(part)).and_then(Value::as_f64).filter(|n| n.is_finite())
}

fn get_str<'a>(v: &'a Value, path: &str) -> Option<&'a str> {
    path.split('.').try_fold(v, |cur, part| cur.get(part)).and_then(Value::as_str)
}

fn get_bool(v: &Value, path: &str) -> Option<bool> {
    path.split('.').try_fold(v, |cur, part| cur.get(part)).and_then(Value::as_bool)
}

fn push_num_table(attrs: &mut Vec<(String, String)>, settings: &Value, rows: &[(&str, &str, bool)]) {
    for &(crs_k, path, signed) in rows {
        if let Some(v) = get_f64(settings, path) {
            attrs.push((crs_k.to_string(), fmt_crs_num(v, signed)));
        }
    }
}

fn format_curve_seq(name: &str, pts: Option<&Vec<Value>>) -> String {
    let mut items = Vec::new();
    if let Some(arr) = pts {
        for pt in arr {
            if let (Some(x), Some(y)) = (pt.get("x").and_then(Value::as_f64), pt.get("y").and_then(Value::as_f64))
                && x.is_finite()
                && y.is_finite()
            {
                let x255 = (x.clamp(0.0, 1.0) * 255.0).round() as i32;
                let y255 = (y.clamp(0.0, 1.0) * 255.0).round() as i32;
                items.push(format!("<rdf:li>{x255}, {y255}</rdf:li>"));
            }
        }
    }
    if items.len() < 2 {
        items.clear();
        items.push("<rdf:li>0, 0</rdf:li>".to_string());
        items.push("<rdf:li>255, 255</rdf:li>".to_string());
    }
    format!("   <crs:{name}>\n    <rdf:Seq>{}</rdf:Seq>\n   </crs:{name}>\n", items.concat())
}

/// Serialize a preset into an `.xmp` preset packet of `crs:` fields (the format [`preset_from_xmp`] reads).
pub fn preset_to_xmp(preset: &Preset) -> String {
    let uuid = preset
        .id
        .strip_prefix("user.xmp.")
        .filter(|u| !u.is_empty() && u.chars().all(|c| c.is_ascii_alphanumeric() || c == '-'))
        .map(str::to_ascii_uppercase)
        .unwrap_or_else(|| {
            let fnv = preset.id.bytes().fold(0xcbf2_9ce4_8422_2325u64, |h, b| (h ^ u64::from(b)).wrapping_mul(0x100_0000_01b3));
            let h1 = lightcraft_catalog::mix64(fnv);
            let h2 = lightcraft_catalog::mix64(h1 ^ 0x9e37_79b9_7f4a_7c15);
            format!("{h1:016X}{h2:016X}")
        });
    partial_to_xmp(&preset.settings, Some(&preset.name), Some(&preset.group), Some(&uuid))
}

fn push_wb_xmp(attrs: &mut Vec<(String, String)>, settings: &Value) {
    let wb_mode = get_str(settings, "wb.mode");
    let wb_temp = get_f64(settings, "wb.temp");
    let wb_tint = get_f64(settings, "wb.tint");
    if let Some(m) = wb_mode {
        let crs_wb = match m {
            "asShot" => "As Shot",
            "auto" => "Auto",
            "daylight" => "Daylight",
            "cloudy" => "Cloudy",
            "shade" => "Shade",
            "tungsten" => "Tungsten",
            "fluorescent" => "Fluorescent",
            "flash" => "Flash",
            _ => "Custom",
        };
        attrs.push(("WhiteBalance".into(), crs_wb.into()));
    } else if wb_temp.is_some() || wb_tint.is_some() {
        attrs.push(("WhiteBalance".into(), "Custom".into()));
    }
    if wb_mode.is_none_or(|m| m == "custom") {
        if get_str(settings, "wb.scale") == Some("kelvin") {
            if let Some(t) = wb_temp {
                attrs.push(("Temperature".into(), fmt_crs_num(t, false)));
            }
            if let Some(tint) = wb_tint {
                attrs.push(("Tint".into(), fmt_crs_num(tint, true)));
            }
        } else {
            if let Some(t) = wb_temp.filter(|t| *t > 0.0) {
                attrs.push(("IncrementalTemperature".into(), fmt_crs_num(kelvin_to_rel(t), true)));
            }
            if let Some(tint) = wb_tint {
                attrs.push(("IncrementalTint".into(), fmt_crs_num(tint, true)));
            }
        }
    }
}

fn push_tone_and_color_xmp(attrs: &mut Vec<(String, String)>, settings: &Value) {
    if let Some(t) = get_str(settings, "treatment") {
        attrs.push(("ConvertToGrayscale".into(), if t == "bw" { "True".into() } else { "False".into() }));
    }
    if let Some(hdr) = get_bool(settings, "hdr.enabled") {
        attrs.push(("HDREditMode".into(), if hdr { "1".into() } else { "0".into() }));
    }
    push_num_table(
        attrs,
        settings,
        &[
            ("Exposure2012", "light.exposure", true),
            ("Contrast2012", "light.contrast", true),
            ("Highlights2012", "light.highlights", true),
            ("Shadows2012", "light.shadows", true),
            ("Whites2012", "light.whites", true),
            ("Blacks2012", "light.blacks", true),
            ("Texture", "effects.texture", true),
            ("Clarity2012", "effects.clarity", true),
            ("Dehaze", "effects.dehaze", true),
            ("Vibrance", "color.vibrance", true),
            ("Saturation", "color.saturation", true),
            ("ParametricShadows", "curve.shadows", true),
            ("ParametricDarks", "curve.darks", true),
            ("ParametricLights", "curve.lights", true),
            ("ParametricHighlights", "curve.highlights", true),
            ("ParametricShadowSplit", "curve.split_shadows", false),
            ("ParametricMidtoneSplit", "curve.split_mid", false),
            ("ParametricHighlightSplit", "curve.split_highlights", false),
            ("CurveRefineSaturation", "curve.refine_saturation", false),
            ("SplitToningShadowHue", "grading.shadows.hue", false),
            ("SplitToningShadowSaturation", "grading.shadows.sat", false),
            ("ColorGradeShadowLum", "grading.shadows.lum", true),
            ("SplitToningHighlightHue", "grading.highlights.hue", false),
            ("SplitToningHighlightSaturation", "grading.highlights.sat", false),
            ("ColorGradeHighlightLum", "grading.highlights.lum", true),
            ("ColorGradeMidtoneHue", "grading.midtones.hue", false),
            ("ColorGradeMidtoneSat", "grading.midtones.sat", false),
            ("ColorGradeMidtoneLum", "grading.midtones.lum", true),
            ("ColorGradeGlobalHue", "grading.global.hue", false),
            ("ColorGradeGlobalSat", "grading.global.sat", false),
            ("ColorGradeGlobalLum", "grading.global.lum", true),
            ("ColorGradeBlending", "grading.blending", false),
            ("SplitToningBalance", "grading.balance", true),
        ],
    );
    for (band, crs) in MIXER_BANDS.iter().zip(CRS_BANDS) {
        for (prefix, sub) in [("HueAdjustment", "hue"), ("SaturationAdjustment", "sat"), ("LuminanceAdjustment", "lum")] {
            if let Some(v) = get_f64(settings, &format!("mixer.{band}.{sub}")) {
                attrs.push((format!("{prefix}{crs}"), fmt_crs_num(v, true)));
            }
        }
        if let Some(v) = get_f64(settings, &format!("bw_mix.{band}")) {
            attrs.push((format!("GrayMixer{crs}"), fmt_crs_num(v, true)));
        }
    }
}

fn push_detail_optics_geometry_xmp(attrs: &mut Vec<(String, String)>, settings: &Value) {
    push_num_table(
        attrs,
        settings,
        &[
            ("Sharpness", "detail.sharpen_amount", false),
            ("SharpenRadius", "detail.sharpen_radius", true),
            ("SharpenDetail", "detail.sharpen_detail", false),
            ("SharpenEdgeMasking", "detail.sharpen_masking", false),
            ("LuminanceSmoothing", "detail.nr_luminance", false),
            ("LuminanceNoiseReductionDetail", "detail.nr_detail", false),
            ("LuminanceNoiseReductionContrast", "detail.nr_contrast", false),
            ("ColorNoiseReduction", "detail.nr_color", false),
            ("ColorNoiseReductionDetail", "detail.nr_color_detail", false),
            ("ColorNoiseReductionSmoothness", "detail.nr_color_smoothness", false),
            ("PostCropVignetteAmount", "vignette.amount", true),
            ("PostCropVignetteMidpoint", "vignette.midpoint", false),
            ("PostCropVignetteRoundness", "vignette.roundness", true),
            ("PostCropVignetteFeather", "vignette.feather", false),
            ("PostCropVignetteHighlightContrast", "vignette.highlights", false),
            ("GrainAmount", "grain.amount", false),
            ("GrainSize", "grain.size", false),
            ("GrainFrequency", "grain.roughness", false),
            ("EyeglassReflections", "reflections.eyeglass", false),
            ("WindowReflections", "reflections.window", false),
            ("LensManualDistortionAmount", "optics.distortion", true),
            ("VignetteAmount", "optics.vignetting", true),
            ("VignetteMidpoint", "optics.vignetting_midpoint", false),
            ("DefringePurpleAmount", "optics.defringe_purple_amount", false),
            ("DefringePurpleHueLo", "optics.defringe_purple_hue_lo", false),
            ("DefringePurpleHueHi", "optics.defringe_purple_hue_hi", false),
            ("DefringeGreenAmount", "optics.defringe_green_amount", false),
            ("DefringeGreenHueLo", "optics.defringe_green_hue_lo", false),
            ("DefringeGreenHueHi", "optics.defringe_green_hue_hi", false),
            ("ShadowTint", "calibration.shadows_tint", true),
            ("RedHue", "calibration.red_hue", true),
            ("RedSaturation", "calibration.red_sat", true),
            ("GreenHue", "calibration.green_hue", true),
            ("GreenSaturation", "calibration.green_sat", true),
            ("BlueHue", "calibration.blue_hue", true),
            ("BlueSaturation", "calibration.blue_sat", true),
            ("PerspectiveVertical", "geometry.vertical", true),
            ("PerspectiveHorizontal", "geometry.horizontal", true),
            ("PerspectiveRotate", "geometry.rotate", true),
            ("PerspectiveScale", "geometry.scale", false),
            ("PerspectiveAspect", "geometry.aspect", true),
            ("PerspectiveX", "geometry.offset_x", false),
            ("PerspectiveY", "geometry.offset_y", false),
        ],
    );
    if let Some(st) = get_str(settings, "vignette.style") {
        let code = match st {
            "colorPriority" => "2",
            "paintOverlay" => "3",
            _ => "1",
        };
        attrs.push(("PostCropVignetteStyle".into(), code.into()));
    }
    if get_bool(settings, "optics.lens_profile") == Some(true) {
        attrs.push(("LensProfileEnable".into(), "1".into()));
    }
    if let Some(ca) = get_bool(settings, "optics.remove_ca") {
        attrs.push(("AutoLateralCA".into(), if ca { "1".into() } else { "0".into() }));
    }
    if let Some(u) = get_str(settings, "geometry.upright") {
        let code = match u {
            "auto" => "1",
            "level" => "2",
            "vertical" => "3",
            "full" => "4",
            "guided" => "5",
            _ => "0",
        };
        attrs.push(("PerspectiveUpright".into(), code.into()));
    }
    push_crop_xmp(attrs, settings);
}

fn push_crop_xmp(attrs: &mut Vec<(String, String)>, settings: &Value) {
    let Some(geom) = settings.get("crop").and_then(|c| c.get("geometry")) else { return };
    let l = get_f64(geom, "rect.x0").unwrap_or(0.0);
    let t = get_f64(geom, "rect.y0").unwrap_or(0.0);
    let r = get_f64(geom, "rect.x1").unwrap_or(1.0);
    let b = get_f64(geom, "rect.y1").unwrap_or(1.0);
    let angle = get_f64(geom, "angle").unwrap_or(0.0);
    let is_unit = l.abs() < 1e-6 && t.abs() < 1e-6 && (r - 1.0).abs() < 1e-6 && (b - 1.0).abs() < 1e-6 && angle.abs() < 1e-6;
    if is_unit {
        attrs.push(("HasCrop".into(), "False".into()));
    } else {
        attrs.push(("HasCrop".into(), "True".into()));
        attrs.push(("CropLeft".into(), fmt_crs_num(l, false)));
        attrs.push(("CropTop".into(), fmt_crs_num(t, false)));
        attrs.push(("CropRight".into(), fmt_crs_num(r, false)));
        attrs.push(("CropBottom".into(), fmt_crs_num(b, false)));
        attrs.push(("CropAngle".into(), fmt_crs_num(angle, false)));
    }
}

/// Serialize partial `DevelopSettings` JSON into a `crs:` XMP packet. Groups without `crs:` fields are left out
/// ([`xmp_unwritten_groups`]).
pub fn partial_to_xmp(settings: &Value, name: Option<&str>, group: Option<&str>, uuid: Option<&str>) -> String {
    let mut attrs: Vec<(String, String)> = Vec::new();
    if name.is_some() || group.is_some() || uuid.is_some() {
        attrs.push(("PresetType".into(), "Normal".into()));
        attrs.push(("Cluster".into(), String::new()));
        if let Some(u) = uuid {
            attrs.push(("UUID".into(), xml_escape(u)));
        }
        for flag in [
            "SupportsAmount",
            "SupportsColor",
            "SupportsMonochrome",
            "SupportsHighDynamicRange",
            "SupportsNormalDynamicRange",
            "SupportsSceneReferred",
            "SupportsOutputReferred",
        ] {
            attrs.push((flag.into(), "True".into()));
        }
    }
    attrs.push(("ProcessVersion".into(), "11.0".into()));
    attrs.push(("HasSettings".into(), "True".into()));
    push_wb_xmp(&mut attrs, settings);
    push_tone_and_color_xmp(&mut attrs, settings);
    push_detail_optics_geometry_xmp(&mut attrs, settings);

    let mut elems = String::new();
    for (tag, val) in [("Name", name), ("Group", group)] {
        if let Some(text) = val {
            elems.push_str(&format!(
                "   <crs:{tag}>\n    <rdf:Alt>\n     <rdf:li xml:lang=\"x-default\">{}</rdf:li>\n    </rdf:Alt>\n   </crs:{tag}>\n",
                xml_escape(text)
            ));
        }
    }
    let master_curve = settings.pointer("/curve/master").and_then(Value::as_array);
    let red_curve = settings.pointer("/curve/red").and_then(Value::as_array);
    let green_curve = settings.pointer("/curve/green").and_then(Value::as_array);
    let blue_curve = settings.pointer("/curve/blue").and_then(Value::as_array);
    let has_channel_curves = red_curve.is_some() || green_curve.is_some() || blue_curve.is_some();
    if master_curve.is_some() || has_channel_curves {
        attrs.push(("ToneCurveName2012".into(), "Custom".into()));
        elems.push_str(&format_curve_seq("ToneCurvePV2012", master_curve));
        if has_channel_curves {
            elems.push_str(&format_curve_seq("ToneCurvePV2012Red", red_curve));
            elems.push_str(&format_curve_seq("ToneCurvePV2012Green", green_curve));
            elems.push_str(&format_curve_seq("ToneCurvePV2012Blue", blue_curve));
        }
    }
    let attr_str: String = attrs.iter().map(|(k, v)| format!("\n    crs:{k}=\"{v}\"")).collect();
    if elems.is_empty() {
        format!(
            "<x:xmpmeta xmlns:x=\"adobe:ns:meta/\">\n <rdf:RDF xmlns:rdf=\"http://www.w3.org/1999/02/22-rdf-syntax-ns#\">\n  <rdf:Description rdf:about=\"\"\n    xmlns:crs=\"http://ns.adobe.com/camera-raw-settings/1.0/\"{attr_str}/>\n </rdf:RDF>\n</x:xmpmeta>"
        )
    } else {
        format!(
            "<x:xmpmeta xmlns:x=\"adobe:ns:meta/\">\n <rdf:RDF xmlns:rdf=\"http://www.w3.org/1999/02/22-rdf-syntax-ns#\">\n  <rdf:Description rdf:about=\"\"\n    xmlns:crs=\"http://ns.adobe.com/camera-raw-settings/1.0/\"{attr_str}>\n{elems}  </rdf:Description>\n </rdf:RDF>\n</x:xmpmeta>"
        )
    }
}

/// The top-level groups of partial develop settings that [`partial_to_xmp`] has no `crs:` fields for (masks,
/// spots, the profile, Point Color…), which an XMP preset therefore leaves out. Groups it does write can still
/// lose fields that have no `crs:` counterpart (e.g. the grain seed, the HDR SDR-rendition offsets).
pub fn xmp_unwritten_groups(settings: &Value) -> Vec<String> {
    const WRITTEN: &[&str] = &[
        "treatment",
        "hdr",
        "wb",
        "light",
        "effects",
        "color",
        "mixer",
        "bw_mix",
        "curve",
        "grading",
        "detail",
        "vignette",
        "grain",
        "optics",
        "calibration",
        "geometry",
        "crop",
    ];
    const BOOKKEEPING: &[&str] = &["version", "process"];
    let Some(groups) = settings.as_object() else { return Vec::new() };
    groups
        .iter()
        .filter(|(k, _)| !WRITTEN.contains(&k.as_str()) && !BOOKKEEPING.contains(&k.as_str()))
        .filter(|(_, v)| !v.is_null() && !v.as_array().is_some_and(Vec::is_empty) && !v.as_object().is_some_and(Map::is_empty))
        .map(|(k, _)| k.clone())
        .collect()
}

#[cfg(test)]
mod tests;
