//! InCopy `.icml` / `.icma` interchange commands (`file.importIcml`, `file.exportIcml`) and
//! the Copyfit Editorial Calculator (`CopyfitReport`, `story.copyfit`).

use designcraft_compose::ComposedStory;
use designcraft_doc::{Document, ItemId, Selection, StoryId, TextSel};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use super::file::{base64_decode, base64_encode};
use super::text::story_of;
use super::{CommandSpec, bad, bool_or, cmd, has_doc, str_param};
use crate::{EngineError, Result, Session};

/// Editorial copyfitting metrics for a story across its threaded text frames.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CopyfitReport {
    pub story: u64,
    pub char_count: usize,
    pub word_count: usize,
    pub actual_lines: usize,
    pub estimated_lines: usize,
    pub frame_depth_used: f64,
    pub target_depth: f64,
    pub overset: bool,
    pub overset_chars: usize,
    pub overset_words: usize,
    pub overset_lines: usize,
    pub underset_lines: usize,
    pub underset_words: usize,
}

/// Compute editorial copyfitting metrics for `sid` given its composed layout `cs`.
pub fn compute_copyfit(doc: &Document, cs: &ComposedStory, sid: StoryId, target_depth_override: Option<f64>) -> Option<CopyfitReport> {
    let story = doc.story(sid)?;
    let spans = doc.visible_spans(story);

    let mut visible_text = String::with_capacity(story.len());
    for r in &spans {
        if let Some(slice) = story.text.get(r.clone()) {
            visible_text.push_str(slice);
        }
    }
    let char_count = visible_text.chars().count();
    let word_count = visible_text.split_whitespace().count();

    let actual_lines = cs.line_count();
    let frame_depth_used = story_depth_used(cs);
    let computed_target_depth = story_target_depth(doc, cs, sid);
    let target_depth = target_depth_override.filter(|d| d.is_finite() && *d >= 0.0).unwrap_or(computed_target_depth);

    let avg_line_height = average_line_height(doc, cs, sid);
    let fit_end = cs.overset_at.unwrap_or(story.len()).min(story.len());
    let (fit_chars, fit_words) = visible_counts_in_range(story, &spans, 0..fit_end);

    let avg_chars_per_line = if actual_lines > 0 && fit_chars > 0 { (fit_chars as f64 / actual_lines as f64).max(1.0) } else { 45.0 };
    let avg_words_per_line = if actual_lines > 0 && fit_words > 0 { (fit_words as f64 / actual_lines as f64).max(1.0) } else { 8.0 };

    if let Some(overset_byte) = cs.overset_at {
        let (over_chars, over_words) = visible_counts_in_range(story, &spans, overset_byte.min(story.len())..story.len());
        let mut over_lines = if over_chars > 0 { ((over_chars as f64) / avg_chars_per_line).ceil() as usize } else { 1 };
        if frame_depth_used > target_depth + 1e-3 && avg_line_height > 0.0 {
            let depth_over = ((frame_depth_used - target_depth) / avg_line_height).ceil() as usize;
            over_lines = over_lines.max(depth_over);
        }
        Some(CopyfitReport {
            story: sid.0,
            char_count,
            word_count,
            actual_lines,
            estimated_lines: actual_lines.saturating_add(over_lines),
            frame_depth_used,
            target_depth,
            overset: true,
            overset_chars: over_chars,
            overset_words: over_words,
            overset_lines: over_lines.max(1),
            underset_lines: 0,
            underset_words: 0,
        })
    } else if target_depth > 0.0 && frame_depth_used > target_depth + 1e-3 {
        let excess = frame_depth_used - target_depth;
        let over_lines = if avg_line_height > 0.0 { (excess / avg_line_height).ceil().max(1.0) as usize } else { 1 };
        let over_words = ((over_lines as f64) * avg_words_per_line).round() as usize;
        let over_chars = ((over_lines as f64) * avg_chars_per_line).round() as usize;
        Some(CopyfitReport {
            story: sid.0,
            char_count,
            word_count,
            actual_lines,
            estimated_lines: actual_lines,
            frame_depth_used,
            target_depth,
            overset: true,
            overset_chars: over_chars,
            overset_words: over_words,
            overset_lines: over_lines,
            underset_lines: 0,
            underset_words: 0,
        })
    } else {
        let estimated_lines =
            if cs.frames.is_empty() && char_count > 0 { ((char_count as f64) / avg_chars_per_line).ceil() as usize } else { actual_lines };
        let slack = (target_depth - frame_depth_used).max(0.0);
        let underset_lines = if avg_line_height > 0.0 { (slack / avg_line_height).floor() as usize } else { 0 };
        let underset_words = ((underset_lines as f64) * avg_words_per_line).round() as usize;
        Some(CopyfitReport {
            story: sid.0,
            char_count,
            word_count,
            actual_lines,
            estimated_lines,
            frame_depth_used,
            target_depth,
            overset: false,
            overset_chars: 0,
            overset_words: 0,
            overset_lines: 0,
            underset_lines,
            underset_words,
        })
    }
}

