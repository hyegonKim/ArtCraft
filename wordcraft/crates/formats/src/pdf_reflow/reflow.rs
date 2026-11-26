//! Geometric line assembly and paragraph / heading / list reflow engine for PDF spans.

use std::collections::HashMap;
use std::sync::Arc;

use wordcraft_doc::{Align, Rgb};

use crate::model::{FBlock, Fmt, Inline, Kind, ListInfo, Para, Spacing, kept_link};

use super::content::TextSpan;
use super::parser::LinkAnnot;

pub(super) struct PageSpans {
    pub width: f64,
    pub height: f64,
    pub spans: Vec<TextSpan>,
    pub links: Vec<LinkAnnot>,
}

#[derive(Clone, Debug)]
struct RichSeg {
    text: String,
    fmt: Fmt,
    size: f32,
    bold: bool,
}

#[derive(Clone, Debug)]
struct VisualLine {
    x: f64,
    end_x: f64,
    y: f64,
    dominant_size: f32,
    all_bold: bool,
    char_count: usize,
    segs: Vec<RichSeg>,
}

impl VisualLine {
    fn plain_text(&self) -> String {
        let mut s = String::new();
        for seg in &self.segs {
            s.push_str(&seg.text);
        }
        s
    }
}

pub(super) fn reflow_document(pages: &[PageSpans]) -> Vec<FBlock> {
    let mut page_lines: Vec<(f64, Vec<VisualLine>)> = Vec::with_capacity(pages.len());
    for p in pages {
        let _ = p.height;
        let lines = assemble_page_lines(&p.spans, &p.links);
        page_lines.push((p.width, lines));
    }

    let body_size = infer_body_font_size(&page_lines);
    let mut blocks: Vec<FBlock> = Vec::new();
    let mut had_any_content = false;

    for (page_idx, (page_width, lines)) in page_lines.iter().enumerate() {
        if lines.is_empty() {
            continue;
        }
        let metrics = infer_page_metrics(lines, body_size, *page_width);
        let page_blocks = reflow_page_lines(lines, body_size, &metrics, page_idx > 0 && had_any_content);
        if !page_blocks.is_empty() {
            had_any_content = true;
            blocks.extend(page_blocks);
        }
    }

    blocks
}

fn assemble_page_lines(spans: &[TextSpan], links: &[LinkAnnot]) -> Vec<VisualLine> {
    let mut order: Vec<usize> = (0..spans.len())
        .filter(|&i| !spans[i].text.is_empty())
        .collect();

    // Sort top-to-bottom (`y` descending), then left-to-right (`x` ascending).
    order.sort_by(|&a, &b| {
        let ya = spans[a].y;
        let yb = spans[b].y;
        let tol = (f64::from(spans[a].size.min(spans[b].size)) * 0.35).max(2.0);
        if (ya - yb).abs() > tol {
            yb.total_cmp(&ya)
        } else {
            spans[a].x.total_cmp(&spans[b].x)
        }
    });

    let mut raw_clusters: Vec<Vec<&TextSpan>> = Vec::new();
    for idx in order {
        let sp = &spans[idx];
        let joined = if let Some(last_cluster) = raw_clusters.last_mut()
            && let Some(first) = last_cluster.first()
        {
            let avg_y = last_cluster.iter().map(|s| s.y).sum::<f64>() / (last_cluster.len() as f64);
            let min_sz = f64::from(first.size.min(sp.size));
            if (avg_y - sp.y).abs() <= (min_sz * 0.42).max(2.5) {
                last_cluster.push(sp);
                true
            } else {
                false
            }
        } else {
            false
        };
        if !joined {
            raw_clusters.push(vec![sp]);
        }
    }

    let mut out = Vec::with_capacity(raw_clusters.len());
    for mut cluster in raw_clusters {
        cluster.sort_by(|a, b| a.x.total_cmp(&b.x));
        if let Some(line) = build_visual_line(&cluster, links) {
            out.push(line);
        }
    }
    out
}

