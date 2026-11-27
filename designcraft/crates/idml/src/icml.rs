//! Clean-room Adobe InCopy Markup Language (`.icml`) and InCopy Assignment (`.icma`) XML reader & writer.
//!
//! `.icml` is a single-story XML interchange document (`<Document DOMVersion="...">` with the
//! `SnippetType="InCopyInterchange"` processing instruction) carrying swatches, font families,
//! character and paragraph styles, and one `<Story>` element with `ParagraphStyleRange`,
//! `CharacterStyleRange`, `<Content>`, `<Br/>`, and `<Change ChangeType="InsertedText"|"DeletedText"|"MovedText">`
//! tracked changes.
//!
//! `.icma` is an editorial assignment manifest listing assigned stories (`<AssignedStory>`) and
//! their `.icml` story references (`StoryRef`), contributor metadata, and lock states.

use std::ops::Range;
use std::sync::Arc;

use designcraft_color::Swatch;
use designcraft_doc::build::NewDocument;
use designcraft_doc::{ChangeMark, Document, ParaFormat, SpreadRef, Story, StoryId, Styles};
use designcraft_geom::Rect;

use crate::export::{Ex, ExportOptions, uid};
use crate::xml::{El, Node, document, from_craft, parse};
use crate::{IdmlError, Result, import_idml_with};

/// Default DOMVersion written to newly exported InCopy `.icml` and `.icma` files (InCopy 2027 / v22.0).
pub const ICML_DOM_VERSION: &str = "22.0";

const AID_ICML_PI: &str = "<?aid style=\"50\" type=\"snippet\" readerVersion=\"6.0\" featureSet=\"257\" product=\"22.0(1)\" ?>\n<?aid SnippetType=\"InCopyInterchange\"?>\n";
const AID_ICMA_PI: &str = "<?aid style=\"50\" type=\"assignment\" readerVersion=\"6.0\" featureSet=\"257\" product=\"22.0(1)\" ?>\n";

/// Parsed InCopy `.icml` interchange story and its accompanying style/swatch definitions.
#[derive(Clone, Debug)]
pub struct IcmlDocument {
    pub dom_version: String,
    pub story_self: String,
    pub story_title: String,
    pub track_changes: bool,
    pub story: Story,
    pub styles: Styles,
    pub swatches: Vec<Swatch>,
}

/// One assigned story entry inside an InCopy `.icma` assignment manifest.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct IcmaStoryEntry {
    pub self_id: String,
    pub name: String,
    pub story_ref: String,
    pub user_name: String,
    pub locked: bool,
}

/// Clean-room InCopy `.icma` assignment manifest.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IcmaAssignment {
    pub dom_version: String,
    pub self_id: String,
    pub name: String,
    pub user_name: String,
    pub frame_color: String,
    pub document_path: String,
    pub stories: Vec<IcmaStoryEntry>,
}

impl Default for IcmaAssignment {
    fn default() -> Self {
        Self {
            dom_version: ICML_DOM_VERSION.to_string(),
            self_id: "u_assign_1".to_string(),
            name: "Editorial Assignment".to_string(),
            user_name: String::new(),
            frame_color: "LightBlue".to_string(),
            document_path: String::new(),
            stories: Vec::new(),
        }
    }
}

/// Returns `true` when `bytes` begins with an InCopy `.icml` XML document.
pub fn is_icml(bytes: &[u8]) -> bool {
    if is_icma(bytes) {
        return false;
    }
    let trimmed = bytes.trim_ascii_start();
    if !(trimmed.starts_with(b"<?xml") || trimmed.starts_with(b"<?aid") || trimmed.starts_with(b"<Document")) {
        return false;
    }
    let head_len = trimmed.len().min(8192);
    let head = String::from_utf8_lossy(&trimmed[..head_len]);
    if head.contains("InCopyInterchange") || head.contains("<InCopyExportOption") {
        return true;
    }
    // Standalone ICML `<Document DOMVersion="...">` with `<Story` and no layout spreads/page-item snippet marker.
    head.contains("<Document")
        && head.contains("<Story")
        && !head.contains("SnippetType=\"PageItem\"")
        && !head.contains("SnippetType='PageItem'")
        && !head.contains("<Spread")
        && !head.contains("<idPkg:Spread")
}

