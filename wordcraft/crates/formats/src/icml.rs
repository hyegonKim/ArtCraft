//! Clean-room Adobe InCopy Markup Language (`.icml`, `AICY v22.0` / IDML snippet) reader and
//! writer for WordCraft.
//!
//! Maps styled paragraphs (`Heading1`–`Heading6`, `Title`, `Quote`, `Code`, `ListParagraph`,
//! custom styles, alignment, indents, spacing), character formatting (bold, italic, underline,
//! strikethrough, superscript/subscript, font family, point size, RGB/CMYK swatch colour,
//! hyperlinks), tables, and `<Change ChangeType="InsertedText"|"DeletedText"|"MovedText">`
//! tracked revisions directly to and from [`wordcraft_doc::Document`].

use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;

use quick_xml::Reader;
use quick_xml::events::{BytesStart, Event};
use wordcraft_doc::props::{CharProps, LineSpacing, ParaProps, TextColor, Underline};
use wordcraft_doc::styles::{Style, StyleKind};
use wordcraft_doc::{Align, Document, Rgb, SectionProps};

use crate::model::{
    self, Cell, Dropped, FBlock, FTable, Flow, FlowOpts, Fmt, Inline, Kind, ListInfo, MAX_FONT_NAME, Para, RevMark, Spacing, begin_import,
    clean_text, dropped, kept_font, kept_link,
};

/// Default InCopy 2027 DOM version written on export.
pub const DEFAULT_DOM_VERSION: &str = "22.0";

/// Maximum XML nesting depth accepted from untrusted `.icml` input.
const MAX_XML_DEPTH: usize = 64;
/// Maximum XML events processed in a single `.icml` file before stopping cleanly.
const MAX_XML_EVENTS: usize = 250_000;
/// Maximum `<Br/>` elements expanded in a single range.
const MAX_BR_PER_EVENT: usize = 4096;

/// Returns `true` when `bytes` begins like an InCopy `.icml` XML story.
pub fn is_icml(bytes: &[u8]) -> bool {
    let head_len = bytes.len().min(4096);
    let Some(head_bytes) = bytes.get(..head_len) else {
        return false;
    };
    let head = crate::txt::decode(head_bytes);
    let trimmed = head.trim_start_matches(['\u{FEFF}', ' ', '\t', '\r', '\n']);
    if !trimmed.starts_with('<') {
        return false;
    }
    (trimmed.contains("SnippetType=\"InCopyInterchange\"")
        || (trimmed.contains("<?aid") && trimmed.contains("<Document") && trimmed.contains("<Story"))
        || (trimmed.contains("<Document") && trimmed.contains("DOMVersion=") && trimmed.contains("<Story")))
        && !trimmed.contains("<InCopyAssignment")
}

/// Import `.icml` bytes onto the default page size.
pub fn import(bytes: &[u8]) -> Result<Document, String> {
    import_on(bytes, &SectionProps::default())
}

/// Import `.icml` bytes onto `page` (size and margins).
pub fn import_on(bytes: &[u8], page: &SectionProps) -> Result<Document, String> {
    begin_import();
    let xml = crate::txt::decode(bytes);
    let parsed = parse_icml(&xml)?;
    let mut doc = model::to_doc_in(&parsed.flow, page.clone());
    apply_imported_styles(&mut doc, &parsed.para_styles, &parsed.char_styles);
    if parsed.track_changes {
        doc.settings.track_changes = true;
    }
    for (author, date) in &parsed.moved_marks {
        for rev in &mut doc.revisions {
            if rev.author == *author && rev.date == *date && rev.move_name.is_none() {
                rev.move_name = Some("icml_move".to_string());
            }
        }
    }
    doc.ensure_nonempty();
    Ok(doc)
}

/// Export `doc` as an InCopy 2027 (`.icml`, `DOMVersion="22.0"`) XML string.
pub fn export(doc: &Document) -> String {
    export_with_version(doc, DEFAULT_DOM_VERSION)
}

/// Export `doc` as an InCopy `.icml` XML string with an explicit `DOMVersion`.
pub fn export_with_version(doc: &Document, dom_version: &str) -> String {
    let version = sanitize_attr_token(dom_version, DEFAULT_DOM_VERSION);
    let opts = FlowOpts { tracked: true, chars: true, paras: true, lists: true, ..Default::default() };
    let flow = model::from_doc_with(doc, opts);
    let swatches = collect_swatches(doc, &flow);
    let links = collect_links(&flow.blocks);
    let has_revisions = doc.settings.track_changes || !doc.revisions.is_empty() || flow_has_revisions(&flow.blocks);

    let mut out = String::with_capacity(4096);
    out.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n");
    out.push_str(&format!("<?aid style=\"50\" type=\"snippet\" readerVersion=\"6.0\" featureSet=\"257\" product=\"{version}(0)\" ?>\n"));
    out.push_str("<?aid SnippetType=\"InCopyInterchange\"?>\n");
    out.push_str(&format!("<Document DOMVersion=\"{}\" Self=\"d\">\n", xml_esc(&version)));

    write_color_defs(&mut out, &swatches);
    write_style_groups(&mut out, doc, &flow.blocks);
    write_hyperlink_destinations(&mut out, &links);

    out.push_str(&format!(
        "  <Story Self=\"u1\" AppliedTOCStyle=\"n\" UserText=\"true\" IsEndnoteStory=\"false\" TrackChanges=\"{}\">\n",
        if has_revisions { "true" } else { "false" }
    ));
    out.push_str("    <StoryPreference OpticalMarginAlignment=\"false\" OpticalMarginSize=\"12\" FrameType=\"TextFrameType\" StoryOrientation=\"Horizontal\" StoryDirection=\"LeftToRightDirection\" />\n");

    let mut ctx = ExportCtx { doc, links: &links, next_id: 1 };
    write_story_blocks(&mut out, &flow.blocks, 2, &mut ctx);
    if flow.blocks.is_empty() {
        out.push_str("    <ParagraphStyleRange AppliedParagraphStyle=\"ParagraphStyle/Normal\">\n");
        out.push_str("      <CharacterStyleRange AppliedCharacterStyle=\"CharacterStyle/$ID/[No character style]\">\n");
        out.push_str("        <Content></Content>\n");
        out.push_str("      </CharacterStyleRange>\n");
        out.push_str("    </ParagraphStyleRange>\n");
    }

    out.push_str("  </Story>\n");
    write_hyperlink_Bindings(&mut out, &links);
    out.push_str("</Document>\n");
    out
}

// ---------------------------------------------------------------------------
// Export helpers
// ---------------------------------------------------------------------------

struct ExportCtx<'a> {
    doc: &'a Document,
    links: &'a BTreeMap<String, usize>,
    next_id: usize,
}

impl ExportCtx<'_> {
    fn alloc_id(&mut self, prefix: &str) -> String {
        let id = self.next_id;
        self.next_id = self.next_id.saturating_add(1);
        format!("{prefix}{id}")
    }

    fn is_move_revision(&self, mark: &RevMark) -> bool {
        if let Some(id) = mark.id {
            return self.doc.move_name(Some(id)).is_some();
        }
        false
    }
}

fn sanitize_attr_token(raw: &str, fallback: &str) -> String {
    let clean: String = raw.trim().chars().filter(|c| !c.is_control() && *c != '"' && *c != '<' && *c != '>').take(32).collect();
    if clean.is_empty() { fallback.to_string() } else { clean }
}

fn xml_esc(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            c if c.is_control() && !matches!(c, '\t' | '\n' | '\r') => {}
            c => out.push(c),
        }
    }
    out
}

fn swatch_name(rgb: Rgb) -> String {
    if rgb == Rgb(0, 0, 0) {
        "Color/Black".to_string()
    } else if rgb == Rgb(255, 255, 255) {
        "Color/Paper".to_string()
    } else {
        format!("Color/WordCraft_{:02X}{:02X}{:02X}", rgb.0, rgb.1, rgb.2)
    }
}

fn collect_swatches(doc: &Document, flow: &Flow) -> BTreeMap<String, Rgb> {
    let mut map = BTreeMap::new();
    for st in &doc.styles.styles {
        if let Some(TextColor::Rgb(rgb)) = st.chr.color {
            let name = swatch_name(rgb);
            if name.starts_with("Color/WordCraft_") {
                map.insert(name, rgb);
            }
        }
    }
    collect_block_swatches(&flow.blocks, 0, &mut map);
    map
}