fn build_visual_line(cluster: &[&TextSpan], links: &[LinkAnnot]) -> Option<VisualLine> {
    let mut segs: Vec<RichSeg> = Vec::new();
    let mut line_x = f64::INFINITY;
    let mut line_end_x = f64::NEG_INFINITY;
    let mut y_sum = 0.0;
    let mut y_count = 0usize;
    let mut max_size = 0.0f32;
    let mut size_weights: HashMap<i32, usize> = HashMap::new();
    let mut bold_chars = 0usize;
    let mut total_non_ws = 0usize;

    let mut prev_end_x: Option<f64> = None;
    let mut prev_size: f32 = 12.0;
    let mut prev_x: f64 = 0.0;
    let mut prev_raw_text: &str = "";

    for &sp in cluster {
        // Detect duplicate shadow-bold glyph overlays drawn at nearly identical `(x, y)`.
        if !prev_raw_text.is_empty()
            && sp.text == prev_raw_text
            && (sp.x - prev_x).abs() <= f64::from(sp.size) * 0.14
        {
            if let Some(last_seg) = segs.last_mut() {
                last_seg.bold = true;
                last_seg.fmt.bold = true;
            }
            bold_chars = bold_chars.saturating_add(sp.text.chars().filter(|c| !c.is_whitespace()).count());
            line_end_x = line_end_x.max(sp.end_x);
            prev_end_x = Some(prev_end_x.unwrap_or(sp.end_x).max(sp.end_x));
            continue;
        }

        let rounded_size = quantize_pt(sp.size);
        let link = match_link(sp.raw_bbox, links);
        let fmt = Fmt {
            bold: sp.bold,
            italic: sp.italic,
            font: sp.font.clone(),
            size: Some(rounded_size),
            color: (sp.color != Rgb::BLACK).then_some(sp.color),
            link,
            ..Default::default()
        };

        if let Some(pe) = prev_end_x {
            let gap = sp.x - pe;
            let min_sz = f64::from(prev_size.min(sp.size));
            let prev_ends_ws = segs.last().is_some_and(|s| s.text.ends_with(char::is_whitespace));
            let curr_starts_ws = sp.text.starts_with(char::is_whitespace);
            if !prev_ends_ws && !curr_starts_ws && gap >= (min_sz * 0.18).max(1.2) {
                let space_insert = segs.last_mut().and_then(|last| {
                    if last.fmt == fmt {
                        last.text.push(' ');
                        None
                    } else {
                        let mut space_fmt = last.fmt.clone();
                        if last.fmt.link != fmt.link {
                            space_fmt.link = None;
                        }
                        Some((space_fmt, last.size, last.bold))
                    }
                });
                if let Some((space_fmt, l_size, l_bold)) = space_insert {
                    push_seg(&mut segs, " ", space_fmt, l_size, l_bold);
                }
            }
        }

        let non_ws = sp.text.chars().filter(|c| !c.is_whitespace()).count();
        if non_ws > 0 {
            let bucket = (rounded_size * 2.0).round() as i32;
            *size_weights.entry(bucket).or_insert(0) += non_ws;
            total_non_ws += non_ws;
            if sp.bold {
                bold_chars += non_ws;
            }
        }

        line_x = line_x.min(sp.x);
        line_end_x = line_end_x.max(sp.end_x);
        y_sum += sp.y;
        y_count += 1;
        max_size = max_size.max(rounded_size);
        prev_end_x = Some(prev_end_x.map_or(sp.end_x, |pe| pe.max(sp.end_x)));
        prev_size = rounded_size;
        prev_x = sp.x;
        prev_raw_text = &sp.text;

        push_seg(&mut segs, &sp.text, fmt, rounded_size, sp.bold);
    }

    trim_line_edges(&mut segs);
    if segs.is_empty() || total_non_ws == 0 {
        return None;
    }

    let dominant_bucket = size_weights
        .into_iter()
        .max_by_key(|&(bucket, weight)| (weight, -bucket))
        .map(|(b, _)| b)
        .unwrap_or((max_size * 2.0).round() as i32);
    let dominant_size = (dominant_bucket as f32) * 0.5;
    let all_bold = bold_chars * 10 >= total_non_ws * 9;

    Some(VisualLine {
        x: if line_x.is_finite() { line_x } else { 0.0 },
        end_x: if line_end_x.is_finite() { line_end_x } else { 0.0 },
        y: y_sum / (y_count.max(1) as f64),
        dominant_size,
        all_bold,
        char_count: segs.iter().map(|s| s.text.chars().count()).sum(),
        segs,
    })
}