/// Returns `true` when `bytes` begins with an InCopy `.icma` assignment manifest.
pub fn is_icma(bytes: &[u8]) -> bool {
    let trimmed = bytes.trim_ascii_start();
    if !(trimmed.starts_with(b"<?xml") || trimmed.starts_with(b"<?aid") || trimmed.starts_with(b"<Document") || trimmed.starts_with(b"<Assignment")) {
        return false;
    }
    let head_len = trimmed.len().min(8192);
    let head = String::from_utf8_lossy(&trimmed[..head_len]);
    head.contains("type=\"assignment\"") || head.contains("type='assignment'") || head.contains("<Assignment") || head.contains("<AssignedStory")
}

/// Export `story_id` from `doc` as an InCopy `.icml` XML byte buffer using [`ICML_DOM_VERSION`].
pub fn export_icml(doc: &Document, story_id: StoryId) -> Result<Vec<u8>> {
    export_icml_with_version(doc, story_id, ICML_DOM_VERSION)
}

/// Export a standalone [`Story`] (wrapping it in a default document for style/swatch defaults) as `.icml`.
pub fn export_icml_story(story: &Story) -> Vec<u8> {
    let mut doc = Document::new(&NewDocument::default());
    let sid = StoryId(1);
    let mut cloned = story.clone();
    cloned.id = sid;
    cloned.frames.clear();
    doc.stories.insert(sid, Arc::new(cloned));
    export_icml(&doc, sid).unwrap_or_default()
}

/// Export `story_id` from `doc` as an InCopy `.icml` XML byte buffer with an explicit `dom_version`.
pub fn export_icml_with_version(doc: &Document, story_id: StoryId, dom_version: &str) -> Result<Vec<u8>> {
    let story = doc.story(story_id).ok_or_else(|| IdmlError::Invalid(format!("no such story {} for ICML export", story_id.0)))?;
    let opts = ExportOptions::default();
    let mut ex = Ex::new(doc, &opts);

    // Build story element first so fonts used in the story are registered.
    let mut story_el = ex.story_el(story);
    if story_el.find("InCopyExportOption").is_none() {
        let opt = El::new("InCopyExportOption").attr("IncludeGraphicProxies", "true").attr("IncludeAllResources", "false");
        let insert_idx = usize::from(story_el.find("StoryPreference").is_some());
        story_el.children.insert(insert_idx.min(story_el.children.len()), Node::El(opt));
    }
    if !doc.title.is_empty() && story_el.get("StoryTitle") == Some("$ID/") {
        story_el.set("StoryTitle", &doc.title);
    }

    let styles_xml = ex.styles_part();
    let graphic_xml = ex.graphic_part();
    let fonts_xml = ex.fonts_part();

    let version = if dom_version.trim().is_empty() { ICML_DOM_VERSION } else { dom_version.trim() };
    let mut root = El::new("Document").attr("DOMVersion", version).attr("Self", "d");

    for part_bytes in [&graphic_xml, &fonts_xml, &styles_xml] {
        if let Ok(part_root) = parse(part_bytes) {
            for child in part_root.children {
                if let Node::El(el) = child {
                    // ICML keeps color/swatch/font/text-style groups; object/table style groups are fine too.
                    root.push(el);
                }
            }
        }
    }
    root.push(story_el);

    Ok(with_aid_header(&document(&root), AID_ICML_PI))
}

fn with_aid_header(raw_xml: &[u8], pi_block: &str) -> Vec<u8> {
    let xml_str = String::from_utf8_lossy(raw_xml);
    if let Some(nl) = xml_str.find('\n') {
        let (decl, rest) = xml_str.split_at(nl + 1);
        let mut out = String::with_capacity(raw_xml.len() + pi_block.len());
        out.push_str(decl);
        out.push_str(pi_block);
        out.push_str(rest);
        out.into_bytes()
    } else {
        let mut out = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n");
        out.push_str(pi_block);
        out.push_str(&xml_str);
        out.into_bytes()
    }
}