fn collect_block_swatches(blocks: &[FBlock], depth: usize, map: &mut BTreeMap<String, Rgb>) {
    if depth > model::MAX_DEPTH {
        return;
    }
    for b in blocks {
        match b {
            FBlock::Para(p) => {
                for inl in &p.inlines {
                    if let Inline::Text(_, fmt) = inl
                        && let Some(rgb) = fmt.color
                    {
                        let name = swatch_name(rgb);
                        if name.starts_with("Color/WordCraft_") {
                            map.insert(name, rgb);
                        }
                    }
                }
            }
            FBlock::Table(t) => {
                for cell in t.rows.iter().flatten() {
                    collect_block_swatches(&cell.blocks, depth + 1, map);
                }
            }
        }
    }
}

fn collect_links(blocks: &[FBlock]) -> BTreeMap<String, usize> {
    let mut map = BTreeMap::new();
    collect_block_links(blocks, 0, &mut map);
    map
}

fn collect_block_links(blocks: &[FBlock], depth: usize, map: &mut BTreeMap<String, usize>) {
    if depth > model::MAX_DEPTH {
        return;
    }
    for b in blocks {
        match b {
            FBlock::Para(p) => {
                for inl in &p.inlines {
                    if let Inline::Text(_, fmt) = inl
                        && let Some(url) = fmt.link.as_deref().filter(|u| !u.is_empty())
                        && !map.contains_key(url)
                    {
                        let next = map.len() + 1;
                        map.insert(url.to_string(), next);
                    }
                }
            }
            FBlock::Table(t) => {
                for cell in t.rows.iter().flatten() {
                    collect_block_links(&cell.blocks, depth + 1, map);
                }
            }
        }
    }
}

fn flow_has_revisions(blocks: &[FBlock]) -> bool {
    blocks.iter().any(|b| match b {
        FBlock::Para(p) => {
            p.mark_ins.is_some()
                || p.mark_del.is_some()
                || p.inlines.iter().any(|i| matches!(i, Inline::Text(_, f) if f.ins.is_some() || f.del.is_some()))
        }
        FBlock::Table(t) => t.rows.iter().flatten().any(|c| flow_has_revisions(&c.blocks)),
    })
}

fn write_color_defs(out: &mut String, swatches: &BTreeMap<String, Rgb>) {
    out.push_str("  <Color Self=\"Color/Black\" Model=\"Process\" Space=\"RGB\" ColorValue=\"0 0 0\" Name=\"Black\" />\n");
    out.push_str("  <Color Self=\"Color/Paper\" Model=\"Process\" Space=\"RGB\" ColorValue=\"255 255 255\" Name=\"Paper\" />\n");
    for (self_id, rgb) in swatches {
        let short = self_id.strip_prefix("Color/").unwrap_or(self_id);
        out.push_str(&format!(
            "  <Color Self=\"{}\" Model=\"Process\" Space=\"RGB\" ColorValue=\"{} {} {}\" Name=\"{}\" />\n",
            xml_esc(self_id),
            rgb.0,
            rgb.1,
            rgb.2,
            xml_esc(short)
        ));
    }
}

fn para_style_name(doc: &Document, p: &Para) -> String {
    if let Some(custom) = p.extra.as_ref().and_then(|x| x.style.as_deref()) {
        if let Some(st) = doc.styles.get(custom) {
            return st.name.clone();
        }
        return custom.to_string();
    }
    match p.kind {
        Kind::Heading(n) => format!("Heading {}", n.clamp(1, 6)),
        Kind::Title => "Title".to_string(),
        Kind::Quote => "Quote".to_string(),
        Kind::Code => "Code".to_string(),
        Kind::Rule => "Horizontal Rule".to_string(),
        Kind::Normal => {
            if p.list.is_some() {
                "List Paragraph".to_string()
            } else {
                "Normal".to_string()
            }
        }
    }
}

fn write_style_groups(out: &mut String, doc: &Document, blocks: &[FBlock]) {
    let mut para_names = BTreeMap::new();
    para_names.insert("Normal".to_string(), ("Normal".to_string(), None));
    collect_used_para_styles(doc, blocks, 0, &mut para_names);

    out.push_str("  <RootParagraphStyleGroup Self=\"u_pstyles\">\n");
    out.push_str("    <ParagraphStyle Self=\"ParagraphStyle/$ID/[No paragraph style]\" Name=\"$ID/[No paragraph style]\" />\n");
    for (name, (style_id, list_opt)) in &para_names {
        let esc_name = xml_esc(name);
        let mut attrs = format!("Self=\"ParagraphStyle/{esc_name}\" Name=\"{esc_name}\"");
        if let Some(st) = doc.styles.get(style_id) {
            append_para_style_attrs(&mut attrs, &st.para, &st.chr);
        }
        if let Some(li) = list_opt {
            let list_type = if li.ordered { "NumberedList" } else { "BulletList" };
            attrs.push_str(&format!(" BulletsAndNumberingListType=\"{list_type}\""));
        }
        if let Some(font) = doc.styles.get(style_id).and_then(|s| s.chr.font.as_deref()) {
            out.push_str(&format!("    <ParagraphStyle {attrs}>\n"));
            out.push_str(&format!("      <Properties><AppliedFont type=\"string\">{}</AppliedFont></Properties>\n", xml_esc(font)));
            out.push_str("    </ParagraphStyle>\n");
        } else {
            out.push_str(&format!("    <ParagraphStyle {attrs} />\n"));
        }
    }
    out.push_str("  </RootParagraphStyleGroup>\n");

    let mut char_names = BTreeMap::new();
    collect_used_char_styles(doc, blocks, 0, &mut char_names);
    out.push_str("  <RootCharacterStyleGroup Self=\"u_cstyles\">\n");
    out.push_str("    <CharacterStyle Self=\"CharacterStyle/$ID/[No character style]\" Name=\"$ID/[No character style]\" />\n");
    for (name, style_id) in &char_names {
        let esc_name = xml_esc(name);
        let mut attrs = format!("Self=\"CharacterStyle/{esc_name}\" Name=\"{esc_name}\"");
        if let Some(st) = doc.styles.get(style_id) {
            append_char_style_attrs(&mut attrs, &st.chr);
        }
        out.push_str(&format!("    <CharacterStyle {attrs} />\n"));
    }
    out.push_str("  </RootCharacterStyleGroup>\n");
}

fn collect_used_para_styles(doc: &Document, blocks: &[FBlock], depth: usize, out: &mut BTreeMap<String, (String, Option<ListInfo>)>) {
    if depth > model::MAX_DEPTH {
        return;
    }
    for b in blocks {
        match b {
            FBlock::Para(p) => {
                let name = para_style_name(doc, p);
                let style_id = p.extra.as_ref().and_then(|x| x.style.clone()).unwrap_or_else(|| name.replace(' ', ""));
                out.entry(name).or_insert((style_id, p.list));
            }
            FBlock::Table(t) => {
                for cell in t.rows.iter().flatten() {
                    collect_used_para_styles(doc, &cell.blocks, depth + 1, out);
                }
            }
        }
    }
}

fn collect_used_char_styles(doc: &Document, blocks: &[FBlock], depth: usize, out: &mut BTreeMap<String, String>) {
    if depth > model::MAX_DEPTH {
        return;
    }
    for b in blocks {
        match b {
            FBlock::Para(p) => {
                for inl in &p.inlines {
                    if let Inline::Text(_, fmt) = inl
                        && let Some(cid) = fmt.extra.as_ref().and_then(|x| x.style.as_deref())
                    {
                        let name = doc.styles.get(cid).map_or_else(|| cid.to_string(), |s| s.name.clone());
                        out.entry(name).or_insert_with(|| cid.to_string());
                    }
                }
            }
            FBlock::Table(t) => {
                for cell in t.rows.iter().flatten() {
                    collect_used_char_styles(doc, &cell.blocks, depth + 1, out);
                }
            }
        }
    }
}

fn append_para_style_attrs(attrs: &mut String, pp: &ParaProps, cp: &CharProps) {
    if let Some(a) = pp.align {
        attrs.push_str(&format!(" Justification=\"{}\"", align_to_icml(a)));
    }
    if let Some(v) = pp.indent_left {
        attrs.push_str(&format!(" LeftIndent=\"{v}\""));
    }
    if let Some(v) = pp.indent_right {
        attrs.push_str(&format!(" RightIndent=\"{v}\""));
    }
    if let Some(v) = pp.indent_first {
        attrs.push_str(&format!(" FirstLineIndent=\"{v}\""));
    }
    if let Some(v) = pp.space_before {
        attrs.push_str(&format!(" SpaceBefore=\"{v}\""));
    }
    if let Some(v) = pp.space_after {
        attrs.push_str(&format!(" SpaceAfter=\"{v}\""));
    }
    append_char_style_attrs(attrs, cp);
}