fn visible_counts_in_range(story: &designcraft_doc::Story, spans: &[std::ops::Range<usize>], window: std::ops::Range<usize>) -> (usize, usize) {
    let mut sub = String::new();
    for r in spans {
        let a = r.start.max(window.start);
        let b = r.end.min(window.end);
        if a < b
            && let Some(s) = story.text.get(a..b)
        {
            sub.push_str(s);
        }
    }
    (sub.chars().count(), sub.split_whitespace().count())
}

fn story_depth_used(cs: &ComposedStory) -> f64 {
    let mut depth = 0.0;
    for ft in &cs.frames {
        let mut covered: std::collections::BTreeMap<u32, Vec<(f64, f64)>> = std::collections::BTreeMap::new();
        let mut before: Option<(u32, f64)> = None;
        for line in &ft.lines {
            let bottom = line.baseline + line.descent;
            let mut top = bottom - line.leading.max(line.ascent + line.descent);
            if let Some((col, above)) = before
                && col == line.column
                && above < top
            {
                top = above;
            }
            before = Some((line.column, bottom));
            if top.is_finite() && bottom > top {
                depth += cover_band(covered.entry(line.column).or_default(), top, bottom);
            }
        }
    }
    depth
}

fn cover_band(bands: &mut Vec<(f64, f64)>, top: f64, bottom: f64) -> f64 {
    let overlap: f64 = bands.iter().map(|&(a, b)| (b.min(bottom) - a.max(top)).max(0.0)).sum();
    let (mut a, mut b) = (top, bottom);
    bands.retain(|&(s, e)| {
        let touches = s <= b + 1e-6 && e >= a - 1e-6;
        if touches {
            (a, b) = (a.min(s), b.max(e));
        }
        !touches
    });
    let at = bands.partition_point(|&(s, _)| s < a);
    bands.insert(at, (a, b));
    (bottom - top - overlap).max(0.0)
}

fn story_target_depth(doc: &Document, cs: &ComposedStory, sid: StoryId) -> f64 {
    let from_composed: f64 = cs
        .frames
        .iter()
        .flat_map(|ft| ft.columns.iter())
        .map(|c| {
            let h = c.height();
            if h.is_finite() && h > 0.0 { h } else { 0.0 }
        })
        .sum();
    if from_composed > 0.0 {
        return from_composed;
    }
    let Some(story) = doc.story(sid) else {
        return 0.0;
    };
    story
        .frames
        .iter()
        .filter_map(|&fid| doc.item(fid))
        .map(|item| {
            let b = item.bounds();
            let cols = item.text_frame().map_or(1, |tf| tf.options.columns.max(1)) as f64;
            let inset_v = item.text_frame().map_or(0.0, |tf| tf.options.inset[0] + tf.options.inset[2]);
            (b.height() - inset_v).max(0.0) * cols
        })
        .sum()
}