fn push_seg(segs: &mut Vec<RichSeg>, text: &str, fmt: Fmt, size: f32, bold: bool) {
    if text.is_empty() {
        return;
    }
    if let Some(last) = segs.last_mut()
        && last.fmt == fmt
    {
        last.text.push_str(text);
        return;
    }
    segs.push(RichSeg {
        text: text.to_string(),
        fmt,
        size,
        bold,
    });
}

fn trim_line_edges(segs: &mut Vec<RichSeg>) {
    while let Some(first) = segs.first_mut() {
        let trimmed = first.text.trim_start_matches([' ', '\t', '\r', '\n']);
        if trimmed.is_empty() {
            segs.remove(0);
        } else {
            if trimmed.len() != first.text.len() {
                first.text = trimmed.to_string();
            }
            break;
        }
    }
    while let Some(last) = segs.last_mut() {
        let trimmed = last.text.trim_end_matches([' ', '\t', '\r', '\n']);
        if trimmed.is_empty() {
            segs.pop();
        } else {
            if trimmed.len() != last.text.len() {
                last.text = trimmed.to_string();
            }
            break;
        }
    }
}

fn match_link(bbox: [f64; 4], links: &[LinkAnnot]) -> Option<Arc<str>> {
    if links.is_empty() {
        return None;
    }
    let cx = 0.5 * (bbox[0] + bbox[2]);
    let cy = 0.5 * (bbox[1] + bbox[3]);
    for l in links {
        let pad = 2.5;
        if cx >= l.rect[0] - pad
            && cx <= l.rect[2] + pad
            && cy >= l.rect[1] - pad
            && cy <= l.rect[3] + pad
        {
            return kept_link(&l.uri);
        }
    }
    None
}

fn quantize_pt(size: f32) -> f32 {
    ((size * 2.0).round() * 0.5).clamp(1.0, 400.0)
}

fn infer_body_font_size(pages: &[(f64, Vec<VisualLine>)]) -> f32 {
    let mut weights: HashMap<i32, usize> = HashMap::new();
    for (_w, lines) in pages {
        for line in lines {
            let bucket = (line.dominant_size * 2.0).round() as i32;
            *weights.entry(bucket).or_insert(0) += line.char_count.max(1);
        }
    }
    weights
        .into_iter()
        .max_by_key(|&(bucket, count)| (count, -bucket))
        .map(|(b, _)| (b as f32) * 0.5)
        .unwrap_or(11.0)
        .clamp(6.0, 36.0)
}

struct PageMetrics {
    body_left: f64,
    col_right: f64,
    typical_leading: Option<f64>,
    page_width: f64,
}

fn infer_page_metrics(lines: &[VisualLine], body_size: f32, page_width: f64) -> PageMetrics {
    let mut lefts: Vec<f64> = lines
        .iter()
        .filter(|l| (l.dominant_size - body_size).abs() <= 1.5)
        .map(|l| l.x)
        .collect();
    if lefts.is_empty() {
        lefts = lines.iter().map(|l| l.x).collect();
    }
    lefts.sort_by(|a, b| a.total_cmp(b));
    let body_left = lefts.first().copied().unwrap_or(72.0);

    let col_right = lines
        .iter()
        .map(|l| l.end_x)
        .fold(body_left + 200.0, f64::max);

    let mut leadings: Vec<f64> = Vec::new();
    for pair in lines.windows(2) {
        let dy = pair[0].y - pair[1].y;
        let sz = f64::from(pair[0].dominant_size.max(pair[1].dominant_size));
        if (pair[0].dominant_size - body_size).abs() <= 1.5
            && (pair[1].dominant_size - body_size).abs() <= 1.5
            && dy >= sz * 0.80
            && dy <= sz * 1.95
        {
            leadings.push(dy);
        }
    }
    leadings.sort_by(|a, b| a.total_cmp(b));
    let typical_leading = if !leadings.is_empty() {
        // Use lower-quartile / median of single-spaced gaps so paragraph gaps don't skew leading upward.
        Some(leadings[leadings.len() / 3])
    } else {
        None
    };

    PageMetrics {
        body_left,
        col_right,
        typical_leading,
        page_width,
    }
}