/// Import an InCopy `.icml` XML byte buffer into an [`IcmlDocument`].
pub fn import_icml(bytes: &[u8]) -> Result<IcmlDocument> {
    let root = parse(bytes).map_err(IdmlError::Invalid)?;
    let (dom_version, doc_children, standalone_story) = match root.local() {
        "Document" => (root.get("DOMVersion").unwrap_or(ICML_DOM_VERSION).to_string(), root.children, None),
        "Story" => (ICML_DOM_VERSION.to_string(), Vec::new(), Some(root)),
        other => {
            return Err(IdmlError::Invalid(format!("expected <Document> or <Story> root in .icml, found <{other}>")));
        }
    };

    let mut graphic_els = Vec::new();
    let mut font_els = Vec::new();
    let mut style_els = Vec::new();
    let mut pref_els = Vec::new();
    let mut story_el: Option<El> = standalone_story;

    for node in doc_children {
        let Node::El(el) = node else { continue };
        match el.local() {
            "Color" | "Ink" | "MixedInk" | "MixedInkGroup" | "Swatch" | "Gradient" | "Tint" | "StrokeStyle" | "DashedStrokeStyle"
            | "DottedStrokeStyle" | "StripedStrokeStyle" => {
                graphic_els.push(el);
            }
            "FontFamily" | "CompositeFont" => font_els.push(el),
            "RootCharacterStyleGroup"
            | "RootParagraphStyleGroup"
            | "RootObjectStyleGroup"
            | "RootCellStyleGroup"
            | "RootTableStyleGroup"
            | "TOCStyle" => style_els.push(el),
            m if m.ends_with("Preference") => pref_els.push(el),
            "Story" | "XmlStory" if story_el.is_none() => story_el = Some(el),
            _ => {}
        }
    }

    let Some(mut story_xml) = story_el else {
        return Err(IdmlError::Invalid("ICML document contains no <Story> element".into()));
    };

    let story_self = story_xml.get("Self").filter(|s| !s.trim().is_empty()).unwrap_or("u_icml_story").to_string();
    story_xml.set("Self", &story_self);
    let track_changes = story_xml.get("TrackChanges") == Some("true");
    let story_title = story_xml.get("StoryTitle").unwrap_or("").trim_start_matches("$ID/").to_string();

    // Collect `MovedText` byte ranges directly from XML so even if an upstream model normalizes
    // `ChangeType` to Inserted/None, `ChangeMark::Moved` survives faithfully.
    let moved_ranges = collect_moved_byte_ranges(&story_xml);

    let pkg_wrap = |kind: &str, children: Vec<El>| -> Vec<u8> {
        let mut w = from_craft(craft_idml::part_root(kind));
        for c in children {
            w.push(c);
        }
        document(&w)
    };

    // Stage a minimal spread with one text frame bound to `story_self` so `import_idml_with`
    // resolves every style, swatch, font, special character, table, note, and tracked change.
    let page = El::new("Page").attr("Self", "p1").attr("Name", "1").attr("GeometricBounds", "0 0 792 612").attr("ItemTransform", "1 0 0 1 0 0");
    let tf = El::new("TextFrame")
        .attr("Self", "tf_icml")
        .attr("ParentStory", &story_self)
        .attr("ContentType", "TextType")
        .attr("ItemTransform", "1 0 0 1 0 0")
        .child(
            El::new("Properties").child(
                El::new("PathGeometry").child(
                    El::new("GeometryPathType").attr("PathOpen", "false").child(
                        El::new("PathPointArray")
                            .child(El::new("PathPointType").attr("Anchor", "72 72").attr("LeftDirection", "72 72").attr("RightDirection", "72 72"))
                            .child(El::new("PathPointType").attr("Anchor", "72 540").attr("LeftDirection", "72 540").attr("RightDirection", "72 540"))
                            .child(
                                El::new("PathPointType").attr("Anchor", "720 540").attr("LeftDirection", "720 540").attr("RightDirection", "720 540"),
                            )
                            .child(
                                El::new("PathPointType").attr("Anchor", "720 72").attr("LeftDirection", "720 72").attr("RightDirection", "720 72"),
                            ),
                    ),
                ),
            ),
        );
    let spread = El::new("Spread").attr("Self", "sp_icml").child(page).child(tf);
    let layer = El::new("Layer").attr("Self", "ub").attr("Name", "Layer 1").attr("Visible", "true").attr("Locked", "false").attr("Printable", "true");

    let story_part_path = format!("Stories/Story_{story_self}.xml");
    let spread_part_path = "Spreads/Spread_sp_icml.xml".to_string();

    let mut dm =
        from_craft(craft_idml::designmap_root("d", &story_self, if story_title.is_empty() { "Story.icml" } else { &story_title }, "0 0", "ub"));
    dm.push(layer);
    dm.push(from_craft(craft_idml::designmap_include("Graphic", "Resources/Graphic.xml")));
    dm.push(from_craft(craft_idml::designmap_include("Fonts", "Resources/Fonts.xml")));
    dm.push(from_craft(craft_idml::designmap_include("Styles", "Resources/Styles.xml")));
    dm.push(from_craft(craft_idml::designmap_include("Preferences", "Resources/Preferences.xml")));
    dm.push(from_craft(craft_idml::designmap_include("Spread", &spread_part_path)));
    dm.push(from_craft(craft_idml::designmap_include("Story", &story_part_path)));

    let parts: Vec<(String, Vec<u8>)> = vec![
        ("META-INF/container.xml".into(), craft_idml::CONTAINER_XML.as_bytes().to_vec()),
        ("designmap.xml".into(), document(&dm)),
        ("Resources/Graphic.xml".into(), pkg_wrap("Graphic", graphic_els)),
        ("Resources/Fonts.xml".into(), pkg_wrap("Fonts", font_els)),
        ("Resources/Styles.xml".into(), pkg_wrap("Styles", style_els)),
        ("Resources/Preferences.xml".into(), pkg_wrap("Preferences", pref_els)),
        (spread_part_path, pkg_wrap("Spread", vec![spread])),
        (story_part_path, pkg_wrap("Story", vec![story_xml])),
    ];

    let zip_bytes = craft_idml::write_package(&parts).map_err(|e| IdmlError::Invalid(format!("failed to stage .icml package: {e}")))?;
    let imported = import_idml_with(&zip_bytes, &|_| None)?;

    let Some(first_story) = imported.stories.values().next() else {
        return Err(IdmlError::Invalid("imported ICML produced no story".into()));
    };
    let mut story = (**first_story).clone();
    story.frames.clear();

    for r in moved_ranges {
        let end = r.end.min(story.len());
        let start = r.start.min(end);
        if start < end {
            story.format_chars(start..end, |f| f.over.change = Some(ChangeMark::Moved));
        }
    }

    Ok(IcmlDocument {
        dom_version,
        story_self,
        story_title,
        track_changes: track_changes
            || story.chars.iter().any(|r| matches!(r.format.over.change, Some(ChangeMark::Inserted | ChangeMark::Deleted | ChangeMark::Moved))),
        story,
        styles: (*imported.styles).clone(),
        swatches: imported.swatches,
    })
}