fn average_line_height(doc: &Document, cs: &ComposedStory, sid: StoryId) -> f64 {
    let mut sum = 0.0;
    let mut count = 0usize;
    for ft in &cs.frames {
        for line in &ft.lines {
            let h = line.leading.max(line.ascent + line.descent);
            if h.is_finite() && h > 0.0 {
                sum += h;
                count += 1;
            }
        }
    }
    if count > 0 {
        return (sum / count as f64).max(1.0);
    }
    if let Some(story) = doc.story(sid)
        && let Some(pf) = story.paras.first()
    {
        let (_, base) = doc.styles.resolve_para(pf);
        let cf = story.char_format_at(0);
        let cp = doc.styles.resolve_char(&base, cf);
        let lead = match cp.leading {
            designcraft_doc::Leading::Points(pt) if pt.is_finite() && pt > 0.0 => pt,
            _ => cp.size * 1.2,
        };
        if lead.is_finite() && lead > 0.0 {
            return lead;
        }
    }
    14.4
}

fn resolve_story_target(s: &Session, p: &Value, cmd_id: &str) -> Result<StoryId> {
    if let Some(sid) = story_of(s, p) {
        let doc = &s.doc()?.doc;
        if doc.story(sid).is_some() {
            return Ok(sid);
        }
        return Err(bad(cmd_id, format!("no such story {}", sid.0)));
    }
    let st = s.doc()?;
    if let Some((&sid, _)) = st.doc.stories.iter().next() {
        return Ok(sid);
    }
    Err(bad(cmd_id, "no story available: select text or a text frame, or pass `story` or `frame`"))
}

pub(crate) fn import_icml_cmd(s: &mut Session, p: &Value) -> Result<Value> {
    const CMD: &str = "file.importIcml";
    let (bytes, base_path) = if let Some(xml) = str_param(p, "xml") {
        (xml.as_bytes().to_vec(), None)
    } else if let Some(b64) = str_param(p, "base64") {
        (base64_decode(b64), None)
    } else if let Some(path) = str_param(p, "path") {
        #[cfg(not(target_arch = "wasm32"))]
        let b = crate::io::read_file(path).map_err(EngineError::Other)?;
        #[cfg(target_arch = "wasm32")]
        let b: Vec<u8> = {
            let _ = path;
            return Err(EngineError::Other("importing .icml from disk requires a file system".into()));
        };
        (b, Some(std::path::PathBuf::from(path)))
    } else {
        return Err(bad(CMD, "provide `path`, `base64` or `xml`"));
    };

    if designcraft_idml::is_icma(&bytes) {
        let manifest = designcraft_idml::import_icma(&bytes).map_err(|e| EngineError::Other(e.to_string()))?;
        let mut imported_stories: Vec<u64> = Vec::new();
        #[cfg(not(target_arch = "wasm32"))]
        if let Some(assign_path) = base_path.as_deref() {
            let mut loaded_docs = Vec::new();
            for entry in &manifest.stories {
                if entry.story_ref.is_empty() {
                    continue;
                }
                if let Ok(story_path) = crate::io::document_path(&entry.story_ref, Some(assign_path))
                    && let Ok(story_bytes) = crate::io::read_file(&story_path)
                    && let Ok(icml) = designcraft_idml::import_icml(&story_bytes)
                {
                    loaded_docs.push(icml);
                }
            }
            if !loaded_docs.is_empty() {
                s.edit(|doc, sel| {
                    for icml in &loaded_docs {
                        let sid = designcraft_idml::apply_icml_to_document(doc, icml, None).map_err(|e| EngineError::Other(e.to_string()))?;
                        imported_stories.push(sid.0);
                    }
                    if let Some(&last_sid) = imported_stories.last() {
                        *sel = Selection::text(TextSel::caret(StoryId(last_sid), 0));
                    }
                    Ok(Value::Null)
                })?;
            }
        }
        #[cfg(target_arch = "wasm32")]
        let _ = &base_path;

        let story_entries: Vec<Value> = manifest
            .stories
            .iter()
            .map(|e| {
                json!({
                    "self": e.self_id,
                    "name": e.name,
                    "storyRef": e.story_ref,
                    "userName": e.user_name,
                    "locked": e.locked,
                })
            })
            .collect();
        return Ok(json!({
            "domVersion": manifest.dom_version,
            "assignment": {
                "name": manifest.name,
                "userName": manifest.user_name,
                "documentPath": manifest.document_path,
                "stories": story_entries,
            },
            "stories": imported_stories,
        }));
    }

    let icml = designcraft_idml::import_icml(&bytes).map_err(|e| EngineError::Other(e.to_string()))?;

    let target_story = if let Some(sid) = p.get("story").and_then(Value::as_u64) {
        let sid = StoryId(sid);
        if s.doc()?.doc.story(sid).is_none() {
            return Err(bad(CMD, format!("no such story {}", sid.0)));
        }
        Some(sid)
    } else if let Some(fid) = p.get("frame").and_then(Value::as_u64) {
        let item = s.doc()?.doc.item(ItemId(fid)).ok_or_else(|| bad(CMD, format!("no such frame {fid}")))?;
        let tf = item.text_frame().ok_or_else(|| bad(CMD, format!("frame {fid} holds no text")))?;
        Some(tf.story)
    } else {
        story_of(s, p)
    };

    let dom_version = icml.dom_version.clone();
    let track_changes = icml.track_changes;
    let sid = s.edit(|doc, sel| {
        let applied = designcraft_idml::apply_icml_to_document(doc, &icml, target_story).map_err(|e| EngineError::Other(e.to_string()))?;
        let frame_id = doc.story(applied).and_then(|st| st.frames.first().copied());
        if let Some(fid) = frame_id {
            *sel = Selection::items(vec![fid]);
        } else {
            *sel = Selection::text(TextSel::caret(applied, 0));
        }
        Ok(applied)
    })?;

    Ok(json!({
        "story": sid.0,
        "stories": [sid.0],
        "domVersion": dom_version,
        "trackChanges": track_changes,
    }))
}