fn classify_line_kind(line: &VisualLine, next_line: Option<&VisualLine>, body_size: f32) -> Kind {
    if line.char_count > 180 {
        return Kind::Normal;
    }
    let ratio = line.dominant_size / body_size.max(1.0);
    if ratio >= 1.65 || (ratio >= 1.55 && line.all_bold) {
        return Kind::Heading(1);
    }
    if ratio >= 1.32 || (ratio >= 1.26 && line.all_bold) {
        return Kind::Heading(2);
    }
    if ratio >= 1.14 {
        return Kind::Heading(3);
    }
    if ratio >= 1.05 && line.all_bold && line.char_count <= 100 {
        return Kind::Heading(3);
    }
    // Same-size bold standalone heading line above a non-bold body paragraph.
    if (ratio - 1.0).abs() <= 0.05 && line.all_bold && line.char_count <= 80 {
        let text = line.plain_text();
        let ends_sentence = text.ends_with(['.', '?', '!', ',', ';']);
        let followed_by_body = next_line.is_some_and(|nl| {
            !nl.all_bold && (line.y - nl.y) >= f64::from(line.dominant_size) * 1.15
        });
        if !ends_sentence && followed_by_body {
            return Kind::Heading(3);
        }
    }
    Kind::Normal
}

fn detect_list_prefix(line: &VisualLine, body_left: f64) -> Option<(ListInfo, usize)> {
    let text = line.plain_text();
    let trimmed = text.trim_start();
    let leading_ws = text.len() - trimmed.len();
    let level = (((line.x - body_left).max(0.0) / 24.0).floor() as u8).min(8);

    // Bullet markers.
    for bullet in ['\u{2022}', '\u{25E6}', '\u{25AA}', '\u{25AB}', '\u{2023}', '\u{00B7}', '-', '*'] {
        if let Some(after) = trimmed.strip_prefix(bullet)
            && let Some(stripped) = after.strip_prefix([' ', '\t', '\u{00A0}'])
        {
            let rest = stripped.trim_start_matches([' ', '\t']);
            if !rest.is_empty() {
                let prefix_bytes = text.len() - rest.len();
                return Some((
                    ListInfo {
                        ordered: false,
                        level,
                        start: 1,
                        list: 0,
                        label: None,
                    },
                    prefix_bytes,
                ));
            }
        }
    }

    // Ordered list markers: `1. `, `2) `, `(1) `.
    let _ = leading_ws;
    if let Some(inner) = trimmed.strip_prefix('(')
        && let Some((num_part, after_paren)) = inner.split_once(')')
        && (1..=4).contains(&num_part.len())
        && num_part.bytes().all(|b| b.is_ascii_digit())
        && after_paren.starts_with([' ', '\t'])
    {
        let rest = after_paren.trim_start_matches([' ', '\t']);
        if !rest.is_empty() {
            let start = num_part.parse::<u32>().unwrap_or(1).max(1);
            return Some((
                ListInfo { ordered: true, level, start, list: 0, label: None },
                text.len() - rest.len(),
            ));
        }
    }

    let num_len = trimmed.bytes().take_while(|b| b.is_ascii_digit()).count();
    if (1..=4).contains(&num_len) {
        let after_digits = &trimmed[num_len..];
        if let Some(after_punct) = after_digits.strip_prefix(['.', ')'])
            && after_punct.starts_with([' ', '\t'])
        {
            let rest = after_punct.trim_start_matches([' ', '\t']);
            if !rest.is_empty() {
                let start = trimmed[..num_len].parse::<u32>().unwrap_or(1).max(1);
                return Some((
                    ListInfo { ordered: true, level, start, list: 0, label: None },
                    text.len() - rest.len(),
                ));
            }
        }
    }

    None
}