fn append_char_style_attrs(attrs: &mut String, cp: &CharProps) {
    let bold = cp.bold.unwrap_or(false);
    let italic = cp.italic.unwrap_or(false);
    if bold || italic {
        let fs = match (bold, italic) {
            (true, true) => "Bold Italic",
            (true, false) => "Bold",
            (false, true) => "Italic",
            (false, false) => "Regular",
        };
        attrs.push_str(&format!(" FontStyle=\"{fs}\""));
    }
    if let Some(sz) = cp.size {
        attrs.push_str(&format!(" PointSize=\"{sz}\""));
    }
    if cp.underline.is_some_and(|u| u != Underline::None) {
        attrs.push_str(" Underline=\"true\"");
    }
    if cp.strike.unwrap_or(false) || cp.double_strike.unwrap_or(false) {
        attrs.push_str(" StrikeThru=\"true\"");
    }
    if let Some(TextColor::Rgb(rgb)) = cp.color {
        attrs.push_str(&format!(" FillColor=\"{}\"", xml_esc(&swatch_name(rgb))));
    }
}

fn align_to_icml(a: Align) -> &'static str {
    match a {
        Align::Left => "LeftAlign",
        Align::Center => "CenterAlign",
        Align::Right => "RightAlign",
        Align::Justify => "LeftJustified",
        Align::Distribute => "FullyJustified",
    }
}

fn write_hyperlink_destinations(out: &mut String, links: &BTreeMap<String, usize>) {
    for (url, idx) in links {
        out.push_str(&format!(
            "  <HyperlinkURLDestination Self=\"HyperlinkURLDestination/d{idx}\" Name=\"d{idx}\" DestinationURL=\"{}\" DestinationUniqueKey=\"{idx}\" />\n",
            xml_esc(url)
        ));
    }
}

#[allow(non_snake_case)]
fn write_hyperlink_Bindings(out: &mut String, links: &BTreeMap<String, usize>) {
    for idx in links.values() {
        out.push_str(&format!(
            "  <Hyperlink Self=\"uf_h{idx}\" Name=\"Hyperlink {idx}\" Source=\"htss_{idx}\" Visible=\"false\" DestinationUniqueKey=\"{idx}\">\n"
        ));
        out.push_str("    <Properties>\n");
        out.push_str(&format!("      <Destination type=\"object\">HyperlinkURLDestination/d{idx}</Destination>\n"));
        out.push_str("    </Properties>\n");
        out.push_str("  </Hyperlink>\n");
    }
}

fn write_story_blocks(out: &mut String, blocks: &[FBlock], indent: usize, ctx: &mut ExportCtx<'_>) {
    let pad = "  ".repeat(indent);
    let total = blocks.len();
    for (idx, b) in blocks.iter().enumerate() {
        let is_last = idx + 1 == total;
        match b {
            FBlock::Para(p) => write_para(out, p, is_last, indent, ctx),
            FBlock::Table(t) => {
                out.push_str(&format!("{pad}<ParagraphStyleRange AppliedParagraphStyle=\"ParagraphStyle/Normal\">\n"));
                out.push_str(&format!("{pad}  <CharacterStyleRange AppliedCharacterStyle=\"CharacterStyle/$ID/[No character style]\">\n"));
                write_table(out, t, indent + 2, ctx);
                if !is_last {
                    out.push_str(&format!("{pad}    <Br />\n"));
                }
                out.push_str(&format!("{pad}  </CharacterStyleRange>\n"));
                out.push_str(&format!("{pad}</ParagraphStyleRange>\n"));
            }
        }
    }
}

fn write_table(out: &mut String, t: &FTable, indent: usize, ctx: &mut ExportCtx<'_>) {
    let pad = "  ".repeat(indent);
    let nrows = t.rows.len();
    let ncols = t.rows.iter().map(|r| r.iter().map(|c| c.colspan.max(1) as usize).sum::<usize>()).max().unwrap_or(0);
    if nrows == 0 || ncols == 0 {
        return;
    }
    let header_rows = t.rows.iter().take_while(|r| r.first().is_some_and(|c| c.header)).count();
    let body_rows = nrows.saturating_sub(header_rows);
    let tid = ctx.alloc_id("tbl_");
    out.push_str(&format!("{pad}<Table Self=\"{tid}\" HeaderRowCount=\"{header_rows}\" BodyRowCount=\"{body_rows}\" ColumnCount=\"{ncols}\">\n"));
    for (ri, row) in t.rows.iter().enumerate() {
        out.push_str(&format!("{pad}  <Row Self=\"{tid}_r{ri}\" Name=\"{ri}\" />\n"));
        let mut col_cursor = 0usize;
        for cell in row {
            let cspan = cell.colspan.max(1) as usize;
            if !cell.covered {
                let rspan = cell.rowspan.max(1);
                out.push_str(&format!(
                    "{pad}  <Cell Self=\"{tid}_c{col_cursor}_{ri}\" Name=\"{col_cursor}:{ri}\" RowSpan=\"{rspan}\" ColumnSpan=\"{cspan}\">\n"
                ));
                write_story_blocks(out, &cell.blocks, indent + 2, ctx);
                out.push_str(&format!("{pad}  </Cell>\n"));
            }
            col_cursor = col_cursor.saturating_add(cspan);
        }
    }
    out.push_str(&format!("{pad}</Table>\n"));
}

fn write_para(out: &mut String, p: &Para, is_last: bool, indent: usize, ctx: &mut ExportCtx<'_>) {
    let pad = "  ".repeat(indent);
    let style_name = para_style_name(ctx.doc, p);
    let mut pattrs = format!("AppliedParagraphStyle=\"ParagraphStyle/{}\"", xml_esc(&style_name));
    if let Some(a) = p.align {
        pattrs.push_str(&format!(" Justification=\"{}\"", align_to_icml(a)));
    }
    if let Some(v) = p.spacing.left {
        pattrs.push_str(&format!(" LeftIndent=\"{v}\""));
    }
    if let Some(v) = p.spacing.right {
        pattrs.push_str(&format!(" RightIndent=\"{v}\""));
    }
    if let Some(v) = p.spacing.first {
        pattrs.push_str(&format!(" FirstLineIndent=\"{v}\""));
    }
    if let Some(v) = p.spacing.before {
        pattrs.push_str(&format!(" SpaceBefore=\"{v}\""));
    }
    if let Some(v) = p.spacing.after {
        pattrs.push_str(&format!(" SpaceAfter=\"{v}\""));
    }
    if let Some(li) = p.list {
        let list_type = if li.ordered { "NumberedList" } else { "BulletList" };
        pattrs.push_str(&format!(" BulletsAndNumberingListType=\"{list_type}\""));
        if li.level > 0 {
            pattrs.push_str(&format!(" ListLevel=\"{}\"", li.level + 1));
        }
    }
    if p.page_break {
        pattrs.push_str(" StartParagraph=\"NextPage\"");
    }

    out.push_str(&format!("{pad}<ParagraphStyleRange {pattrs}>\n"));
    let mut wrote_any = false;
    for inl in &p.inlines {
        match inl {
            Inline::Text(text, fmt) => {
                write_text_span(out, text, fmt, indent + 1, ctx);
                wrote_any = true;
            }
            Inline::Equation { linear, .. } => {
                write_text_span(out, linear, &Fmt::default(), indent + 1, ctx);
                wrote_any = true;
            }
            Inline::Ruby { base, .. } => {
                write_text_span(out, base, &Fmt::default(), indent + 1, ctx);
                wrote_any = true;
            }
            Inline::Field { instr, result, fmt, .. } if !model::shows_nothing(instr) => {
                write_text_span(out, result, fmt, indent + 1, ctx);
                wrote_any = true;
            }
            Inline::Control { text, fmt, .. } => {
                write_text_span(out, text, fmt, indent + 1, ctx);
                wrote_any = true;
            }
            _ => {}
        }
    }
    if !wrote_any {
        out.push_str(&format!("{pad}  <CharacterStyleRange AppliedCharacterStyle=\"CharacterStyle/$ID/[No character style]\">\n"));
        out.push_str(&format!("{pad}    <Content></Content>\n"));
        if !is_last {
            out.push_str(&format!("{pad}    <Br />\n"));
        }
        out.push_str(&format!("{pad}  </CharacterStyleRange>\n"));
    } else if !is_last {
        out.push_str(&format!("{pad}  <CharacterStyleRange AppliedCharacterStyle=\"CharacterStyle/$ID/[No character style]\">\n"));
        out.push_str(&format!("{pad}    <Br />\n"));
        out.push_str(&format!("{pad}  </CharacterStyleRange>\n"));
    }
    out.push_str(&format!("{pad}</ParagraphStyleRange>\n"));
}

