//! Adobe InDesign Snippet (`.idms`) single-file XML export & import.
//!
//! An `.idms` snippet is a self-contained IDML `<Document DOMVersion="20.4" SnippetType="PageItem">`
//! XML document with the `<?aid style="50" type="snippet" ...?>` processing instruction, inlining
//! the swatches, fonts, styles, preferences, spreads, page items, and stories that an IDML ZIP
//! package stores across separate `Resources/*`, `Spreads/*`, and `Stories/*` parts.

use std::collections::HashMap;

use designcraft_doc::Document;

use crate::export::{ExportOptions, export_parts_with};
use crate::xml::{El, Node, document, from_craft, parse};
use crate::{IdmlError, Result, import_idml_with};

const AID_SNIPPET_PI: &str = "<?aid style=\"50\" type=\"snippet\" readerVersion=\"6.0\" featureSet=\"257\" product=\"20.4(32)\" ?>\n";

/// Returns `true` when `bytes` begins with an XML InDesign Snippet (`.idms`).
pub fn is_idms(bytes: &[u8]) -> bool {
    if crate::icml::is_icml(bytes) || crate::icml::is_icma(bytes) {
        return false;
    }
    let trimmed = bytes.trim_ascii_start();
    if !(trimmed.starts_with(b"<?xml") || trimmed.starts_with(b"<?aid") || trimmed.starts_with(b"<Document")) {
        return false;
    }
    let head_len = trimmed.len().min(4096);
    let head = String::from_utf8_lossy(&trimmed[..head_len]);
    head.contains("type=\"snippet\"") || head.contains("type='snippet'") || head.contains("SnippetType=")
}

/// Export `doc` as a single-file `.idms` XML snippet with embedded images.
pub fn export_snippet(doc: &Document) -> Vec<u8> {
    export_snippet_with(doc, &ExportOptions::default())
}

/// Export `doc` as a single-file `.idms` XML snippet with `opts`.
pub fn export_snippet_with(doc: &Document, opts: &ExportOptions) -> Vec<u8> {
    let parts = export_parts_with(doc, opts);
    let part_map: HashMap<&str, &[u8]> = parts.iter().map(|(name, data)| (name.as_str(), data.as_slice())).collect();

    let Some(dm_bytes) = part_map.get("designmap.xml") else {
        return Vec::new();
    };
    let Ok(mut root) = parse(dm_bytes) else {
        return Vec::new();
    };
    root.set("SnippetType", "PageItem");

    let mut inlined_children = Vec::with_capacity(root.children.len() + 16);
    for child in root.children {
        let Node::El(el) = child else {
            inlined_children.push(child);
            continue;
        };
        if el.name.starts_with("idPkg:")
            && let Some(src) = el.get("src")
            && let Some(part_bytes) = part_map.get(src)
            && let Ok(part_root) = parse(part_bytes)
        {
            for sub in part_root.children {
                if matches!(sub, Node::El(_)) {
                    inlined_children.push(sub);
                }
            }
            continue;
        }
        inlined_children.push(Node::El(el));
    }
    root.children = inlined_children;

    let raw = document(&root);
    let xml_str = String::from_utf8_lossy(&raw);
    if let Some(nl) = xml_str.find('\n') {
        let (decl, rest) = xml_str.split_at(nl + 1);
        let mut out = String::with_capacity(raw.len() + AID_SNIPPET_PI.len());
        out.push_str(decl);
        out.push_str(AID_SNIPPET_PI);
        out.push_str(rest);
        out.into_bytes()
    } else {
        let mut out = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n");
        out.push_str(AID_SNIPPET_PI);
        out.push_str(&xml_str);
        out.into_bytes()
    }
}

/// Import an `.idms` XML snippet without reading external file links.
pub fn import_snippet(bytes: &[u8]) -> Result<Document> {
    import_snippet_with(bytes, &|_| None)
}