fn export_icml_cmd(s: &mut Session, p: &Value) -> Result<Value> {
    const CMD: &str = "file.exportIcml";
    let want_assignment = bool_or(p, "assignment", false)
        || str_param(p, "path").is_some_and(|pt| {
            std::path::Path::new(pt).extension().and_then(std::ffi::OsStr::to_str).is_some_and(|ext| ext.eq_ignore_ascii_case("icma"))
        });

    if want_assignment {
        let doc = &s.doc()?.doc;
        let name = str_param(p, "name").unwrap_or(&doc.title);
        let (manifest, story_files) = designcraft_idml::export_document_icma(doc, name, "Stories").map_err(|e| EngineError::Other(e.to_string()))?;
        let icma_bytes = designcraft_idml::export_icma(&manifest);
        let xml = String::from_utf8_lossy(&icma_bytes).into_owned();
        if let Some(path) = str_param(p, "path") {
            #[cfg(not(target_arch = "wasm32"))]
            {
                crate::io::write_file(path, &icma_bytes).map_err(EngineError::Other)?;
                if let Some(parent) = std::path::Path::new(path).parent()
                    && !parent.as_os_str().is_empty()
                {
                    let stories_dir = parent.join("Stories");
                    crate::io::create_dir_all(&stories_dir).map_err(EngineError::Other)?;
                    for (rel, data) in &story_files {
                        let out_path = parent.join(rel);
                        crate::io::write_file(&out_path, data).map_err(EngineError::Other)?;
                    }
                }
            }
            return Ok(json!({
                "path": path,
                "xml": xml,
                "base64": base64_encode(&icma_bytes),
                "bytes": icma_bytes.len(),
                "stories": story_files.len(),
            }));
        }
        return Ok(json!({
            "xml": xml,
            "base64": base64_encode(&icma_bytes),
            "bytes": icma_bytes.len(),
            "stories": story_files.len(),
        }));
    }

    let sid = resolve_story_target(s, p, CMD)?;
    let doc = &s.doc()?.doc;
    let dom_version = str_param(p, "domVersion").unwrap_or(designcraft_idml::ICML_DOM_VERSION);
    let bytes = designcraft_idml::export_icml_with_version(doc, sid, dom_version).map_err(|e| EngineError::Other(e.to_string()))?;
    let xml = String::from_utf8_lossy(&bytes).into_owned();
    if let Some(path) = str_param(p, "path") {
        #[cfg(not(target_arch = "wasm32"))]
        crate::io::write_file(path, &bytes).map_err(EngineError::Other)?;
        return Ok(json!({
            "path": path,
            "story": sid.0,
            "xml": xml,
            "base64": base64_encode(&bytes),
            "bytes": bytes.len(),
        }));
    }
    Ok(json!({
        "story": sid.0,
        "xml": xml,
        "base64": base64_encode(&bytes),
        "bytes": bytes.len(),
    }))
}