fn write_text_span(out: &mut String, text: &str, fmt: &Fmt, indent: usize, ctx: &mut ExportCtx<'_>) {
    let pad = "  ".repeat(indent);
    let change = fmt.del.as_ref().map(|m| ("DeletedText", m)).or_else(|| {
        fmt.ins.as_ref().map(|m| {
            let kind = if ctx.is_move_revision(m) { "MovedText" } else { "InsertedText" };
            (kind, m)
        })
    });

    if let Some((change_type, mark)) = change {
        let cid = ctx.alloc_id("chg_");
        let user = if mark.author.is_empty() { "$ID/InCopy User" } else { mark.author.as_str() };
        let mut cattrs = format!("Self=\"{cid}\" ChangeType=\"{change_type}\" UserName=\"{}\"", xml_esc(user));
        if !mark.date.is_empty() {
            cattrs.push_str(&format!(" Date=\"{}\"", xml_esc(&mark.date)));
        }
        out.push_str(&format!("{pad}<Change {cattrs}>\n"));
        write_char_range(out, text, fmt, indent + 1, ctx);
        out.push_str(&format!("{pad}</Change>\n"));
    } else {
        write_char_range(out, text, fmt, indent, ctx);
    }
}

fn write_char_range(out: &mut String, text: &str, fmt: &Fmt, indent: usize, ctx: &mut ExportCtx<'_>) {
    let pad = "  ".repeat(indent);
    let cstyle = fmt
        .extra
        .as_ref()
        .and_then(|x| x.style.as_deref())
        .and_then(|cid| ctx.doc.styles.get(cid).map(|s| s.name.clone()).or_else(|| Some(cid.to_string())))
        .unwrap_or_else(|| "$ID/[No character style]".to_string());

    let mut cattrs = format!("AppliedCharacterStyle=\"CharacterStyle/{}\"", xml_esc(&cstyle));
    let bold = fmt.bold;
    let italic = fmt.italic;
    if bold || italic {
        let fs = match (bold, italic) {
            (true, true) => "Bold Italic",
            (true, false) => "Bold",
            (false, true) => "Italic",
            (false, false) => "Regular",
        };
        cattrs.push_str(&format!(" FontStyle=\"{fs}\""));
    }
    if fmt.underline {
        cattrs.push_str(" Underline=\"true\"");
    }
    if fmt.strike {
        cattrs.push_str(" StrikeThru=\"true\"");
    }
    if fmt.sup {
        cattrs.push_str(" Position=\"Superscript\"");
    } else if fmt.sub {
        cattrs.push_str(" Position=\"Subscript\"");
    }
    if let Some(sz) = fmt.size {
        cattrs.push_str(&format!(" PointSize=\"{sz}\""));
    }
    if let Some(rgb) = fmt.color {
        cattrs.push_str(&format!(" FillColor=\"{}\"", xml_esc(&swatch_name(rgb))));
    }

    out.push_str(&format!("{pad}<CharacterStyleRange {cattrs}>\n"));
    let font_family = if fmt.code { Some(model::MONO_FONT) } else { fmt.font.as_deref() };
    if let Some(font) = font_family {
        out.push_str(&format!("{pad}  <Properties><AppliedFont type=\"string\">{}</AppliedFont></Properties>\n", xml_esc(font)));
    }

    let link_idx = fmt.link.as_deref().and_then(|u| ctx.links.get(u).copied());
    if let Some(idx) = link_idx {
        out.push_str(&format!("{pad}  <HyperlinkTextSource Self=\"htss_{idx}\" Name=\"Hyperlink {idx}\" Hidden=\"false\">\n"));
        write_content_lines(out, text, indent + 2);
        out.push_str(&format!("{pad}  </HyperlinkTextSource>\n"));
    } else {
        write_content_lines(out, text, indent + 1);
    }
    out.push_str(&format!("{pad}</CharacterStyleRange>\n"));
}

fn write_content_lines(out: &mut String, text: &str, indent: usize) {
    let pad = "  ".repeat(indent);
    let parts: Vec<&str> = text.split('\n').collect();
    for (i, seg) in parts.iter().enumerate() {
        out.push_str(&format!("{pad}<Content>{}</Content>\n", xml_esc(seg)));
        if i + 1 < parts.len() {
            out.push_str(&format!("{pad}<Br />\n"));
        }
    }
}

// ---------------------------------------------------------------------------
// Import parser
// ---------------------------------------------------------------------------

struct ParsedIcml {
    flow: Flow,
    para_styles: Vec<ImportedParaStyle>,
    char_styles: Vec<ImportedCharStyle>,
    track_changes: bool,
    moved_marks: Vec<(String, String)>,
}

#[derive(Clone, Debug, Default)]
struct ImportedParaStyle {
    name: String,
    align: Option<Align>,
    spacing: Spacing,
    fmt: Fmt,
    list: Option<ListInfo>,
}

#[derive(Clone, Debug, Default)]
struct ImportedCharStyle {
    name: String,
    fmt: Fmt,
}

#[derive(Clone, Debug)]
struct ActiveChange {
    kind: ChangeKind,
    author: String,
    date: String,
    id: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ChangeKind {
    Insert,
    Delete,
    Move,
}

#[derive(Default)]
struct TableBuild {
    header_rows: usize,
    cells: Vec<CellBuild>,
    cur_cell: Option<CellBuild>,
}

struct CellBuild {
    col: usize,
    row: usize,
    colspan: u32,
    rowspan: u32,
    blocks: Vec<FBlock>,
    cur_para: Option<Para>,
    para_template: Para,
    ended_with_br: bool,
}

#[derive(Default)]
struct ParseState {
    swatches: HashMap<String, Rgb>,
    para_defs: HashMap<String, ImportedParaStyle>,
    char_defs: HashMap<String, ImportedCharStyle>,
    para_list: Vec<ImportedParaStyle>,
    char_list: Vec<ImportedCharStyle>,
    dest_urls: HashMap<String, String>,
    source_to_dest: HashMap<String, String>,
    blocks: Vec<FBlock>,
    cur_para: Option<Para>,
    para_template: Para,
    ended_with_br: bool,
    in_story: bool,
    in_para_range: bool,
    in_content: bool,
    in_applied_font: bool,
    style_target: StyleTarget,
    char_stack: Vec<Fmt>,
    change_stack: Vec<ActiveChange>,
    next_change_id: u32,
    active_link_source: Option<String>,
    table_stack: Vec<TableBuild>,
    track_changes: bool,
    moved_marks: Vec<(String, String)>,
}

#[derive(Clone, Copy, Default, PartialEq, Eq)]
enum StyleTarget {
    #[default]
    None,
    ParaStyle(usize),
    CharStyle(usize),
    CharRange,
    HyperlinkBinding,
}

fn parse_icml(xml: &str) -> Result<ParsedIcml, String> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(false);

    let mut st = ParseState::default();
    st.swatches.insert("Color/Black".to_string(), Rgb(0, 0, 0));
    st.swatches.insert("Color/Paper".to_string(), Rgb(255, 255, 255));

    let mut saw_root = false;
    let mut depth = 0usize;
    let mut events = 0usize;
    let mut pending_hyperlink_source = String::new();
    let mut in_destination_prop = false;
    let mut text_buf = String::new();

    loop {
        events = events.saturating_add(1);
        if events > MAX_XML_EVENTS {
            break;
        }
        let ev = reader.read_event().map_err(|e| format!("ICML XML error: {e}"))?;
        match ev {
            Event::Start(ref e) => {
                depth = depth.saturating_add(1);
                if depth > MAX_XML_DEPTH {
                    return Err("ICML nesting exceeds maximum depth".to_string());
                }
                let local = local_tag_name(e.name().as_ref());
                if matches!(local.as_str(), "Document" | "Story") {
                    saw_root = true;
                }
                if local == "Hyperlink" {
                    pending_hyperlink_source = attr_val(e, "Source").unwrap_or_default();
                    st.style_target = StyleTarget::HyperlinkBinding;
                } else if local == "Destination" && st.style_target == StyleTarget::HyperlinkBinding {
                    in_destination_prop = true;
                    text_buf.clear();
                } else {
                    handle_open_tag(&mut st, &local, e, false);
                }
            }
            Event::Empty(ref e) => {
                let local = local_tag_name(e.name().as_ref());
                if matches!(local.as_str(), "Document" | "Story") {
                    saw_root = true;
                }
                handle_open_tag(&mut st, &local, e, true);
            }
            Event::Text(ref t) => {
                let decoded = decode_xml_text(t.as_ref());
                if in_destination_prop {
                    text_buf.push_str(&decoded);
                } else {
                    handle_text(&mut st, &decoded);
                }
            }
            Event::CData(ref c) => {
                let raw = String::from_utf8_lossy(c.as_ref());
                if in_destination_prop {
                    text_buf.push_str(&raw);
                } else {
                    handle_text(&mut st, &raw);
                }
            }
            Event::GeneralRef(ref r) => {
                let resolved = resolve_entity_ref(r.as_ref());
                if !resolved.is_empty() {
                    if in_destination_prop {
                        text_buf.push_str(&resolved);
                    } else {
                        handle_text(&mut st, &resolved);
                    }
                }
            }
            Event::End(ref e) => {
                depth = depth.saturating_sub(1);
                let local = local_tag_name(e.name().as_ref());
                if local == "Destination" && in_destination_prop {
                    in_destination_prop = false;
                    let dest = text_buf.trim().to_string();
                    if !pending_hyperlink_source.is_empty() && !dest.is_empty() {
                        st.source_to_dest.insert(pending_hyperlink_source.clone(), dest);
                    }
                    text_buf.clear();
                } else if local == "Hyperlink" {
                    pending_hyperlink_source.clear();
                    st.style_target = StyleTarget::None;
                } else {
                    handle_close_tag(&mut st, &local);
                }
            }
            Event::Eof => break,
            _ => {}
        }
    }