/// Import an `.idms` XML snippet with custom external link resolution.
pub fn import_snippet_with(bytes: &[u8], read_link: &dyn Fn(&str) -> Option<Vec<u8>>) -> Result<Document> {
    let root = parse(bytes).map_err(IdmlError::Invalid)?;
    if root.local() != "Document" {
        return Err(IdmlError::Invalid(format!("expected <Document> root in .idms snippet, found <{}>", root.name)));
    }

    let pkg_wrap = |kind: &str, children: Vec<El>| -> Vec<u8> {
        let mut w = from_craft(craft_idml::part_root(kind));
        for c in children {
            w.push(c);
        }
        document(&w)
    };

    let mut graphic_els = Vec::new();
    let mut font_els = Vec::new();
    let mut style_els = Vec::new();
    let mut pref_els = Vec::new();
    let doc_name = root.get("Name").unwrap_or("Snippet.indd").to_string();
    let zero_point = root.get("ZeroPoint").unwrap_or("0 0").to_string();
    let active_layer = root.get("ActiveLayer").unwrap_or("ub").to_string();

    let mut master_els = Vec::new();
    let mut spread_els = Vec::new();
    let mut story_els = Vec::new();
    let mut dm_children = Vec::new();
    let mut has_layer = false;

    for node in root.children {
        let Node::El(mut el) = node else { continue };
        match el.local() {
            "Color" | "Ink" | "MixedInk" | "MixedInkGroup" | "Swatch" | "Gradient" | "Tint" | "StrokeStyle" | "DashedStrokeStyle"
            | "DottedStrokeStyle" | "StripedStrokeStyle" | "PastedSmoothShade" => graphic_els.push(el),
            "FontFamily" | "CompositeFont" => font_els.push(el),
            "RootCharacterStyleGroup"
            | "RootParagraphStyleGroup"
            | "RootObjectStyleGroup"
            | "RootCellStyleGroup"
            | "RootTableStyleGroup"
            | "TOCStyle" => style_els.push(el),
            m if m.ends_with("Preference") || m == "FrameFittingOption" || m == "AnchoredObjectSetting" => {
                pref_els.push(el);
            }
            "MasterSpread" => master_els.push(el),
            "Spread" => {
                if el.find("Page").is_none() {
                    let page = El::new("Page")
                        .attr("Self", "p1")
                        .attr("Name", "1")
                        .attr("GeometricBounds", "0 0 792 612")
                        .attr("ItemTransform", "1 0 0 1 0 0");
                    el.children.insert(0, Node::El(page));
                }
                spread_els.push(el);
            }
            "Story" | "XmlStory" => story_els.push(el),
            "Layer" => {
                has_layer = true;
                dm_children.push(el);
            }
            _ => dm_children.push(el),
        }
    }

    if !has_layer {
        dm_children.push(
            El::new("Layer").attr("Self", "ub").attr("Name", "Layer 1").attr("Visible", "true").attr("Locked", "false").attr("Printable", "true"),
        );
    }

    let mut story_ids = Vec::new();
    let mut story_parts = Vec::new();
    for (idx, st_el) in story_els.into_iter().enumerate() {
        let sid = st_el.get("Self").map(ToString::to_string).unwrap_or_else(|| format!("u_st_{idx}"));
        story_ids.push(sid.clone());
        let path = format!("Stories/Story_{sid}.xml");
        story_parts.push((path, pkg_wrap("Story", vec![st_el])));
    }

    let mut master_parts = Vec::new();
    for (idx, ms_el) in master_els.into_iter().enumerate() {
        let mid = ms_el.get("Self").map(ToString::to_string).unwrap_or_else(|| format!("u_ms_{idx}"));
        let path = format!("MasterSpreads/MasterSpread_{mid}.xml");
        master_parts.push((path, pkg_wrap("MasterSpread", vec![ms_el])));
    }

    let mut spread_parts = Vec::new();
    for (idx, sp_el) in spread_els.into_iter().enumerate() {
        let spid = sp_el.get("Self").map(ToString::to_string).unwrap_or_else(|| format!("u_sp_{idx}"));
        let path = format!("Spreads/Spread_{spid}.xml");
        spread_parts.push((path, pkg_wrap("Spread", vec![sp_el])));
    }

    let mut dm = from_craft(craft_idml::designmap_root("d", &story_ids.join(" "), &doc_name, &zero_point, &active_layer));
    for c in dm_children {
        dm.push(c);
    }
    dm.push(from_craft(craft_idml::designmap_include("Graphic", "Resources/Graphic.xml")));
    dm.push(from_craft(craft_idml::designmap_include("Fonts", "Resources/Fonts.xml")));
    dm.push(from_craft(craft_idml::designmap_include("Styles", "Resources/Styles.xml")));
    dm.push(from_craft(craft_idml::designmap_include("Preferences", "Resources/Preferences.xml")));
    for (path, _) in &master_parts {
        dm.push(from_craft(craft_idml::designmap_include("MasterSpread", path)));
    }
    for (path, _) in &spread_parts {
        dm.push(from_craft(craft_idml::designmap_include("Spread", path)));
    }
    for (path, _) in &story_parts {
        dm.push(from_craft(craft_idml::designmap_include("Story", path)));
    }

    let mut parts: Vec<(String, Vec<u8>)> = vec![
        ("META-INF/container.xml".into(), craft_idml::CONTAINER_XML.as_bytes().to_vec()),
        ("designmap.xml".into(), document(&dm)),
        ("Resources/Graphic.xml".into(), pkg_wrap("Graphic", graphic_els)),
        ("Resources/Fonts.xml".into(), pkg_wrap("Fonts", font_els)),
        ("Resources/Styles.xml".into(), pkg_wrap("Styles", style_els)),
        ("Resources/Preferences.xml".into(), pkg_wrap("Preferences", pref_els)),
    ];
    parts.extend(master_parts);
    parts.extend(spread_parts);
    parts.extend(story_parts);

    let zip_bytes = craft_idml::write_package(&parts).map_err(|e| IdmlError::Invalid(format!("failed to stage .idms package: {e}")))?;
    import_idml_with(&zip_bytes, read_link)
}

#[cfg(test)]
mod tests {
    use designcraft_doc::build::NewDocument;
    use designcraft_doc::{Document, ParaFormat, SpreadRef};
    use designcraft_geom::Rect;

    use super::{export_snippet, import_snippet, is_idms};

    #[test]
    fn snippet_idms_round_trips_text_and_geometry() {
        let mut doc = Document::new(&NewDocument::default());
        let layer = doc.default_layer();
        doc.add_text_frame(SpreadRef::Doc(0), Rect::new(90.0, 120.0, 310.0, 200.0), layer, "Reusable Callout Card", ParaFormat::default()).unwrap();

        let idms_bytes = export_snippet(&doc);
        assert!(is_idms(&idms_bytes));
        let xml_text = String::from_utf8_lossy(&idms_bytes);
        assert!(xml_text.contains("type=\"snippet\""));
        assert!(xml_text.contains("SnippetType=\"PageItem\""));

        let imported = import_snippet(&idms_bytes).unwrap();
        assert_eq!(imported.spreads[0].items.len(), 1);
        let placed = &imported.spreads[0].items[0];
        assert!((placed.bounds().x0 - 90.0).abs() < 1e-3);
        assert!((placed.bounds().y0 - 120.0).abs() < 1e-3);
        let imp_sid = placed.text_frame().unwrap().story;
        assert_eq!(imported.stories[&imp_sid].text, "Reusable Callout Card");
    }
}