/// Apply an imported [`IcmlDocument`] to `doc`: merges missing swatches and styles, and replaces
/// `target_story`'s content (preserving its frame thread) or creates a new text frame on page 1.
pub fn apply_icml_to_document(doc: &mut Document, icml: &IcmlDocument, target_story: Option<StoryId>) -> Result<StoryId> {
    for sw in &icml.swatches {
        if doc.swatch(&sw.name).is_none() {
            doc.swatches.push(sw.clone());
        }
    }
    let missing_p: Vec<_> = icml.styles.paragraph.iter().filter(|ps| doc.styles.para(&ps.name).is_none()).cloned().collect();
    let missing_c: Vec<_> = icml.styles.character.iter().filter(|cs| doc.styles.char_style(&cs.name).is_none()).cloned().collect();
    if !missing_p.is_empty() || !missing_c.is_empty() {
        let styles = doc.styles_mut();
        styles.paragraph.extend(missing_p);
        styles.character.extend(missing_c);
    }
    if icml.track_changes {
        doc.settings.track_changes = true;
    }

    let sid = if let Some(existing_sid) = target_story.filter(|id| doc.story(*id).is_some()) {
        existing_sid
    } else {
        let layer = doc.default_layer();
        let rect = default_story_frame_rect(doc);
        let (_, new_sid) =
            doc.add_text_frame(SpreadRef::Doc(0), rect, layer, "", ParaFormat::default()).map_err(|e| IdmlError::Invalid(e.to_string()))?;
        new_sid
    };

    if let Some(dst) = doc.story_mut(sid) {
        let frames = std::mem::take(&mut dst.frames);
        let rev = dst.rev.wrapping_add(1);
        let mut replacement = icml.story.clone();
        replacement.id = sid;
        replacement.frames = frames;
        replacement.rev = rev;
        *dst = replacement;
    }
    Ok(sid)
}