    if !saw_root {
        return Err("not an InCopy (.icml) document".to_string());
    }

    flush_open_para(&mut st);
    resolve_deferred_links(&mut st.blocks, &st.source_to_dest, &st.dest_urls);

    Ok(ParsedIcml {
        flow: Flow { blocks: st.blocks, ..Default::default() },
        para_styles: st.para_list,
        char_styles: st.char_list,
        track_changes: st.track_changes,
        moved_marks: st.moved_marks,
    })
}

fn local_tag_name(raw: &[u8]) -> String {
    let full = String::from_utf8_lossy(raw);
    match full.rsplit_once(':') {
        Some((_, local)) => local.to_string(),
        None => full.into_owned(),
    }
}

fn decode_xml_text(raw: &[u8]) -> String {
    String::from_utf8_lossy(raw).into_owned()
}

fn resolve_entity_ref(raw: &[u8]) -> String {
    let s = String::from_utf8_lossy(raw);
    match s.as_ref() {
        "amp" => "&".to_string(),
        "lt" => "<".to_string(),
        "gt" => ">".to_string(),
        "quot" => "\"".to_string(),
        "apos" => "'".to_string(),
        _ => {
            if let Some(hex) = s.strip_prefix("#x").or_else(|| s.strip_prefix("#X")) {
                u32::from_str_radix(hex, 16).ok().and_then(char::from_u32).map_or_else(String::new, |c| c.to_string())
            } else if let Some(dec) = s.strip_prefix('#') {
                dec.parse::<u32>().ok().and_then(char::from_u32).map_or_else(String::new, |c| c.to_string())
            } else {
                String::new()
            }
        }
    }
}

fn attr_val(e: &BytesStart<'_>, key: &str) -> Option<String> {
    for a in e.attributes().flatten() {
        if a.key.as_ref() == key.as_bytes() {
            let raw = String::from_utf8_lossy(&a.value);
            let val = match quick_xml::escape::unescape(&raw) {
                Ok(u) => u.into_owned(),
                Err(_) => raw.to_string(),
            };
            return Some(val);
        }
    }
    None
}

fn handle_open_tag(st: &mut ParseState, local: &str, e: &BytesStart<'_>, is_empty: bool) {
    match local {
        "Color" => parse_color_tag(st, e),
        "ParagraphStyle" => parse_para_style_tag(st, e, is_empty),
        "CharacterStyle" => parse_char_style_tag(st, e, is_empty),
        "HyperlinkURLDestination" => {
            if let (Some(self_id), Some(url)) = (attr_val(e, "Self"), attr_val(e, "DestinationURL")) {
                st.dest_urls.insert(self_id, url);
            }
        }
        "Story" => {
            if attr_val(e, "TrackChanges").as_deref() == Some("true") {
                st.track_changes = true;
            }
            if !is_empty {
                st.in_story = true;
            }
        }
        "ParagraphStyleRange" if st.in_story => {
            open_para_range(st, e);
            if is_empty {
                close_para_range(st);
            }
        }
        "CharacterStyleRange" if st.in_story => {
            let fmt = build_char_fmt(st, e);
            st.char_stack.push(fmt);
            if !is_empty {
                st.style_target = StyleTarget::CharRange;
            } else {
                st.char_stack.pop();
            }
        }
        "AppliedFont" => {
            if !is_empty {
                st.in_applied_font = true;
            }
        }
        "Change" if st.in_story => {
            st.track_changes = true;
            let ctype = match attr_val(e, "ChangeType").as_deref() {
                Some("DeletedText") => ChangeKind::Delete,
                Some("MovedText") => ChangeKind::Move,
                _ => ChangeKind::Insert,
            };
            let author = clean_author(&attr_val(e, "UserName").unwrap_or_default());
            let date = clean_text(&attr_val(e, "Date").unwrap_or_default());
            if ctype == ChangeKind::Move {
                st.moved_marks.push((author.clone(), date.clone()));
            }
            if !is_empty {
                let id = st.next_change_id;
                st.next_change_id = st.next_change_id.saturating_add(1);
                st.change_stack.push(ActiveChange { kind: ctype, author, date, id });
            }
        }
        "HyperlinkTextSource" if st.in_story => {
            if !is_empty {
                st.active_link_source = attr_val(e, "Self");
            }
        }
        "Table" if st.in_story && st.table_stack.len() < model::MAX_DEPTH => {
            if !is_empty {
                let header_rows = attr_val(e, "HeaderRowCount").and_then(|v| v.parse::<usize>().ok()).unwrap_or(0);
                st.table_stack.push(TableBuild { header_rows, cells: Vec::new(), cur_cell: None });
            }
        }
        "Cell" if !st.table_stack.is_empty() => {
            if !is_empty {
                let (col, row) = parse_cell_coords(attr_val(e, "Name").as_deref().unwrap_or("0:0"));
                let colspan = attr_val(e, "ColumnSpan").and_then(|v| v.parse::<u32>().ok()).unwrap_or(1).clamp(1, 64);
                let rowspan = attr_val(e, "RowSpan").and_then(|v| v.parse::<u32>().ok()).unwrap_or(1).clamp(1, 1000);
                if let Some(tbl) = st.table_stack.last_mut() {
                    tbl.cur_cell = Some(CellBuild {
                        col,
                        row,
                        colspan,
                        rowspan,
                        blocks: Vec::new(),
                        cur_para: None,
                        para_template: Para::default(),
                        ended_with_br: false,
                    });
                }
            }
        }
        "Content" if st.in_story => {
            if !is_empty {
                st.in_content = true;
            }
        }
        "Br" if st.in_story => {
            let count = attr_val(e, "Count").and_then(|s| s.parse::<usize>().ok()).unwrap_or(1).clamp(1, MAX_BR_PER_EVENT);
            for _ in 0..count {
                emit_paragraph_break(st);
            }
        }
        _ => {}
    }
}

fn handle_close_tag(st: &mut ParseState, local: &str) {
    match local {
        "ParagraphStyle" | "CharacterStyle" => {
            st.style_target = StyleTarget::None;
        }
        "AppliedFont" => {
            st.in_applied_font = false;
        }
        "Content" => {
            st.in_content = false;
        }
        "HyperlinkTextSource" => {
            st.active_link_source = None;
        }
        "Change" => {
            st.change_stack.pop();
        }
        "CharacterStyleRange" => {
            st.char_stack.pop();
            if st.style_target == StyleTarget::CharRange {
                st.style_target = StyleTarget::None;
            }
        }
        "ParagraphStyleRange" => {
            close_para_range(st);
        }
        "Cell" => {
            if let Some(tbl) = st.table_stack.last_mut()
                && let Some(mut cell) = tbl.cur_cell.take()
            {
                if let Some(p) = cell.cur_para.take()
                    && (!cell.ended_with_br || !p.inlines.is_empty())
                {
                    cell.blocks.push(FBlock::Para(p));
                }
                if cell.blocks.is_empty() {
                    cell.blocks.push(FBlock::Para(Para::default()));
                }
                tbl.cells.push(cell);
            }
        }
        "Table" => {
            if let Some(tbl) = st.table_stack.pop()
                && let Some(ft) = finalize_table(tbl)
            {
                push_completed_block(st, FBlock::Table(ft));
            }
        }
        "Story" => {
            flush_open_para(st);
            st.in_story = false;
        }
        _ => {}
    }
}