fn story_copyfit_cmd(s: &mut Session, p: &Value) -> Result<Value> {
    const CMD: &str = "story.copyfit";
    let target_depth = match p.get("targetDepth") {
        None | Some(Value::Null) => None,
        Some(v) => Some(
            v.as_f64()
                .filter(|d| d.is_finite() && *d >= 0.0)
                .ok_or_else(|| bad(CMD, "`targetDepth` must be a non-negative finite number of points"))?,
        ),
    };
    let sid = resolve_story_target(s, p, CMD)?;
    let doc = &s.doc()?.doc;
    let cs = s.cache.get(doc, sid, None);
    let report = compute_copyfit(doc, &cs, sid, target_depth).ok_or_else(|| bad(CMD, format!("no such story {}", sid.0)))?;
    serde_json::to_value(report).map_err(|e| EngineError::Other(e.to_string()))
}

pub fn specs() -> Vec<CommandSpec> {
    vec![
        cmd!(
            "file.importIcml",
            "Import InCopy Story…",
            [],
            None,
            "{path?, base64?, xml?, name?, story?, frame?} — import an InCopy (.icml) story or (.icma) assignment into the target story/frame or a new text frame → {story, stories, domVersion, trackChanges}",
            has_doc,
            import_icml_cmd
        ),
        cmd!(
            query "file.exportIcml",
            "Export InCopy Story…",
            [],
            None,
            "{path?, story?, frame?, name?, domVersion?, assignment?: bool} — export a story as InCopy Markup (.icml) or the document's stories as an InCopy Assignment (.icma) → {path?, story?, xml, base64, bytes}",
            has_doc,
            export_icml_cmd
        ),
        cmd!(
            query "story.copyfit",
            "Copyfit Info",
            [],
            None,
            "{story?, frame?, targetDepth?: pt} — editorial copyfitting metrics for the story (character/word/line counts, frame depth used vs target depth, overset/underset lines and words) → {story, charCount, wordCount, actualLines, estimatedLines, frameDepthUsed, targetDepth, overset, oversetChars, oversetWords, oversetLines, undersetLines, undersetWords}",
            has_doc,
            story_copyfit_cmd
        ),
    ]
}

#[cfg(test)]
mod tests {
    use designcraft_doc::ChangeMark;
    use serde_json::json;

    use super::CopyfitReport;
    use crate::Session;