fn reflow_page_lines(
    lines: &[VisualLine],
    body_size: f32,
    metrics: &PageMetrics,
    page_break_on_first: bool,
) -> Vec<FBlock> {
    let mut blocks: Vec<FBlock> = Vec::new();
    let mut active: Option<(Para, VisualLine)> = None;
    let mut pending_page_break = page_break_on_first;

    for (idx, line) in lines.iter().enumerate() {
        let next_hint = lines.get(idx + 1);
        let mut kind = classify_line_kind(line, next_hint, body_size);
        let list_hit = if kind == Kind::Normal {
            detect_list_prefix(line, metrics.body_left)
        } else {
            None
        };
        if list_hit.is_some() {
            kind = Kind::Normal;
        }

        if let Some((ref mut curr_para, ref mut prev_line)) = active {
            if list_hit.is_none() && should_merge_lines(curr_para, prev_line, line, kind, metrics) {
                append_line_to_para(curr_para, &line.segs, 0);
                *prev_line = line.clone();
                continue;
            }
            let (finished, _) = active.take().unwrap_or_else(|| (Para::new(Kind::Normal), line.clone()));
            if !finished.inlines.is_empty() {
                blocks.push(FBlock::Para(finished));
            }
        }

        let mut para = Para::new(kind);
        if pending_page_break {
            para.page_break = true;
            pending_page_break = false;
        }
        if let Some((list_info, skip_bytes)) = list_hit {
            para.list = Some(list_info);
            append_line_to_para(&mut para, &line.segs, skip_bytes);
        } else {
            if kind != Kind::Normal {
                para.keep_next = true;
            }
            if let Some(align) = detect_centered_line(line, metrics) {
                para.align = Some(align);
            }
            // Preserve first-line indent when clearly indented relative to body left margin.
            let indent = line.x - metrics.body_left;
            if kind == Kind::Normal
                && para.align.is_none()
                && indent >= f64::from(line.dominant_size) * 1.25
                && indent <= 72.0
            {
                para.spacing = Spacing {
                    first: Some((indent as f32).round()),
                    ..Default::default()
                };
            }
            append_line_to_para(&mut para, &line.segs, 0);
        }
        active = Some((para, line.clone()));
    }

    if let Some((finished, _)) = active
        && !finished.inlines.is_empty()
    {
        blocks.push(FBlock::Para(finished));
    }

    blocks
}

fn detect_centered_line(line: &VisualLine, metrics: &PageMetrics) -> Option<Align> {
    if metrics.page_width < 200.0 {
        return None;
    }
    let left_margin = line.x;
    let right_margin = metrics.page_width - line.end_x;
    let line_w = line.end_x - line.x;
    if left_margin >= metrics.body_left + 36.0
        && right_margin >= 36.0
        && line_w < (metrics.page_width - 2.0 * metrics.body_left) * 0.78
        && (left_margin - right_margin).abs() <= 18.0
    {
        Some(Align::Center)
    } else {
        None
    }
}