fn handle_text(st: &mut ParseState, text: &str) {
    if st.in_applied_font {
        let trimmed = text.trim();
        if !trimmed.is_empty() {
            let kept = kept_font(trimmed);
            match st.style_target {
                StyleTarget::ParaStyle(idx) => {
                    if let Some(ps) = st.para_list.get_mut(idx) {
                        ps.fmt.font = kept.clone();
                        st.para_defs.insert(ps.name.clone(), ps.clone());
                    }
                }
                StyleTarget::CharStyle(idx) => {
                    if let Some(cs) = st.char_list.get_mut(idx) {
                        cs.fmt.font = kept.clone();
                        st.char_defs.insert(cs.name.clone(), cs.clone());
                    }
                }
                StyleTarget::CharRange => {
                    if let Some(top) = st.char_stack.last_mut() {
                        top.font = kept;
                    }
                }
                StyleTarget::None | StyleTarget::HyperlinkBinding => {}
            }
        }
        return;
    }

    if !st.in_content || !st.in_story || text.is_empty() {
        return;
    }

    // U+2028 in InCopy Content represents a forced line break; U+FEFF is a zero-width marker.
    let cleaned: String = text.chars().filter(|&c| c != '\u{FEFF}').map(|c| if c == '\u{2028}' { '\n' } else { c }).collect();
    if cleaned.is_empty() {
        return;
    }

    let fmt = effective_span_fmt(st);
    append_text_to_current_para(st, &cleaned, &fmt);
}

fn clean_author(raw: &str) -> String {
    let stripped = raw.strip_prefix("$ID/").unwrap_or(raw).trim();
    if stripped.is_empty() { "InCopy Editor".to_string() } else { clean_text(stripped) }
}

fn parse_color_tag(st: &mut ParseState, e: &BytesStart<'_>) {
    let Some(self_id) = attr_val(e, "Self") else { return };
    let space = attr_val(e, "Space").unwrap_or_default();
    let vals: Vec<f32> = attr_val(e, "ColorValue").unwrap_or_default().split_whitespace().filter_map(|s| s.parse::<f32>().ok()).collect();
    let rgb = if space.eq_ignore_ascii_case("CMYK") && vals.len() >= 4 {
        let c = (vals[0] / 100.0).clamp(0.0, 1.0);
        let m = (vals[1] / 100.0).clamp(0.0, 1.0);
        let y = (vals[2] / 100.0).clamp(0.0, 1.0);
        let k = (vals[3] / 100.0).clamp(0.0, 1.0);
        Rgb(
            ((1.0 - c) * (1.0 - k) * 255.0).round() as u8,
            ((1.0 - m) * (1.0 - k) * 255.0).round() as u8,
            ((1.0 - y) * (1.0 - k) * 255.0).round() as u8,
        )
    } else if vals.len() >= 3 {
        Rgb(vals[0].round().clamp(0.0, 255.0) as u8, vals[1].round().clamp(0.0, 255.0) as u8, vals[2].round().clamp(0.0, 255.0) as u8)
    } else {
        return;
    };
    st.swatches.insert(self_id, rgb);
}

fn normalize_style_ref(raw: &str, prefix: &str) -> String {
    let s = raw.strip_prefix(prefix).unwrap_or(raw);
    let s = s.strip_prefix("$ID/").unwrap_or(s);
    if s.len() > MAX_FONT_NAME {
        dropped(Dropped::StyleName);
        let end = (0..=MAX_FONT_NAME).rev().find(|&i| s.is_char_boundary(i)).unwrap_or(0);
        s.get(..end).unwrap_or("").to_string()
    } else {
        s.to_string()
    }
}

fn parse_para_style_tag(st: &mut ParseState, e: &BytesStart<'_>, is_empty: bool) {
    let raw_name = attr_val(e, "Name").or_else(|| attr_val(e, "Self")).unwrap_or_default();
    let name = normalize_style_ref(&raw_name, "ParagraphStyle/");
    if name.is_empty() || name.starts_with('[') {
        return;
    }
    let mut ps = ImportedParaStyle {
        name: name.clone(),
        align: attr_val(e, "Justification").as_deref().and_then(icml_to_align),
        spacing: parse_spacing_attrs(e),
        fmt: parse_inline_fmt_attrs(&st.swatches, e, Fmt::default()),
        list: parse_list_attrs(e),
    };
    if let Some(font) = attr_val(e, "AppliedFont") {
        ps.fmt.font = kept_font(&font);
    }
    let idx = st.para_list.len();
    st.para_defs.insert(name, ps.clone());
    st.para_list.push(ps);
    if !is_empty {
        st.style_target = StyleTarget::ParaStyle(idx);
    }
}

fn parse_char_style_tag(st: &mut ParseState, e: &BytesStart<'_>, is_empty: bool) {
    let raw_name = attr_val(e, "Name").or_else(|| attr_val(e, "Self")).unwrap_or_default();
    let name = normalize_style_ref(&raw_name, "CharacterStyle/");
    if name.is_empty() || name.starts_with('[') {
        return;
    }
    let mut cs = ImportedCharStyle { name: name.clone(), fmt: parse_inline_fmt_attrs(&st.swatches, e, Fmt::default()) };
    if let Some(font) = attr_val(e, "AppliedFont") {
        cs.fmt.font = kept_font(&font);
    }
    let idx = st.char_list.len();
    st.char_defs.insert(name, cs.clone());
    st.char_list.push(cs);
    if !is_empty {
        st.style_target = StyleTarget::CharStyle(idx);
    }
}

fn icml_to_align(val: &str) -> Option<Align> {
    match val {
        "LeftAlign" | "ToBindingSide" => Some(Align::Left),
        "CenterAlign" | "CenterJustified" => Some(Align::Center),
        "RightAlign" | "AwayFromBindingSide" => Some(Align::Right),
        "LeftJustified" | "RightJustified" => Some(Align::Justify),
        "FullyJustified" => Some(Align::Distribute),
        _ => None,
    }
}

fn parse_f32_attr(e: &BytesStart<'_>, key: &str) -> Option<f32> {
    attr_val(e, key).and_then(|s| s.parse::<f32>().ok()).filter(|v| v.is_finite())
}

fn parse_spacing_attrs(e: &BytesStart<'_>) -> Spacing {
    let leading = parse_f32_attr(e, "Leading").filter(|&v| v > 0.0).map(LineSpacing::AtLeast);
    Spacing {
        left: parse_f32_attr(e, "LeftIndent"),
        right: parse_f32_attr(e, "RightIndent"),
        first: parse_f32_attr(e, "FirstLineIndent"),
        before: parse_f32_attr(e, "SpaceBefore"),
        after: parse_f32_attr(e, "SpaceAfter"),
        line: leading,
        before_auto: false,
        after_auto: false,
    }
}

fn parse_list_attrs(e: &BytesStart<'_>) -> Option<ListInfo> {
    match attr_val(e, "BulletsAndNumberingListType").as_deref() {
        Some("BulletList") => {
            let lvl = attr_val(e, "ListLevel").and_then(|s| s.parse::<u8>().ok()).unwrap_or(1).saturating_sub(1).min(8);
            Some(ListInfo { ordered: false, level: lvl, start: 1, ..Default::default() })
        }
        Some("NumberedList") => {
            let lvl = attr_val(e, "ListLevel").and_then(|s| s.parse::<u8>().ok()).unwrap_or(1).saturating_sub(1).min(8);
            Some(ListInfo { ordered: true, level: lvl, start: 1, ..Default::default() })
        }
        _ => None,
    }
}

fn parse_inline_fmt_attrs(swatches: &HashMap<String, Rgb>, e: &BytesStart<'_>, mut base: Fmt) -> Fmt {
    if let Some(fs) = attr_val(e, "FontStyle") {
        let lower = fs.to_ascii_lowercase();
        if lower.contains("bold") || lower.contains("semibold") || lower.contains("black") || lower.contains("heavy") {
            base.bold = true;
        }
        if lower.contains("italic") || lower.contains("oblique") {
            base.italic = true;
        }
        if lower == "regular" || lower == "roman" {
            base.bold = false;
            base.italic = false;
        }
    }
    if let Some(u) = attr_val(e, "Underline") {
        base.underline = u.eq_ignore_ascii_case("true");
    }
    if let Some(s) = attr_val(e, "StrikeThru") {
        base.strike = s.eq_ignore_ascii_case("true");
    }
    if let Some(pos) = attr_val(e, "Position") {
        match pos.as_str() {
            "Superscript" | "OTSuperscript" => {
                base.sup = true;
                base.sub = false;
            }
            "Subscript" | "OTSubscript" => {
                base.sub = true;
                base.sup = false;
            }
            "Normal" => {
                base.sup = false;
                base.sub = false;
            }
            _ => {}
        }
    }
    if let Some(sz) = parse_f32_attr(e, "PointSize").filter(|&v| (1.0..=1638.0).contains(&v)) {
        base.size = Some(sz);
    }
    if let Some(fill) = attr_val(e, "FillColor") {
        if let Some(&rgb) = swatches.get(&fill) {
            base.color = Some(rgb);
        } else if let Some(hex) = fill.strip_prefix("Color/WordCraft_")
            && let Some(rgb) = Rgb::parse(hex)
        {
            base.color = Some(rgb);
        }
    }
    base
}