    #[test]
    fn copyfit_reports_underset_and_overset_lines_and_words() {
        let mut s = Session::new();
        s.execute("file.new", &json!({})).unwrap();
        let short = s
            .execute("frame.create", &json!({"rect": [72, 72, 320, 272], "content": "text", "text": "Short editorial deck with room below."}))
            .unwrap();
        let sid = short["story"].as_u64().unwrap();

        let fit_val = s.execute("story.copyfit", &json!({"story": sid})).unwrap();
        let fit: CopyfitReport = serde_json::from_value(fit_val).unwrap();
        assert_eq!(fit.word_count, 6);
        assert!(fit.actual_lines >= 1);
        assert!(!fit.overset);
        assert_eq!(fit.overset_lines, 0);
        assert_eq!(fit.overset_words, 0);
        assert!(fit.target_depth > fit.frame_depth_used);
        assert!(fit.underset_lines >= 5, "{fit:?}");
        assert!(fit.underset_words > 0, "{fit:?}");

        let long_copy = "Editorial feature paragraph with enough words to overflow a compact magazine column box. ".repeat(24);
        s.execute("story.setText", &json!({"story": sid, "text": long_copy})).unwrap();
        let over_val = s.execute("story.copyfit", &json!({"story": sid})).unwrap();
        let over: CopyfitReport = serde_json::from_value(over_val).unwrap();
        assert!(over.overset, "{over:?}");
        assert!(over.overset_chars > 0);
        assert!(over.overset_words > 0);
        assert!(over.overset_lines >= 1);
        assert!(over.estimated_lines > over.actual_lines);
        assert_eq!(over.underset_lines, 0);
        assert_eq!(over.underset_words, 0);
    }

    #[test]
    fn engine_export_and_import_icml_preserves_changes_and_styles() {
        let mut s = Session::new();
        s.execute("file.new", &json!({})).unwrap();
        s.execute("style.paragraph.create", &json!({"name": "Byline", "chars": {"size": 11}})).unwrap();
        let created = s
            .execute("frame.create", &json!({"rect": [72, 72, 400, 300], "content": "text", "text": "By Jane Doe\nInserted Deleted Moved"}))
            .unwrap();
        let sid = designcraft_doc::StoryId(created["story"].as_u64().unwrap());
        s.execute("text.select", &json!({"story": sid.0, "anchor": 0, "focus": 0})).unwrap();
        s.execute("style.paragraph.apply", &json!({"name": "Byline"})).unwrap();
        s.edit(|doc, _| {
            let st = doc.story_mut(sid).unwrap();
            st.format_chars(12..20, |f| f.over.change = Some(ChangeMark::Inserted));
            st.format_chars(21..28, |f| f.over.change = Some(ChangeMark::Deleted));
            st.format_chars(29..34, |f| f.over.change = Some(ChangeMark::Moved));
            Ok(serde_json::Value::Null)
        })
        .unwrap();

        let exported = s.execute("file.exportIcml", &json!({"story": sid.0})).unwrap();
        let xml = exported["xml"].as_str().unwrap();
        assert!(xml.contains("ChangeType=\"InsertedText\""));
        assert!(xml.contains("ChangeType=\"DeletedText\""));
        assert!(xml.contains("ChangeType=\"MovedText\""));

        let mut dest = Session::new();
        dest.execute("file.new", &json!({})).unwrap();
        let imp = dest.execute("file.importIcml", &json!({"xml": xml})).unwrap();
        assert_eq!(imp["trackChanges"], true);
        let new_sid = designcraft_doc::StoryId(imp["story"].as_u64().unwrap());
        let st = dest.doc().unwrap().doc.story(new_sid).unwrap();
        assert_eq!(st.text, "By Jane Doe\nInserted Deleted Moved");
        assert_eq!(st.paras[0].style, "Byline");
        assert_eq!(st.format_after(12).over.change, Some(ChangeMark::Inserted));
        assert_eq!(st.format_after(21).over.change, Some(ChangeMark::Deleted));
        assert_eq!(st.format_after(29).over.change, Some(ChangeMark::Moved));

        let changes = dest.execute("changes.list", &json!({})).unwrap();
        let kinds: Vec<&str> = changes.as_array().unwrap().iter().filter_map(|c| c["kind"].as_str()).collect();
        assert_eq!(kinds, ["inserted", "deleted", "moved"]);
    }
}