fn default_story_frame_rect(doc: &Document) -> Rect {
    if let Some(sp) = doc.spreads.first()
        && let Some(pg) = sp.pages.first()
    {
        let m = &pg.margins;
        let x0 = m.inside.max(36.0);
        let y0 = m.top.max(36.0);
        let x1 = (doc.settings.page_width - m.outside).max(x0 + 144.0);
        let y1 = (doc.settings.page_height - m.bottom).max(y0 + 144.0);
        return Rect::new(x0, y0, x1, y1);
    }
    Rect::new(72.0, 72.0, 540.0, 720.0)
}

/// Walk `<Story>` XML to find exact UTF-8 byte offsets of `<Change ChangeType="MovedText">` spans.
fn collect_moved_byte_ranges(story_el: &El) -> Vec<Range<usize>> {
    struct WalkState {
        offset: usize,
        ranges: Vec<Range<usize>>,
    }

    fn walk(el: &El, in_moved: bool, brk_type: Option<&str>, depth: usize, st: &mut WalkState) {
        if depth > crate::xml::MAX_DEPTH {
            return;
        }
        let local = el.local();
        if matches!(local, "Properties" | "Note" | "StoryPreference" | "InCopyExportOption") {
            return;
        }
        if local == "Content" {
            let start = st.offset;
            for child in &el.children {
                match child {
                    Node::Text(t) | Node::CData(t) => {
                        for c in t.chars().filter(|c| *c != '\u{FEFF}') {
                            let mapped = if matches!(c, '\n' | '\r' | '\u{2029}') { designcraft_doc::story::FORCED_LINE_BREAK } else { c };
                            st.offset += mapped.len_utf8();
                        }
                    }
                    Node::Pi(target, _) if target == "ACE" => {
                        // ACE special markers occupy one PUA char (3 UTF-8 bytes).
                        st.offset += designcraft_doc::story::PAGE_NUMBER.len_utf8();
                    }
                    _ => {}
                }
            }
            if in_moved && st.offset > start {
                push_or_merge_range(&mut st.ranges, start..st.offset);
            }
            return;
        }
        if local == "Br" {
            let start = st.offset;
            if brk_type.is_some() {
                st.offset += designcraft_doc::story::PAGE_BREAK.len_utf8();
            }
            st.offset += 1; // '\n'
            if in_moved && st.offset > start {
                push_or_merge_range(&mut st.ranges, start..st.offset);
            }
            return;
        }
        let next_moved = if local == "Change" {
            match el.get("ChangeType") {
                Some("MovedText") => true,
                Some(_) => false,
                None => in_moved,
            }
        } else {
            in_moved
        };
        let next_brk = if local == "CharacterStyleRange" { el.get("ParagraphBreakType") } else { brk_type };
        for child in &el.children {
            if let Node::El(sub) = child {
                walk(sub, next_moved, next_brk, depth + 1, st);
            }
        }
    }

    let mut st = WalkState { offset: 0, ranges: Vec::new() };
    walk(story_el, false, None, 0, &mut st);
    st.ranges
}

fn push_or_merge_range(ranges: &mut Vec<Range<usize>>, next: Range<usize>) {
    if let Some(last) = ranges.last_mut()
        && last.end == next.start
    {
        last.end = next.end;
        return;
    }
    ranges.push(next);
}

// ---------- InCopy Assignment (.icma) ----------

/// Serialize an [`IcmaAssignment`] manifest as clean-room `.icma` XML bytes.
pub fn export_icma(assignment: &IcmaAssignment) -> Vec<u8> {
    let version = if assignment.dom_version.trim().is_empty() { ICML_DOM_VERSION } else { assignment.dom_version.trim() };
    let mut assign_el = El::new("Assignment")
        .attr("Self", if assignment.self_id.is_empty() { "u_assign_1" } else { &assignment.self_id })
        .attr("Name", &assignment.name)
        .attr("UserName", &assignment.user_name)
        .attr("ExportOptions", "AssignedSpreads")
        .attr("IncludeLinksWhenPackage", "true")
        .attr("FilePath", &assignment.document_path);

    if !assignment.frame_color.is_empty() {
        assign_el.push(El::new("Properties").child(El::new("FrameColor").attr("type", "enumeration").text(&assignment.frame_color)));
    }

    for (idx, entry) in assignment.stories.iter().enumerate() {
        let sid = if entry.self_id.is_empty() { format!("u_assigned_{idx}") } else { entry.self_id.clone() };
        let story_el = El::new("AssignedStory")
            .attr("Self", sid)
            .attr("Name", &entry.name)
            .attr("StoryRef", &entry.story_ref)
            .attr("UserName", &entry.user_name)
            .attr("Locked", if entry.locked { "true" } else { "false" });
        assign_el.push(story_el);
    }

    let root = El::new("Document").attr("DOMVersion", version).attr("Self", "d").child(assign_el);

    with_aid_header(&document(&root), AID_ICMA_PI)
}