fn classify_style_name(name: &str) -> (Kind, Option<String>) {
    let lower = name.trim().to_ascii_lowercase();
    let compact = lower.replace([' ', '-', '_'], "");
    if let Some(rest) = compact.strip_prefix("heading").or_else(|| compact.strip_prefix("h"))
        && rest.len() == 1
        && let Some(d) = rest.chars().next().and_then(|c| c.to_digit(10))
        && (1..=6).contains(&d)
    {
        return (Kind::Heading(d as u8), None);
    }
    match compact.as_str() {
        "normal" | "normalparagraphstyle" | "basicparagraph" | "[basicparagraph]" | "[noparagraphstyle]" | "body" | "bodytext" => {
            (Kind::Normal, None)
        }
        "title" => (Kind::Title, None),
        "quote" | "blockquote" | "intensequote" | "quotations" => (Kind::Quote, None),
        "code" | "codeblock" | "preformatted" => (Kind::Code, None),
        "horizontalrule" => (Kind::Rule, None),
        "listparagraph" => (Kind::Normal, None),
        _ => {
            let id: String = name.chars().filter(|c| c.is_alphanumeric() || *c == '_').take(64).collect();
            let custom = (!id.is_empty()).then_some(id);
            (Kind::Normal, custom)
        }
    }
}

fn build_para_template(st: &ParseState, e: &BytesStart<'_>) -> Para {
    let raw_style = attr_val(e, "AppliedParagraphStyle").unwrap_or_default();
    let style_name = normalize_style_ref(&raw_style, "ParagraphStyle/");
    let (kind, custom_id) = classify_style_name(&style_name);

    let def = st.para_defs.get(&style_name);
    let mut spacing = def.map(|d| d.spacing).unwrap_or_default();
    let direct_sp = parse_spacing_attrs(e);
    spacing.left = direct_sp.left.or(spacing.left);
    spacing.right = direct_sp.right.or(spacing.right);
    spacing.first = direct_sp.first.or(spacing.first);
    spacing.before = direct_sp.before.or(spacing.before);
    spacing.after = direct_sp.after.or(spacing.after);
    spacing.line = direct_sp.line.or(spacing.line);

    let align = attr_val(e, "Justification").as_deref().and_then(icml_to_align).or_else(|| def.and_then(|d| d.align));
    let list = parse_list_attrs(e).or_else(|| def.and_then(|d| d.list));
    let page_break = matches!(attr_val(e, "StartParagraph").as_deref(), Some("NextPage" | "NextOddPage" | "NextEvenPage"));

    let extra = custom_id.map(|id| Box::new(ParaProps { style: Some(id), ..Default::default() }));

    Para { kind, list, align, page_break, spacing, extra, ..Default::default() }
}

fn open_para_range(st: &mut ParseState, e: &BytesStart<'_>) {
    flush_open_para(st);
    let tmpl = build_para_template(st, e);
    if let Some(tbl) = st.table_stack.last_mut()
        && let Some(cell) = tbl.cur_cell.as_mut()
    {
        cell.para_template = tmpl.clone();
        cell.cur_para = Some(tmpl);
        cell.ended_with_br = false;
        return;
    }
    st.para_template = tmpl.clone();
    st.cur_para = Some(tmpl);
    st.in_para_range = true;
    st.ended_with_br = false;
}

fn close_para_range(st: &mut ParseState) {
    if let Some(tbl) = st.table_stack.last_mut()
        && let Some(cell) = tbl.cur_cell.as_mut()
    {
        if let Some(p) = cell.cur_para.take()
            && (!cell.ended_with_br || !p.inlines.is_empty())
        {
            cell.blocks.push(FBlock::Para(p));
        }
        cell.ended_with_br = false;
        return;
    }
    if let Some(p) = st.cur_para.take()
        && (!st.ended_with_br || !p.inlines.is_empty())
    {
        st.blocks.push(FBlock::Para(p));
    }
    st.in_para_range = false;
    st.ended_with_br = false;
}

fn flush_open_para(st: &mut ParseState) {
    if st.in_para_range {
        close_para_range(st);
    } else if let Some(p) = st.cur_para.take()
        && !p.inlines.is_empty()
    {
        st.blocks.push(FBlock::Para(p));
    }
}

fn emit_paragraph_break(st: &mut ParseState) {
    if let Some(tbl) = st.table_stack.last_mut()
        && let Some(cell) = tbl.cur_cell.as_mut()
    {
        let finished = cell.cur_para.take().unwrap_or_else(|| cell.para_template.clone());
        cell.blocks.push(FBlock::Para(finished));
        cell.cur_para = Some(cell.para_template.clone());
        cell.ended_with_br = true;
        return;
    }
    let finished = st.cur_para.take().unwrap_or_else(|| st.para_template.clone());
    st.blocks.push(FBlock::Para(finished));
    if st.in_para_range {
        st.cur_para = Some(st.para_template.clone());
    }
    st.ended_with_br = true;
}

fn build_char_fmt(st: &ParseState, e: &BytesStart<'_>) -> Fmt {
    let mut base = st.char_stack.last().cloned().unwrap_or_default();
    if let Some(raw_cs) = attr_val(e, "AppliedCharacterStyle") {
        let cname = normalize_style_ref(&raw_cs, "CharacterStyle/");
        if let Some(cs) = st.char_defs.get(&cname) {
            base = overlay_fmt(&cs.fmt, &base);
            if !cname.starts_with('[') && cname != "Default Paragraph Font" {
                let cid: String = cname.chars().filter(|c| c.is_alphanumeric() || *c == '_').take(64).collect();
                if !cid.is_empty() {
                    base.extra.get_or_insert_with(Default::default).style = Some(cid);
                }
            }
        }
    }
    let mut out = parse_inline_fmt_attrs(&st.swatches, e, base);
    if let Some(font) = attr_val(e, "AppliedFont") {
        out.font = kept_font(&font);
    }
    out
}

fn overlay_fmt(style_fmt: &Fmt, prev: &Fmt) -> Fmt {
    let mut f = prev.clone();
    if style_fmt.bold {
        f.bold = true;
    }
    if style_fmt.italic {
        f.italic = true;
    }
    if style_fmt.underline {
        f.underline = true;
    }
    if style_fmt.strike {
        f.strike = true;
    }
    if style_fmt.sup {
        f.sup = true;
        f.sub = false;
    } else if style_fmt.sub {
        f.sub = true;
        f.sup = false;
    }
    if style_fmt.color.is_some() {
        f.color = style_fmt.color;
    }
    if style_fmt.size.is_some() {
        f.size = style_fmt.size;
    }
    if style_fmt.font.is_some() {
        f.font.clone_from(&style_fmt.font);
    }
    f
}

fn effective_span_fmt(st: &ParseState) -> Fmt {
    let mut f = st.char_stack.last().cloned().unwrap_or_default();
    if let Some(src_id) = st.active_link_source.as_deref() {
        // Temporary marker resolved to actual URL at EOF once `<Hyperlink>` bindings are read.
        f.link = Some(Arc::from(format!("icml-src:{src_id}")));
    }
    for chg in &st.change_stack {
        let mark = RevMark { author: chg.author.clone(), date: chg.date.clone(), id: Some(chg.id) };
        match chg.kind {
            ChangeKind::Insert | ChangeKind::Move => f.ins = Some(mark),
            ChangeKind::Delete => f.del = Some(mark),
        }
    }
    f
}

fn append_text_to_current_para(st: &mut ParseState, text: &str, fmt: &Fmt) {
    if let Some(tbl) = st.table_stack.last_mut()
        && let Some(cell) = tbl.cur_cell.as_mut()
    {
        let p = cell.cur_para.get_or_insert_with(|| cell.para_template.clone());
        p.push_text(text, fmt);
        cell.ended_with_br = false;
        return;
    }
    let p = st.cur_para.get_or_insert_with(|| st.para_template.clone());
    p.push_text(text, fmt);
    st.ended_with_br = false;
}

fn push_completed_block(st: &mut ParseState, block: FBlock) {
    if let Some(tbl) = st.table_stack.last_mut()
        && let Some(cell) = tbl.cur_cell.as_mut()
    {
        if let Some(p) = cell.cur_para.take()
            && !p.inlines.is_empty()
        {
            cell.blocks.push(FBlock::Para(p));
        }
        cell.blocks.push(block);
        cell.ended_with_br = true;
        return;
    }
    if let Some(p) = st.cur_para.take()
        && !p.inlines.is_empty()
    {
        st.blocks.push(FBlock::Para(p));
    }
    st.blocks.push(block);
    st.ended_with_br = true;
}