fn should_merge_lines(
    curr_para: &Para,
    prev_line: &VisualLine,
    next_line: &VisualLine,
    next_kind: Kind,
    metrics: &PageMetrics,
) -> bool {
    let dy = prev_line.y - next_line.y;
    let ref_size = f64::from(prev_line.dominant_size.max(next_line.dominant_size));
    if dy < ref_size * 0.55 {
        return false;
    }

    // Wrapped multi-line headings of identical heading level and font size:
    if curr_para.kind != Kind::Normal {
        return curr_para.kind == next_kind
            && (prev_line.dominant_size - next_line.dominant_size).abs() <= 0.6
            && dy <= ref_size * 1.38
            && prev_line.end_x >= metrics.col_right - ref_size * 4.0;
    }

    if next_kind != Kind::Normal {
        return false;
    }

    if curr_para.align == Some(Align::Center) {
        return false;
    }

    // Font size must stay compatible across wrapped lines of the same paragraph.
    if (prev_line.dominant_size - next_line.dominant_size).abs() > 1.2 {
        return false;
    }

    // Vertical baseline gap check: break on inter-paragraph spacing.
    let max_dy = match metrics.typical_leading {
        Some(lead) => (lead * 1.36).max(ref_size * 1.42),
        None => ref_size * 1.62,
    };
    if dy > max_dy {
        return false;
    }

    // Indentation check: a new first-line indent signals a new paragraph.
    let next_indent_from_body = next_line.x - metrics.body_left;
    let next_shift_from_prev = next_line.x - prev_line.x;
    if curr_para.list.is_none()
        && next_indent_from_body >= ref_size * 1.15
        && next_shift_from_prev >= ref_size * 1.0
    {
        return false;
    }

    // List continuation check: if inside a list item, continuation lines stay at or right of body_left.
    if curr_para.list.is_some() && next_line.x + 6.0 < metrics.body_left {
        return false;
    }

    // Short previous line check: if the column width is established and `prev_line` stopped well
    // before the right edge with sentence-ending punctuation, start a new paragraph.
    let col_span = metrics.col_right - metrics.body_left;
    if col_span >= 180.0 && prev_line.end_x < metrics.col_right - (ref_size * 5.0).max(48.0) {
        let prev_txt = prev_line.plain_text();
        let next_txt = next_line.plain_text();
        let prev_ends_terminal = prev_txt.ends_with(['.', '!', '?', ':', '\u{3002}', '\u{FF01}', '\u{FF1F}']);
        let next_starts_lower = next_txt.chars().next().is_some_and(|c| c.is_lowercase());
        if prev_ends_terminal && !next_starts_lower {
            return false;
        }
    }

    true
}

fn append_line_to_para(para: &mut Para, segs: &[RichSeg], mut skip_bytes: usize) {
    let mut sliced: Vec<(&str, &Fmt)> = Vec::with_capacity(segs.len());
    for seg in segs {
        if skip_bytes >= seg.text.len() {
            skip_bytes -= seg.text.len();
            continue;
        }
        let sub = &seg.text[skip_bytes..];
        skip_bytes = 0;
        if !sub.is_empty() {
            sliced.push((sub, &seg.fmt));
        }
    }
    let Some(&(first_str, first_fmt)) = sliced.first() else { return };

    if !para.inlines.is_empty() {
        // De-hyphenate wrapped ASCII words (`docu-` + `ment` -> `document`), else insert space
        // unless joining adjacent Chinese/Japanese characters.
        let dehyphenated = if let Some(Inline::Text(prev_text, _)) = para.inlines.last_mut()
            && prev_text.ends_with('-')
            && prev_text[..prev_text.len() - 1]
                .chars()
                .next_back()
                .is_some_and(|c| c.is_ascii_alphabetic())
            && first_str.chars().next().is_some_and(|c| c.is_ascii_lowercase())
        {
            prev_text.pop();
            true
        } else {
            false
        };

        if !dehyphenated {
            let prev_last_char = para_last_char(para);
            let next_first_char = first_str.chars().next();
            let needs_space = match (prev_last_char, next_first_char) {
                (Some(p), _) if p.is_whitespace() => false,
                (_, Some(n)) if n.is_whitespace() => false,
                (Some(p), Some(n)) if is_spaceless_cjk(p) && is_spaceless_cjk(n) => false,
                _ => true,
            };
            if needs_space {
                let mut join_fmt = match para.inlines.last() {
                    Some(Inline::Text(_, f)) => f.clone(),
                    _ => first_fmt.clone(),
                };
                if join_fmt.link != first_fmt.link {
                    join_fmt.link = None;
                }
                para.push_text(" ", &join_fmt);
            }
        }
    }

    for (text, fmt) in sliced {
        para.push_text(text, fmt);
    }
}

fn para_last_char(para: &Para) -> Option<char> {
    for inline in para.inlines.iter().rev() {
        if let Inline::Text(t, _) = inline
            && let Some(ch) = t.chars().next_back()
        {
            return Some(ch);
        }
    }
    None
}

/// Han ideographs and Japanese Kana join across line wraps without inserting an ASCII space
/// (Korean Hangul uses spaces between eojeols, so Hangul wraps keep spaces).
fn is_spaceless_cjk(c: char) -> bool {
    matches!(
        c as u32,
        0x3000..=0x30FF | 0x3400..=0x4DBF | 0x4E00..=0x9FFF | 0xF900..=0xFAFF | 0xFF01..=0xFF60
    )
}