/// Parse an InCopy `.icma` assignment manifest from XML bytes.
pub fn import_icma(bytes: &[u8]) -> Result<IcmaAssignment> {
    let root = parse(bytes).map_err(IdmlError::Invalid)?;
    let (dom_version, assign_el) = match root.local() {
        "Document" => {
            let ver = root.get("DOMVersion").unwrap_or(ICML_DOM_VERSION).to_string();
            let assign = root.find("Assignment").ok_or_else(|| IdmlError::Invalid("expected <Assignment> inside .icma <Document>".into()))?;
            (ver, assign)
        }
        "Assignment" => (root.get("DOMVersion").unwrap_or(ICML_DOM_VERSION).to_string(), &root),
        other => {
            return Err(IdmlError::Invalid(format!("expected <Document> or <Assignment> root in .icma, found <{other}>")));
        }
    };

    let frame_color = assign_el.prop("FrameColor").unwrap_or_else(|| "LightBlue".to_string());
    let stories = assign_el
        .find_all("AssignedStory")
        .map(|el| IcmaStoryEntry {
            self_id: el.get("Self").unwrap_or("").to_string(),
            name: el.get("Name").unwrap_or("").to_string(),
            story_ref: el
                .get("StoryRef")
                .or_else(|| el.prop("StoryRef").as_deref().map(|_| ""))
                .map(str::to_string)
                .unwrap_or_else(|| el.prop("StoryRef").unwrap_or_default()),
            user_name: el.get("UserName").unwrap_or("").to_string(),
            locked: el.get("Locked") == Some("true"),
        })
        .collect();

    Ok(IcmaAssignment {
        dom_version,
        self_id: assign_el.get("Self").unwrap_or("u_assign_1").to_string(),
        name: assign_el.get("Name").unwrap_or("Editorial Assignment").to_string(),
        user_name: assign_el.get("UserName").unwrap_or("").to_string(),
        frame_color,
        document_path: assign_el.get("FilePath").unwrap_or("").to_string(),
        stories,
    })
}

/// Build an [`IcmaAssignment`] manifest and per-story `(relative_icml_path, icml_bytes)` pairs
/// for all stories in `doc`.
pub fn export_document_icma(doc: &Document, assignment_name: &str, story_subfolder: &str) -> Result<(IcmaAssignment, Vec<(String, Vec<u8>)>)> {
    let mut entries = Vec::new();
    let mut files = Vec::new();
    let prefix = story_subfolder.trim_matches(['/', '\\']);

    for (&sid, story) in &doc.stories {
        let icml_bytes = export_icml(doc, sid)?;
        let file_name = format!("Story_{}.icml", uid(sid.0));
        let rel_path = if prefix.is_empty() { file_name } else { format!("{prefix}/{file_name}") };
        let preview: String = story.text.lines().next().unwrap_or("").chars().take(32).collect();
        let entry_name = if preview.trim().is_empty() { format!("Story {}", sid.0) } else { preview.trim().to_string() };
        entries.push(IcmaStoryEntry {
            self_id: format!("ua_{}", uid(sid.0)),
            name: entry_name,
            story_ref: rel_path.clone(),
            user_name: String::new(),
            locked: false,
        });
        files.push((rel_path, icml_bytes));
    }

    let assignment = IcmaAssignment {
        dom_version: ICML_DOM_VERSION.to_string(),
        self_id: "u_assign_1".to_string(),
        name: if assignment_name.trim().is_empty() { doc.title.clone() } else { assignment_name.trim().to_string() },
        user_name: String::new(),
        frame_color: "LightBlue".to_string(),
        document_path: format!("{}.designcraft", doc.title),
        stories: entries,
    };

    Ok((assignment, files))
}