fn parse_cell_coords(name: &str) -> (usize, usize) {
    let Some((c, r)) = name.split_once(':') else {
        return (0, 0);
    };
    (c.parse::<usize>().unwrap_or(0).min(63), r.parse::<usize>().unwrap_or(0).min(10_000))
}

fn finalize_table(tbl: TableBuild) -> Option<FTable> {
    if tbl.cells.is_empty() {
        return None;
    }
    let max_row = tbl.cells.iter().map(|c| c.row).max().unwrap_or(0);
    let nrows = (max_row + 1).min(wordcraft_doc::table::MAX_ROWS);
    let mut rows: Vec<Vec<(usize, Cell)>> = (0..nrows).map(|_| Vec::new()).collect();
    for cb in tbl.cells {
        if let Some(slot) = rows.get_mut(cb.row) {
            let cell = Cell { blocks: cb.blocks, colspan: cb.colspan, rowspan: cb.rowspan, header: cb.row < tbl.header_rows, ..Default::default() };
            slot.push((cb.col, cell));
        }
    }
    let mut final_rows = Vec::with_capacity(nrows);
    for mut r in rows {
        r.sort_by_key(|(col, _)| *col);
        let cells: Vec<Cell> = r.into_iter().take(wordcraft_doc::table::MAX_COLS).map(|(_, c)| c).collect();
        if !cells.is_empty() {
            final_rows.push(cells);
        }
    }
    if final_rows.is_empty() {
        return None;
    }
    let mut ft = FTable { rows: final_rows, widths: Vec::new(), borderless: false, extra: None, kept: None, wrap: None };
    ft.insert_covered();
    Some(ft)
}

fn resolve_deferred_links(blocks: &mut [FBlock], source_to_dest: &HashMap<String, String>, dest_urls: &HashMap<String, String>) {
    resolve_links_inner(blocks, 0, source_to_dest, dest_urls);
}

fn resolve_links_inner(blocks: &mut [FBlock], depth: usize, source_to_dest: &HashMap<String, String>, dest_urls: &HashMap<String, String>) {
    if depth > model::MAX_DEPTH {
        return;
    }
    for b in blocks {
        match b {
            FBlock::Para(p) => {
                for inl in &mut p.inlines {
                    if let Inline::Text(_, fmt) = inl
                        && let Some(marker) = fmt.link.as_deref()
                        && let Some(src_id) = marker.strip_prefix("icml-src:")
                    {
                        let resolved = source_to_dest
                            .get(src_id)
                            .and_then(|dest_id| dest_urls.get(dest_id))
                            .or_else(|| src_id.strip_prefix("htss_").and_then(|idx| dest_urls.get(&format!("HyperlinkURLDestination/d{idx}"))));
                        fmt.link = resolved.and_then(|u| kept_link(u));
                    }
                }
            }
            FBlock::Table(t) => {
                for cell in t.rows.iter_mut().flatten() {
                    resolve_links_inner(&mut cell.blocks, depth + 1, source_to_dest, dest_urls);
                }
            }
        }
    }
}

fn apply_imported_styles(doc: &mut Document, pstyles: &[ImportedParaStyle], cstyles: &[ImportedCharStyle]) {
    let styles = Arc::make_mut(&mut doc.styles);
    for ps in pstyles {
        let (kind, custom_id) = classify_style_name(&ps.name);
        if kind != Kind::Normal {
            continue;
        }
        let Some(id) = custom_id else { continue };
        if styles.get(&id).is_some() {
            continue;
        }
        styles.upsert(Style {
            id,
            name: ps.name.clone(),
            kind: StyleKind::Paragraph,
            based_on: Some("Normal".to_string()),
            para: ParaProps {
                align: ps.align,
                indent_left: ps.spacing.left,
                indent_right: ps.spacing.right,
                indent_first: ps.spacing.first,
                space_before: ps.spacing.before,
                space_after: ps.spacing.after,
                line_spacing: ps.spacing.line,
                ..Default::default()
            },
            chr: model::char_props(&ps.fmt),
            quick: true,
            ..Default::default()
        });
    }
    for cs in cstyles {
        let id: String = cs.name.chars().filter(|c| c.is_alphanumeric() || *c == '_').take(64).collect();
        if id.is_empty() || styles.get(&id).is_some() {
            continue;
        }
        styles.upsert(Style {
            id,
            name: cs.name.clone(),
            kind: StyleKind::Character,
            based_on: Some("DefaultParagraphFont".to_string()),
            chr: model::char_props(&cs.fmt),
            quick: true,
            ..Default::default()
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wordcraft_doc::{Paragraph, Revision, RevisionKind, StoryRef, para_block};

    #[test]
    fn icml_round_trip_preserves_styles_formatting_and_tracked_changes() {
        let mut doc = Document::new();
        doc.settings.track_changes = true;
        doc.revisions.push(Revision::new(RevisionKind::Insert, "Alice Editor", "2026-11-27T09:00:00Z"));
        doc.revisions.push(Revision::new(RevisionKind::Delete, "Bob CopyDesk", "2026-11-27T09:15:00Z"));
        let mut move_rev = Revision::new(RevisionKind::Insert, "Carol Sub", "2026-11-27T09:30:00Z");
        move_rev.move_name = Some("m1".to_string());
        doc.revisions.push(move_rev);

        let mut h1 = Paragraph::with_text("Editorial Headline", CharProps::default()).styled("Heading1");
        h1.props.align = Some(Align::Center);

        let mut body = Paragraph::with_text("Lead ", CharProps::default());
        let n = body.len();
        body.insert_text(
            n,
            "bold italic",
            &CharProps {
                bold: Some(true),
                italic: Some(true),
                underline: Some(Underline::Single),
                font: Some("Georgia".to_string()),
                size: Some(14.0),
                color: Some(TextColor::Rgb(Rgb(0xCC, 0x22, 0x11))),
                ..Default::default()
            },
        )
        .unwrap();
        let n = body.len();
        body.insert_text(n, " inserted", &CharProps { ins: Some(0), bold: Some(true), ..Default::default() }).unwrap();
        let n = body.len();
        body.insert_text(n, " removed", &CharProps { del: Some(1), ..Default::default() }).unwrap();
        let n = body.len();
        body.insert_text(n, " relocated", &CharProps { ins: Some(2), ..Default::default() }).unwrap();

        doc.body = vec![para_block(h1), para_block(body)];

        let xml = export(&doc);
        assert!(is_icml(xml.as_bytes()));
        assert!(xml.contains("DOMVersion=\"22.0\""));
        assert!(xml.contains("ChangeType=\"InsertedText\""));
        assert!(xml.contains("ChangeType=\"DeletedText\""));
        assert!(xml.contains("ChangeType=\"MovedText\""));

        let back = import(xml.as_bytes()).unwrap();
        assert!(back.settings.track_changes);
        assert_eq!(back.final_text(StoryRef::Body), "Editorial Headline\nLead bold italic inserted relocated");
        assert!(back.revisions.iter().any(|r| r.kind == RevisionKind::Insert && r.author == "Alice Editor"));
        assert!(back.revisions.iter().any(|r| r.kind == RevisionKind::Delete && r.author == "Bob CopyDesk"));
        assert!(back.revisions.iter().any(|r| r.move_name.is_some() && r.author == "Carol Sub"));

        let p0 = back.body[0].as_para().unwrap();
        assert_eq!(p0.props.style.as_deref(), Some("Heading1"));
        assert_eq!(p0.props.align, Some(Align::Center));

        let p1 = back.body[1].as_para().unwrap();
        let styled_run = p1.runs.iter().find(|r| r.props.bold == Some(true) && r.props.italic == Some(true)).unwrap();
        assert_eq!(styled_run.props.underline, Some(Underline::Single));
        assert_eq!(styled_run.props.font.as_deref(), Some("Georgia"));
        assert_eq!(styled_run.props.size, Some(14.0));
        assert_eq!(styled_run.props.color, Some(TextColor::Rgb(Rgb(0xCC, 0x22, 0x11))));
    }

    #[test]
    fn hostile_and_malformed_icml_never_panics() {
        assert!(import(b"not xml at all").is_err());
        assert!(import(b"<html><body>wrong xml</body></html>").is_err());
        let deep = "<Document DOMVersion=\"22.0\"><Story>".to_string() + &"<ParagraphStyleRange>".repeat(80) + "</Story></Document>";
        assert!(import(deep.as_bytes()).is_err());
    }
}