#[cfg(test)]
mod tests {
    use designcraft_doc::build::NewDocument;
    use designcraft_doc::{ChangeMark, CharacterStyle, Document, ParaFormat, ParagraphStyle, SpreadRef};
    use designcraft_geom::Rect;

    use super::{
        IcmaAssignment, IcmaStoryEntry, apply_icml_to_document, export_document_icma, export_icma, export_icml_with_version, import_icma,
        import_icml, is_icma, is_icml,
    };
    use crate::is_idms;

    #[test]
    fn icml_round_trips_dom_version_styles_br_and_all_three_change_types() {
        let mut doc = Document::new(&NewDocument::default());
        doc.styles_mut().paragraph.push(ParagraphStyle {
            name: "EditorialLead".into(),
            based_on: None,
            next_style: None,
            shortcut: String::new(),
            para: designcraft_doc::ParaAttrs { space_after: Some(8.0), ..Default::default() },
            chars: designcraft_doc::CharAttrs { size: Some(14.0), ..Default::default() },
        });
        doc.styles_mut().character.push(CharacterStyle {
            name: "Emphasis".into(),
            based_on: None,
            shortcut: String::new(),
            chars: designcraft_doc::CharAttrs { font_style: Some("Italic".into()), ..Default::default() },
        });

        let layer = doc.default_layer();
        let (_, sid) = doc
            .add_text_frame(
                SpreadRef::Doc(0),
                Rect::new(72.0, 72.0, 500.0, 600.0),
                layer,
                "Lead copy added old moved\nSecond paragraph.",
                ParaFormat { style: "EditorialLead".into(), ..Default::default() },
            )
            .unwrap();

        {
            let st = doc.story_mut(sid).unwrap();
            st.paras[1].style = designcraft_doc::BASIC_PARAGRAPH.into();
            st.format_chars(0..4, |f| f.style = "Emphasis".into());
            // "added" -> InsertedText, "old" -> DeletedText, "moved" -> MovedText
            st.format_chars(10..15, |f| f.over.change = Some(ChangeMark::Inserted));
            st.format_chars(16..19, |f| f.over.change = Some(ChangeMark::Deleted));
            st.format_chars(20..25, |f| f.over.change = Some(ChangeMark::Moved));
        }

        let xml_bytes = export_icml_with_version(&doc, sid, "22.0").unwrap();
        assert!(is_icml(&xml_bytes));
        assert!(!is_icma(&xml_bytes));
        assert!(!is_idms(&xml_bytes), "ICML must not be misidentified as an IDMS page-item snippet");

        let xml_str = String::from_utf8_lossy(&xml_bytes);
        assert!(xml_str.contains("DOMVersion=\"22.0\""), "{xml_str}");
        assert!(xml_str.contains("ParagraphStyleRange"), "{xml_str}");
        assert!(xml_str.contains("CharacterStyleRange"), "{xml_str}");
        assert!(xml_str.contains("<Content>"), "{xml_str}");
        assert!(xml_str.contains("<Br/>") || xml_str.contains("<Br />"), "{xml_str}");
        assert!(xml_str.contains("ChangeType=\"InsertedText\""), "{xml_str}");
        assert!(xml_str.contains("ChangeType=\"DeletedText\""), "{xml_str}");
        assert!(xml_str.contains("ChangeType=\"MovedText\""), "{xml_str}");

        let parsed = import_icml(&xml_bytes).unwrap();
        assert_eq!(parsed.dom_version, "22.0");
        assert!(parsed.track_changes);
        assert_eq!(parsed.story.text, "Lead copy added old moved\nSecond paragraph.");
        assert_eq!(parsed.story.paras.len(), 2);
        assert_eq!(parsed.story.paras[0].style, "EditorialLead");
        assert_eq!(parsed.story.format_after(0).style, "Emphasis");
        assert_eq!(parsed.story.format_after(10).over.change, Some(ChangeMark::Inserted));
        assert_eq!(parsed.story.format_after(16).over.change, Some(ChangeMark::Deleted));
        assert_eq!(parsed.story.format_after(20).over.change, Some(ChangeMark::Moved));

        let mut target_doc = Document::new(&NewDocument::default());
        let imported_sid = apply_icml_to_document(&mut target_doc, &parsed, None).unwrap();
        assert_eq!(target_doc.story(imported_sid).unwrap().text, "Lead copy added old moved\nSecond paragraph.");
        assert!(target_doc.styles.para("EditorialLead").is_some());
        assert!(target_doc.styles.char_style("Emphasis").is_some());
    }

    #[test]
    fn icml_imports_external_change_elements_wrapping_character_style_ranges() {
        let external_icml = br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<?aid style="50" type="snippet" readerVersion="6.0" featureSet="257" product="22.0(1)" ?>
<?aid SnippetType="InCopyInterchange"?>
<Document DOMVersion="21.2" Self="d">
  <Story Self="u99" TrackChanges="true" StoryTitle="Feature">
    <ParagraphStyleRange AppliedParagraphStyle="ParagraphStyle/$ID/[No paragraph style]">
      <CharacterStyleRange AppliedCharacterStyle="CharacterStyle/$ID/[No character style]">
        <Content>Opening </Content>
      </CharacterStyleRange>
      <Change Self="ch1" ChangeType="InsertedText" UserName="Editor">
        <CharacterStyleRange AppliedCharacterStyle="CharacterStyle/$ID/[No character style]">
          <Content>fresh</Content>
        </CharacterStyleRange>
      </Change>
      <Change Self="ch2" ChangeType="DeletedText" UserName="Editor">
        <CharacterStyleRange AppliedCharacterStyle="CharacterStyle/$ID/[No character style]">
          <Content>stale</Content>
        </CharacterStyleRange>
      </Change>
      <Change Self="ch3" ChangeType="MovedText" UserName="Editor">
        <CharacterStyleRange AppliedCharacterStyle="CharacterStyle/$ID/[No character style]">
          <Content>relocated</Content>
          <Br/>
        </CharacterStyleRange>
      </Change>
      <CharacterStyleRange AppliedCharacterStyle="CharacterStyle/$ID/[No character style]">
        <Content>Tail</Content>
      </CharacterStyleRange>
    </ParagraphStyleRange>
  </Story>
</Document>"#;

        assert!(is_icml(external_icml));
        let parsed = import_icml(external_icml).unwrap();
        assert_eq!(parsed.dom_version, "21.2");
        assert_eq!(parsed.story_title, "Feature");
        assert_eq!(parsed.story.text, "Opening freshstalerelocated\nTail");
        assert_eq!(parsed.story.format_after(8).over.change, Some(ChangeMark::Inserted));
        assert_eq!(parsed.story.format_after(13).over.change, Some(ChangeMark::Deleted));
        assert_eq!(parsed.story.format_after(18).over.change, Some(ChangeMark::Moved));
    }

    #[test]
    fn icma_assignment_manifest_round_trips_and_exports_document_stories() {
        let manifest = IcmaAssignment {
            dom_version: "22.0".into(),
            self_id: "u_assign_42".into(),
            name: "Cover Package".into(),
            user_name: "J. Kim".into(),
            frame_color: "Gold".into(),
            document_path: "Issue04.designcraft".into(),
            stories: vec![
                IcmaStoryEntry {
                    self_id: "ua1".into(),
                    name: "Cover Headline".into(),
                    story_ref: "Assignments/Story_u1.icml".into(),
                    user_name: "J. Kim".into(),
                    locked: false,
                },
                IcmaStoryEntry {
                    self_id: "ua2".into(),
                    name: "Sidebar Deck".into(),
                    story_ref: "Assignments/Story_u2.icml".into(),
                    user_name: "A. Park".into(),
                    locked: true,
                },
            ],
        };

        let bytes = export_icma(&manifest);
        assert!(is_icma(&bytes));
        assert!(!is_icml(&bytes));
        assert!(!is_idms(&bytes));

        let back = import_icma(&bytes).unwrap();
        assert_eq!(back, manifest);

        let mut doc = Document::new(&NewDocument::default());
        let layer = doc.default_layer();
        doc.add_text_frame(
            SpreadRef::Doc(0),
            Rect::new(72.0, 72.0, 300.0, 200.0),
            layer,
            "Feature Story Headline\nBody text.",
            ParaFormat::default(),
        )
        .unwrap();
        let (doc_assign, story_files) = export_document_icma(&doc, "Spring Feature", "Stories").unwrap();
        assert_eq!(doc_assign.stories.len(), 1);
        assert_eq!(story_files.len(), 1);
        assert!(is_icml(&story_files[0].1));
    }
}
