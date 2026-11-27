//! Books (File › New / Open Book): an ordered list of documents with continuous / odd / even page
//! numbering, granular style & resource synchronization from a style source, book-wide preflight,
//! and whole-book PDF or EPUB 3.0 export. Supports both `.indb` (XML/ZIP + binary InDesign book
//! path scanning) and `.dcbook` JSON.

use std::collections::{HashMap, HashSet};
use std::fmt::Write as _;

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use super::{CommandSpec, always, bad, cmd, ok, str_param};
use crate::{EngineError, Result, Session};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PageNumberingMode {
    /// Continue from Previous Document.
    #[default]
    Continue,
    /// Continue on Next Odd Page.
    NextOdd,
    /// Continue on Next Even Page.
    NextEven,
}

impl PageNumberingMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Continue => "continue",
            Self::NextOdd => "nextOdd",
            Self::NextEven => "nextEven",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "continue" | "Continue" => Some(Self::Continue),
            "nextOdd" | "NextOdd" | "odd" => Some(Self::NextOdd),
            "nextEven" | "NextEven" | "even" => Some(Self::NextEven),
            _ => None,
        }
    }

    fn advance(self, n: u32) -> (u32, bool) {
        match self {
            Self::Continue => (n, false),
            Self::NextOdd => {
                if n.is_multiple_of(2) {
                    (n.saturating_add(1), true)
                } else {
                    (n, false)
                }
            }
            Self::NextEven => {
                if !n.is_multiple_of(2) {
                    (n.saturating_add(1), true)
                } else {
                    (n, false)
                }
            }
        }
    }
}

fn yes() -> bool {
    true
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncOptions {
    #[serde(default = "yes")]
    pub paragraph_styles: bool,
    #[serde(default = "yes")]
    pub character_styles: bool,
    #[serde(default = "yes")]
    pub object_styles: bool,
    #[serde(default = "yes")]
    pub table_styles: bool,
    #[serde(default = "yes")]
    pub cell_styles: bool,
    #[serde(default = "yes")]
    pub swatches: bool,
    #[serde(default = "yes")]
    pub parent_pages: bool,
    #[serde(default = "yes")]
    pub numbered_lists: bool,
    #[serde(default = "yes")]
    pub text_variables: bool,
    #[serde(default = "yes")]
    pub conditional_text: bool,
}

impl Default for SyncOptions {
    fn default() -> Self {
        Self {
            paragraph_styles: true,
            character_styles: true,
            object_styles: true,
            table_styles: true,
            cell_styles: true,
            swatches: true,
            parent_pages: true,
            numbered_lists: true,
            text_variables: true,
            conditional_text: true,
        }
    }
}

impl SyncOptions {
    fn apply_json(&mut self, p: &Value) {
        let set = |key: &str, field: &mut bool| {
            if let Some(v) = p.get(key).and_then(Value::as_bool) {
                *field = v;
            }
        };
        set("paragraphStyles", &mut self.paragraph_styles);
        set("characterStyles", &mut self.character_styles);
        set("objectStyles", &mut self.object_styles);
        set("tableStyles", &mut self.table_styles);
        set("cellStyles", &mut self.cell_styles);
        set("swatches", &mut self.swatches);
        set("parentPages", &mut self.parent_pages);
        set("numberedLists", &mut self.numbered_lists);
        set("textVariables", &mut self.text_variables);
        set("conditionalText", &mut self.conditional_text);
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Book {
    #[serde(skip)]
    pub path: String,
    pub documents: Vec<String>,
    /// Index of the style source document.
    pub style_source: usize,
    pub page_numbering: PageNumberingMode,
    pub insert_blank_page: bool,
    #[serde(default = "yes")]
    pub continue_lists: bool,
    pub sync_options: SyncOptions,
}

impl Default for Book {
    fn default() -> Self {
        Self {
            path: String::new(),
            documents: Vec::new(),
            style_source: 0,
            page_numbering: PageNumberingMode::Continue,
            insert_blank_page: false,
            continue_lists: true,
            sync_options: SyncOptions::default(),
        }
    }
}

fn has_book(s: &Session) -> std::result::Result<(), String> {
    if s.book.is_some() { Ok(()) } else { Err("no book open".into()) }
}

fn no_book() -> EngineError {
    bad("book", "no book open")
}

fn is_indb_path(path: &str) -> bool {
    std::path::Path::new(path).extension().and_then(std::ffi::OsStr::to_str).is_some_and(|e| e.eq_ignore_ascii_case("indb"))
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

pub(crate) fn book_to_xml(b: &Book) -> String {
    let mut s = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n");
    let _ = writeln!(
        s,
        "<Book DOMVersion=\"20.4\" StyleSource=\"{}\" PageNumbering=\"{}\" InsertBlankPage=\"{}\" ContinueLists=\"{}\">",
        b.style_source,
        b.page_numbering.as_str(),
        b.insert_blank_page,
        b.continue_lists,
    );
    let o = &b.sync_options;
    let _ = writeln!(
        s,
        "  <SyncOptions ParagraphStyles=\"{}\" CharacterStyles=\"{}\" ObjectStyles=\"{}\" TableStyles=\"{}\" CellStyles=\"{}\" Swatches=\"{}\" ParentPages=\"{}\" NumberedLists=\"{}\" TextVariables=\"{}\" ConditionalText=\"{}\"/>",
        o.paragraph_styles,
        o.character_styles,
        o.object_styles,
        o.table_styles,
        o.cell_styles,
        o.swatches,
        o.parent_pages,
        o.numbered_lists,
        o.text_variables,
        o.conditional_text,
    );
    for (idx, doc_path) in b.documents.iter().enumerate() {
        let _ = writeln!(
            s,
            "  <BookContent Self=\"u{idx}\" FullName=\"{}\" Index=\"{idx}\" SynchronizeMaster=\"{}\"/>",
            xml_escape(doc_path),
            idx == b.style_source,
        );
    }
    s.push_str("</Book>\n");
    s
}

pub(crate) fn encode_book_bytes(b: &Book) -> Result<Vec<u8>> {
    if is_indb_path(&b.path) {
        let xml = book_to_xml(b).into_bytes();
        let manifest = serde_json::to_vec_pretty(b).map_err(|e| EngineError::Other(e.to_string()))?;
        let parts = vec![("Book.xml".to_string(), xml.clone()), ("manifest.json".to_string(), manifest)];
        Ok(craft_idml::write_package(&parts).unwrap_or(xml))
    } else {
        serde_json::to_vec_pretty(b).map_err(|e| EngineError::Other(e.to_string()))
    }
}

fn parse_book_xml(xml: &str) -> std::result::Result<Book, String> {
    let opts = craft_xml::Options::default();
    let root = opts.parse(xml.as_bytes()).map_err(|e| format!("invalid Book XML: {e}"))?;
    if root.name != "Book" && !root.name.ends_with(":Book") {
        return Err(format!("expected <Book>, found <{}>", root.name));
    }
    fn attr<'a>(el: &'a craft_xml::El, k: &str) -> Option<&'a str> {
        el.attrs.iter().find(|(n, _)| n == k).map(|(_, v)| v.as_str())
    }
    let mut b = Book::default();
    if let Some(ss) = attr(&root, "StyleSource").and_then(|v| v.parse::<usize>().ok()) {
        b.style_source = ss;
    }
    if let Some(pm) = attr(&root, "PageNumbering").and_then(PageNumberingMode::parse) {
        b.page_numbering = pm;
    }
    if let Some(ib) = attr(&root, "InsertBlankPage") {
        b.insert_blank_page = ib.eq_ignore_ascii_case("true") || ib == "1";
    }
    if let Some(cl) = attr(&root, "ContinueLists") {
        b.continue_lists = cl.eq_ignore_ascii_case("true") || cl == "1";
    }
    for node in &root.children {
        let craft_xml::Node::El(child) = node else { continue };
        match child.name.as_str() {
            "SyncOptions" => {
                let flag = |k: &str, d: bool| attr(child, k).map(|v| v.eq_ignore_ascii_case("true") || v == "1").unwrap_or(d);
                b.sync_options.paragraph_styles = flag("ParagraphStyles", true);
                b.sync_options.character_styles = flag("CharacterStyles", true);
                b.sync_options.object_styles = flag("ObjectStyles", true);
                b.sync_options.table_styles = flag("TableStyles", true);
                b.sync_options.cell_styles = flag("CellStyles", true);
                b.sync_options.swatches = flag("Swatches", true);
                b.sync_options.parent_pages = flag("ParentPages", true);
                b.sync_options.numbered_lists = flag("NumberedLists", true);
                b.sync_options.text_variables = flag("TextVariables", true);
                b.sync_options.conditional_text = flag("ConditionalText", true);
            }
            "BookContent" => {
                if let Some(full) = attr(child, "FullName").or_else(|| attr(child, "FilePath")) {
                    if attr(child, "SynchronizeMaster").is_some_and(|v| v.eq_ignore_ascii_case("true")) {
                        b.style_source = b.documents.len();
                    }
                    b.documents.push(full.to_string());
                }
            }
            _ => {}
        }
    }
    if b.style_source >= b.documents.len() {
        b.style_source = 0;
    }
    Ok(b)
}

/// Scans binary Adobe InDesign `.indb` database streams (magic `06 06 ED F5 D8 1D 46 E5`) for
/// embedded UTF-8 or UTF-16LE chapter file paths (`.indd`, `.idml`, `.indc`, `.designcraft`).
fn scan_binary_indb_paths(bytes: &[u8]) -> Option<Book> {
    const INDB_MAGIC: [u8; 8] = [0x06, 0x06, 0xED, 0xF5, 0xD8, 0x1D, 0x46, 0xE5];
    let has_magic = bytes.starts_with(&INDB_MAGIC);
    let mut found = Vec::new();
    let mut seen = HashSet::new();

    let mut collect_from = |text: &str| {
        for raw in text.split(|c: char| c.is_control() || matches!(c, '"' | '<' | '>' | '|' | '*' | '?')) {
            let trimmed = raw.trim();
            if trimmed.len() < 6 || trimmed.len() > 1024 {
                continue;
            }
            let lower = trimmed.to_ascii_lowercase();
            for ext in [".designcraft", ".idml", ".indd", ".indc"] {
                if let Some(pos) = lower.find(ext) {
                    let candidate = &trimmed[..pos + ext.len()];
                    let cleaned = candidate.trim_start_matches(|c: char| !c.is_alphanumeric() && !matches!(c, '/' | '\\' | '.'));
                    if cleaned.len() > ext.len() && seen.insert(cleaned.to_string()) {
                        found.push(cleaned.to_string());
                    }
                }
            }
        }
    };

    collect_from(&String::from_utf8_lossy(bytes));
    if bytes.len() >= 4 {
        let u16_le: Vec<u16> = bytes.chunks_exact(2).map(|c| u16::from_le_bytes([c[0], c[1]])).collect();
        collect_from(&String::from_utf16_lossy(&u16_le));
    }

    if !found.is_empty() || has_magic { Some(Book { documents: found, ..Default::default() }) } else { None }
}

pub(crate) fn decode_book_bytes(bytes: &[u8]) -> std::result::Result<Book, String> {
    if bytes.starts_with(b"PK\x03\x04") {
        use std::io::Read as _;
        let mut archive = zip::ZipArchive::new(std::io::Cursor::new(bytes)).map_err(|e| format!("invalid .indb archive: {e}"))?;
        if let Ok(mut mf) = archive.by_name("manifest.json") {
            let mut buf = Vec::new();
            mf.read_to_end(&mut buf).map_err(|e| e.to_string())?;
            if let Ok(book) = serde_json::from_slice::<Book>(&buf) {
                return Ok(book);
            }
        }
        for idx in 0..archive.len() {
            let mut entry = archive.by_index(idx).map_err(|e| e.to_string())?;
            if entry.name().ends_with(".xml") {
                let mut xml_bytes = Vec::new();
                entry.read_to_end(&mut xml_bytes).map_err(|e| e.to_string())?;
                let xml = String::from_utf8_lossy(&xml_bytes);
                return parse_book_xml(&xml);
            }
        }
        return Err("no Book.xml or manifest.json inside .indb archive".into());
    }
    if let Ok(book) = serde_json::from_slice::<Book>(bytes) {
        return Ok(book);
    }
    let trimmed = bytes.trim_ascii_start();
    if trimmed.starts_with(b"<")
        && let Ok(text) = std::str::from_utf8(bytes)
    {
        return parse_book_xml(text);
    }
    scan_binary_indb_paths(bytes).ok_or_else(|| "unrecognized book file format".into())
}

#[cfg(not(target_arch = "wasm32"))]
fn save(b: &Book) -> Result<()> {
    let bytes = encode_book_bytes(b)?;
    crate::io::write_file(&b.path, &bytes).map_err(EngineError::Other)
}

#[cfg(target_arch = "wasm32")]
fn save(_: &Book) -> Result<()> {
    Err(EngineError::Other("books need a file system".into()))
}

/// A chapter's file, resolved against the book's file. A chapter on another computer (other than
/// the book's own network share) or a device is not opened ([`crate::io::document_path`]).
fn chapter(b: &Book, path: &str) -> Result<String> {
    crate::io::document_path(path, Some(std::path::Path::new(&b.path)))
        .map(|p| p.to_string_lossy().into_owned())
        .map_err(|why| EngineError::Other(format!("{path}: {why}")))
}

/// Every chapter's file ([`chapter`]), in order.
fn chapters(b: &Book) -> Result<Vec<String>> {
    b.documents.iter().map(|path| chapter(b, path)).collect()
}

fn load(path: &str) -> Result<designcraft_doc::Document> {
    #[cfg(not(target_arch = "wasm32"))]
    let bytes = crate::io::read_file(path).map_err(EngineError::Other)?;
    #[cfg(target_arch = "wasm32")]
    let bytes: Vec<u8> = {
        let _ = path;
        vec![]
    };
    if craft_file_formats::detect(path, &bytes) == Some(craft_file_formats::Format::Idml) {
        return super::interchange::import_idml_confined(&bytes, Some(std::path::Path::new(path)))
            .map_err(|e| EngineError::Other(format!("{path}: {e}")));
    }
    super::file::from_bytes(&bytes).map_err(|e| EngineError::Other(format!("{path}: {e}")))
}

#[cfg(not(target_arch = "wasm32"))]
const MAX_BATCH_CHAPTERS: usize = 4096;

/// A batch keeps only bounded path records in memory. Chapter snapshots live in a private
/// temporary directory; only one document and serialized blob are live while staging.
#[cfg(not(target_arch = "wasm32"))]
struct ChapterBatch {
    directory: tempfile::TempDir,
    chapters: Vec<(String, tempfile::TempPath)>,
    command: &'static str,
}

#[cfg(not(target_arch = "wasm32"))]
impl ChapterBatch {
    fn new(chapters: usize, command: &'static str) -> Result<Self> {
        if chapters > MAX_BATCH_CHAPTERS {
            return Err(bad(command, format!("a book update supports at most {MAX_BATCH_CHAPTERS} chapters")));
        }
        let directory = tempfile::Builder::new()
            .prefix(".designcraft-book-")
            .tempdir()
            .map_err(|e| EngineError::Other(format!("{command}: cannot stage chapter changes: {e}")))?;
        Ok(Self { directory, chapters: Vec::with_capacity(chapters), command })
    }

    fn stage(&mut self, path: &str, doc: &designcraft_doc::Document) -> Result<()> {
        use std::io::Write;

        let bytes = designcraft_format::save(doc).map_err(|e| EngineError::Other(format!("{path}: {e}")))?;
        let mut file = tempfile::Builder::new()
            .prefix("chapter-")
            .tempfile_in(self.directory.path())
            .map_err(|e| EngineError::Other(format!("{path}: cannot stage changes: {e}")))?;
        file.write_all(&bytes).map_err(|e| EngineError::Other(format!("{path}: cannot stage changes: {e}")))?;
        self.chapters.push((path.to_string(), file.into_temp_path()));
        Ok(())
    }

    fn commit(self) -> Result<()> {
        self.commit_with(|path, staged| crate::io::replace_with(path, staged))
    }

    fn commit_with(self, mut write: impl FnMut(&std::path::Path, &std::path::Path) -> std::io::Result<()>) -> Result<()> {
        let mut completed = Vec::with_capacity(self.chapters.len());
        for (path, staged) in self.chapters {
            if let Err(e) = write(std::path::Path::new(&path), &staged) {
                return Err(EngineError::Other(format!(
                    "{}: failed to save chapter {path}: {e}; completed chapter paths: {completed:?}",
                    self.command
                )));
            }
            completed.push(path);
        }
        Ok(())
    }
}

/// Batch commands only write closed native chapters. IDML is an imported source, and an open
/// chapter's model (including unsaved edits) cannot be reconciled by a disk-only book update.
#[cfg(not(target_arch = "wasm32"))]
fn writable_chapter(s: &Session, path: &str, command: &'static str) -> Result<()> {
    use std::path::Path;

    if Path::new(path)
        .extension()
        .and_then(std::ffi::OsStr::to_str)
        .is_some_and(|ext| craft_file_formats::from_extension(ext) == Some(craft_file_formats::Format::Idml))
    {
        return Err(bad(command, format!("cannot change imported IDML chapter {path}; convert it to a native DesignCraft document first")));
    }
    let target = crate::io::canonicalize(path).map_err(|e| EngineError::Other(format!("{path}: {e}")))?;
    if s.docs.iter().any(|doc| doc.path.as_ref().is_some_and(|open| crate::io::canonicalize(open).is_ok_and(|open| open == target))) {
        return Err(bad(command, format!("chapter {path} is open; close it before updating the book")));
    }
    if !crate::io::is_file(&target) {
        return Err(bad(command, format!("chapter {path} is not a regular file")));
    }
    crate::io::writable(&target).map_err(|e| EngineError::Other(format!("{path}: {e}")))?;
    let parent = target.parent().ok_or_else(|| bad(command, format!("chapter {path} has no parent folder")))?;
    tempfile::Builder::new().prefix(".designcraft-book-check-").tempfile_in(parent).map_err(|e| EngineError::Other(format!("{path}: {e}")))?;
    Ok(())
}

#[cfg(not(target_arch = "wasm32"))]
fn paginate(s: &mut Session, p: &Value) -> Result<Value> {
    let mut b = s.book.as_ref().ok_or_else(no_book)?.clone();
    if let Some(m) = str_param(p, "mode") {
        b.page_numbering =
            PageNumberingMode::parse(m).ok_or_else(|| bad("book.paginate", format!("unknown mode `{m}` (continue, nextOdd, nextEven)")))?;
    }
    if let Some(ib) = p.get("insertBlankPage").and_then(Value::as_bool) {
        b.insert_blank_page = ib;
    }
    if let Some(ul) = p.get("updateLists").and_then(Value::as_bool) {
        b.continue_lists = ul;
    }
    if b.documents.is_empty() {
        s.book = Some(b);
        return Ok(json!({"pages": 0, "chapters": [], "blankPagesInserted": 0}));
    }

    let paths = chapters(&b)?;
    for path in &paths {
        writable_chapter(s, path, "book.paginate")?;
    }
    let mut docs: Vec<designcraft_doc::Document> = paths.iter().map(|cp| load(cp)).collect::<Result<_>>()?;

    let mut next = 1u32;
    let mut chapter_num = 0u32;
    let mut chapters_out = Vec::with_capacity(docs.len());
    let mut blank_inserted = 0usize;

    for idx in 0..docs.len() {
        if idx > 0 {
            let (target_next, skipped) = b.page_numbering.advance(next);
            if skipped && b.insert_blank_page {
                let prev = &mut docs[idx - 1];
                let after = prev.page_count().checked_sub(1);
                let _ = prev.insert_pages(after, 1, None);
                blank_inserted += 1;
            }
            next = target_next;
        }
        let d = &mut docs[idx];
        start_at(d, next);
        if b.continue_lists {
            for list in &mut d.settings.lists {
                list.continue_across_stories = true;
            }
        }
        let pages = u32::try_from(d.page_count()).map_err(|_| bad("book.paginate", "too many pages for continuous numbering"))?;
        next = next.checked_add(pages).ok_or_else(|| bad("book.paginate", "too many pages for continuous numbering"))?;
        chapter_num = match d.settings.chapter_source {
            designcraft_doc::ChapterSource::Automatic => chapter_num.saturating_add(1),
            designcraft_doc::ChapterSource::UserDefined => d.settings.chapter_number.max(1),
            designcraft_doc::ChapterSource::SameAsPrevious => chapter_num.max(1),
        };
        d.settings.chapter_number = chapter_num;
        chapters_out.push(d.chapter_label());
    }

    let mut batch = ChapterBatch::new(docs.len(), "book.paginate")?;
    for (path, doc) in paths.iter().zip(&docs) {
        batch.stage(path, doc)?;
    }
    batch.commit()?;
    let _ = save(&b);
    s.book = Some(b);
    Ok(json!({
        "pages": next.saturating_sub(1),
        "chapters": chapters_out,
        "blankPagesInserted": blank_inserted,
    }))
}

/// `list` with only the last entry of each name, in the order of those last entries.
#[cfg(not(target_arch = "wasm32"))]
fn keep_last<T>(list: &[T], name: impl Fn(&T) -> &str) -> Vec<&T> {
    let mut seen = HashSet::new();
    let mut out: Vec<&T> = list.iter().rev().filter(|x| seen.insert(name(x))).collect();
    out.reverse();
    out
}

#[cfg(not(target_arch = "wasm32"))]
fn merge_named<T: Clone>(dst: &mut Vec<T>, src: &[T], name: impl Fn(&T) -> &str) {
    let kept = keep_last(src, &name);
    let names: HashSet<&str> = kept.iter().map(|x| name(x)).collect();
    dst.retain(|x| !names.contains(name(x)));
    dst.extend(kept.into_iter().cloned());
}

#[cfg(not(target_arch = "wasm32"))]
fn sync_chapter_styles(d: &mut designcraft_doc::Document, src: &designcraft_doc::Document, opts: &SyncOptions) {
    let st = d.styles_mut();
    if opts.paragraph_styles {
        merge_named(&mut st.paragraph, &src.styles.paragraph, |x| x.name.as_str());
    }
    if opts.character_styles {
        merge_named(&mut st.character, &src.styles.character, |x| x.name.as_str());
    }
    if opts.object_styles {
        merge_named(&mut st.object, &src.styles.object, |x| x.name.as_str());
    }
    if opts.table_styles {
        merge_named(&mut st.table, &src.styles.table, |x| x.name.as_str());
    }
    if opts.cell_styles {
        merge_named(&mut st.cell, &src.styles.cell, |x| x.name.as_str());
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn sync_chapter_extras(d: &mut designcraft_doc::Document, src: &designcraft_doc::Document, opts: &SyncOptions) {
    if opts.swatches {
        let mut at: HashMap<String, usize> = HashMap::new();
        for (i, w) in d.swatches.iter().enumerate() {
            at.entry(w.name.clone()).or_insert(i);
        }
        for sw in &src.swatches {
            match at.get(&sw.name).and_then(|i| d.swatches.get_mut(*i)) {
                Some(w) if !w.locked => *w = sw.clone(),
                Some(_) => {}
                None => {
                    at.insert(sw.name.clone(), d.swatches.len());
                    d.swatches.push(sw.clone());
                }
            }
        }
    }
    if opts.numbered_lists {
        merge_named(&mut d.settings.lists, &src.settings.lists, |x| x.name.as_str());
    }
    if opts.text_variables {
        merge_named(&mut d.text_variables, &src.text_variables, |x| x.name.as_str());
    }
    if opts.conditional_text {
        merge_named(&mut d.conditions, &src.conditions, |x| x.name.as_str());
    }
    if opts.parent_pages {
        for parent in &src.parents {
            let label = parent.parent.as_ref().map(|p| format!("{}-{}", p.prefix, p.name)).unwrap_or_default();
            if let Some(existing) =
                d.parents.iter_mut().find(|ep| ep.parent.as_ref().map(|p| format!("{}-{}", p.prefix, p.name)).unwrap_or_default() == label)
            {
                let mut cloned = (**parent).clone();
                cloned.id = existing.id;
                *existing = std::sync::Arc::new(cloned);
            } else {
                let mut cloned = (**parent).clone();
                cloned.id = designcraft_doc::SpreadId(d.alloc());
                d.parents.push(std::sync::Arc::new(cloned));
            }
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn sync_styles(s: &mut Session, p: &Value) -> Result<Value> {
    let mut b = s.book.as_ref().ok_or_else(no_book)?.clone();
    let mut opts = b.sync_options.clone();
    opts.apply_json(p);
    b.sync_options = opts.clone();

    let mut batch = ChapterBatch::new(b.documents.len(), "book.syncStyles")?;
    let paths = chapters(&b)?;
    let src_path = paths.get(b.style_source).ok_or_else(|| bad("book.syncStyles", "the book is empty"))?;
    let src = load(src_path)?;
    let mut n = 0;
    for (i, path) in paths.iter().enumerate() {
        if i == b.style_source {
            continue;
        }
        writable_chapter(s, path, "book.syncStyles")?;
        let mut d = load(path)?;
        sync_chapter_styles(&mut d, &src, &opts);
        sync_chapter_extras(&mut d, &src, &opts);
        batch.stage(path, &d)?;
        n += 1;
    }
    batch.commit()?;
    let _ = save(&b);
    s.book = Some(b);
    Ok(json!({"synced": n}))
}

#[cfg(target_arch = "wasm32")]
fn paginate(_: &mut Session, _: &Value) -> Result<Value> {
    Err(EngineError::Other("books need a file system".into()))
}

#[cfg(target_arch = "wasm32")]
fn sync_styles(_: &mut Session, _: &Value) -> Result<Value> {
    Err(EngineError::Other("books need a file system".into()))
}

#[cfg(all(test, not(target_arch = "wasm32")))]
#[path = "book/safety_tests.rs"]
mod safety_tests;

/// Make page numbering of `d` start at `n` (the first section).
#[cfg(not(target_arch = "wasm32"))]
fn start_at(d: &mut designcraft_doc::Document, n: u32) {
    if !d.sections.iter().any(|x| x.start == 0) {
        d.sections.insert(
            0,
            designcraft_doc::Section {
                start: 0,
                start_number: None,
                style: Default::default(),
                prefix: String::new(),
                marker: String::new(),
                include_prefix: false,
                layout: None,
            },
        );
    }
    if let Some(first) = d.sections.iter_mut().find(|x| x.start == 0) {
        first.start_number = Some(n.max(1));
    }
}

fn preflight_book(s: &mut Session, p: &Value) -> Result<Value> {
    let b = s.book.as_ref().ok_or_else(no_book)?;
    let mut prof = match str_param(p, "profile") {
        Some(name) => {
            super::preflight::resolve_profile(s, Some(name)).ok_or_else(|| bad("book.preflight", format!("unknown preflight profile `{name}`")))?
        }
        None => super::preflight::resolve_profile(s, None).unwrap_or_default(),
    };
    if let Some(min) = p.get("minPpi").and_then(Value::as_f64) {
        prof.min_ppi = min;
    }
    let mut total_errors = 0usize;
    let mut total_warnings = 0usize;
    let mut chapter_reports = Vec::new();
    for path in &chapters(b)? {
        let mut d = load(path)?;
        let _fonts = super::file::load_document_fonts(&mut d, path).0;
        let cache = designcraft_compose::Cache::new();
        let issues = super::preflight::check_document_with_profile(&d, &cache, Some(std::path::Path::new(path)), &prof);
        let errors = issues.iter().filter(|i| i.severity == "error").count();
        let warnings = issues.len().saturating_sub(errors);
        total_errors += errors;
        total_warnings += warnings;
        chapter_reports.push(json!({
            "path": path,
            "errors": errors,
            "warnings": warnings,
            "issues": issues,
        }));
    }
    Ok(json!({
        "profile": prof.name,
        "errors": total_errors,
        "warnings": total_warnings,
        "chapters": chapter_reports,
    }))
}

fn export_book_pdf(s: &mut Session, p: &Value) -> Result<Value> {
    let b = s.book.clone().ok_or_else(no_book)?;
    let mut parts = Vec::new();
    let mut warnings = Vec::new();
    for path in &chapters(&b)? {
        let mut d = load(path)?;
        let _fonts = super::file::load_document_fonts(&mut d, path).0;
        let cache = designcraft_compose::Cache::new();
        let opts = super::export::options_with_session(Some(s), p, d.page_count())?;
        let r = designcraft_pdf::export_pdf_with_report(&super::interchange::with_pdf_boxes(&d), &cache, &opts)
            .map_err(|e| EngineError::Other(e.to_string()))?;
        warnings.extend(r.warnings);
        let sizes: Vec<(f32, f32)> =
            designcraft_render::pdf_page_sizes(&r.bytes).unwrap_or_default().into_iter().map(|(w, h)| (w as f32, h as f32)).collect();
        parts.push((r.bytes, sizes));
    }
    let pages: usize = parts.iter().map(|pt| pt.1.len()).sum();
    let bytes = designcraft_pdf::merge_pdfs(&parts, None).map_err(|e| EngineError::Other(e.to_string()))?;
    match str_param(p, "path") {
        Some(path) => {
            #[cfg(not(target_arch = "wasm32"))]
            crate::io::write_file(path, &bytes).map_err(EngineError::Other)?;
            Ok(json!({"path": path, "bytes": bytes.len(), "pages": pages, "warnings": warnings}))
        }
        None => Ok(json!({"base64": super::file::base64_encode(&bytes), "bytes": bytes.len(), "pages": pages, "warnings": warnings})),
    }
}

fn export_book_epub(s: &mut Session, p: &Value) -> Result<Value> {
    let b = s.book.clone().ok_or_else(no_book)?;
    let chapter_paths = chapters(&b)?;
    if chapter_paths.is_empty() {
        return Err(bad("book.exportEpub", "the book has no documents"));
    }
    let fixed = p.get("fixedLayout").and_then(Value::as_bool).unwrap_or(false);
    let embed_fonts = p.get("embedFonts").and_then(Value::as_bool).unwrap_or(true);
    let book_title = str_param(p, "title")
        .map(str::to_string)
        .unwrap_or_else(|| std::path::Path::new(&b.path).file_stem().and_then(std::ffi::OsStr::to_str).unwrap_or("Book").to_string());
    let mut opts = designcraft_epub::EpubOptions {
        title: Some(book_title),
        author: str_param(p, "author").map(str::to_string),
        language: str_param(p, "language").unwrap_or("en").to_string(),
        identifier: None,
        modified: None,
        cover: None,
        toc_title: None,
        generic_families: std::collections::BTreeMap::new(),
        fonts: Vec::new(),
        split: Some(designcraft_doc::EpubSplit::Off),
    };

    let (bytes, count, warnings) = if fixed {
        let mut docs_and_caches = Vec::new();
        let mut total_pages = 0usize;
        for path in &chapter_paths {
            let mut d = load(path)?;
            let _fonts = super::file::load_document_fonts(&mut d, path).0;
            total_pages += d.page_count();
            docs_and_caches.push((d, designcraft_compose::Cache::new()));
        }
        let mut scenes = Vec::with_capacity(total_pages);
        let mut all_fonts = Vec::new();
        let mut seen_fonts = HashSet::new();
        let mut warnings = Vec::new();
        let mut page_offset = 0usize;
        for (d, cache) in &docs_and_caches {
            let mut builder = super::export::fixed_epub::FixedSceneBuilder::new(d, cache, &mut warnings, page_offset, total_pages);
            scenes.extend(builder.build_all_pages());
            if embed_fonts {
                for f in builder.collect_embedded_fonts() {
                    if seen_fonts.insert((f.family.clone(), f.style.clone())) {
                        all_fonts.push(f);
                    }
                }
            }
            page_offset += d.page_count();
        }
        opts.fonts = all_fonts;
        let bytes =
            designcraft_epub::export_fixed_epub_scene(&docs_and_caches[0].0, &scenes, &opts).map_err(|e| EngineError::Other(e.to_string()))?;
        (bytes, scenes.len(), warnings)
    } else {
        // Merge chapters into a combined reflowable document.
        let mut first_doc = load(&chapter_paths[0])?;
        let _fonts = super::file::load_document_fonts(&mut first_doc, &chapter_paths[0]).0;
        for path in chapter_paths.iter().skip(1) {
            let mut next_doc = load(path)?;
            let _fonts = super::file::load_document_fonts(&mut next_doc, path).0;
            for ps in &next_doc.styles.paragraph {
                if first_doc.styles.para(&ps.name).is_none() {
                    first_doc.styles_mut().paragraph.push(ps.clone());
                }
            }
            for cs in &next_doc.styles.character {
                if first_doc.styles.char_style(&cs.name).is_none() {
                    first_doc.styles_mut().character.push(cs.clone());
                }
            }
            let ids: Vec<_> = next_doc.spreads.iter().flat_map(|sp| sp.items.iter().map(|i| i.id)).collect();
            let after = first_doc.page_count().checked_sub(1);
            let _ = first_doc.insert_pages(after, 1, None);
            let dst_spread = designcraft_doc::SpreadRef::Doc(first_doc.spreads.len().saturating_sub(1));
            let _ = super::object::duplicate_from(&mut first_doc, &next_doc, &ids, dst_spread, designcraft_geom::Vec2::ZERO);
        }
        let bytes = designcraft_epub::export_epub(&first_doc, &opts).map_err(|e| EngineError::Other(e.to_string()))?;
        (bytes, chapter_paths.len(), Vec::<String>::new())
    };

    match str_param(p, "path") {
        Some(path) => {
            #[cfg(not(target_arch = "wasm32"))]
            crate::io::write_file(path, &bytes).map_err(EngineError::Other)?;
            Ok(json!({"path": path, "bytes": bytes.len(), "chapters": count, "warnings": warnings}))
        }
        None => Ok(json!({"base64": super::file::base64_encode(&bytes), "bytes": bytes.len(), "chapters": count, "warnings": warnings})),
    }
}

pub fn specs() -> Vec<CommandSpec> {
    vec![
        cmd!(noundo "book.new", "New Book", ["File", "New"], None, "{path} — an empty book file (.indb or .dcbook)", always, |s, p| {
            let path = str_param(p, "path").ok_or_else(|| bad("book.new", "`path` required"))?.to_string();
            let b = Book { path, ..Default::default() };
            save(&b)?;
            s.book = Some(b);
            ok()
        }),
        cmd!(noundo "book.open", "Open Book", [], None, "{path} → {documents, format}", always, |s, p| {
            let path = str_param(p, "path").ok_or_else(|| bad("book.open", "`path` required"))?.to_string();
            #[cfg(not(target_arch = "wasm32"))]
            let bytes = crate::io::read_file(&path).map_err(EngineError::Other)?;
            #[cfg(target_arch = "wasm32")]
            let bytes: Vec<u8> = Vec::new();
            let mut b = decode_book_bytes(&bytes).map_err(|e| bad("book.open", format!("not a book: {e}")))?;
            let fmt = if is_indb_path(&path) { "indb" } else { "dcbook" };
            b.path = path;
            let n = b.documents.len();
            s.book = Some(b);
            Ok(json!({"documents": n, "format": fmt}))
        }),
        cmd!(noundo "book.save", "Save Book", [], None, "{}", has_book, |s, _| {
            let b = s.book.as_ref().ok_or_else(no_book)?;
            save(b)?;
            ok()
        }),
        cmd!(noundo "book.saveAs", "Save Book As…", [], None, "{path}", has_book, |s, p| {
            let path = str_param(p, "path").ok_or_else(|| bad("book.saveAs", "`path` required"))?.to_string();
            let mut b = s.book.as_ref().ok_or_else(no_book)?.clone();
            b.path = path;
            save(&b)?;
            s.book = Some(b);
            ok()
        }),
        cmd!(noundo "book.close", "Close Book", [], None, "{}", has_book, |s, _| {
            s.book = None;
            ok()
        }),
        cmd!(noundo "book.add", "Add Document", [], None, "{path, at?: index}", has_book, |s, p| {
            let path = str_param(p, "path").ok_or_else(|| bad("book.add", "`path` required"))?.to_string();
            load(&path)?;
            let mut b = s.book.as_ref().ok_or_else(no_book)?.clone();
            let at = p.get("at").and_then(Value::as_u64).map_or(b.documents.len(), |i| (i as usize).min(b.documents.len()));
            b.documents.insert(at, path);
            save(&b)?;
            s.book = Some(b);
            ok()
        }),
        cmd!(noundo "book.remove", "Remove Document", [], None, "{index}", has_book, |s, p| {
            let i = p.get("index").and_then(Value::as_u64).ok_or_else(|| bad("book.remove", "`index` required"))? as usize;
            let mut b = s.book.as_ref().ok_or_else(no_book)?.clone();
            if i >= b.documents.len() {
                return Err(bad("book.remove", format!("no document {i}")));
            }
            b.documents.remove(i);
            if b.style_source >= b.documents.len() {
                b.style_source = 0;
            }
            save(&b)?;
            s.book = Some(b);
            ok()
        }),
        cmd!(noundo "book.move", "Reorder Book Document", [], None, "{from, to}", has_book, |s, p| {
            let from = p.get("from").and_then(Value::as_u64).ok_or_else(|| bad("book.move", "`from` required"))? as usize;
            let to = p.get("to").and_then(Value::as_u64).ok_or_else(|| bad("book.move", "`to` required"))? as usize;
            let mut b = s.book.as_ref().ok_or_else(no_book)?.clone();
            if from >= b.documents.len() || to >= b.documents.len() {
                return Err(bad("book.move", "document index out of bounds"));
            }
            if from != to {
                let style_doc = b.documents.get(b.style_source).cloned();
                let item = b.documents.remove(from);
                b.documents.insert(to, item);
                if let Some(sd) = style_doc
                    && let Some(pos) = b.documents.iter().position(|x| *x == sd)
                {
                    b.style_source = pos;
                }
                save(&b)?;
                s.book = Some(b);
            }
            ok()
        }),
        cmd!(noundo "book.styleSource", "Style Source", [], None, "{index}", has_book, |s, p| {
            let i = p.get("index").and_then(Value::as_u64).unwrap_or(0) as usize;
            let mut b = s.book.as_ref().ok_or_else(no_book)?.clone();
            if i >= b.documents.len() {
                return Err(bad("book.styleSource", format!("no document {i}")));
            }
            b.style_source = i;
            save(&b)?;
            s.book = Some(b);
            ok()
        }),
        cmd!(noundo "book.options", "Book Numbering & Sync Options", [], None,
        "{pageNumbering?: \"continue\"|\"nextOdd\"|\"nextEven\", insertBlankPage?: bool, continueLists?: bool, paragraphStyles?: bool, characterStyles?: bool, objectStyles?: bool, tableStyles?: bool, cellStyles?: bool, swatches?: bool, parentPages?: bool, numberedLists?: bool, textVariables?: bool, conditionalText?: bool}",
        has_book, |s, p| {
            let mut b = s.book.as_ref().ok_or_else(no_book)?.clone();
            if let Some(m) = str_param(p, "pageNumbering") {
                b.page_numbering = PageNumberingMode::parse(m)
                    .ok_or_else(|| bad("book.options", format!("unknown pageNumbering `{m}`")))?;
            }
            if let Some(ib) = p.get("insertBlankPage").and_then(Value::as_bool) {
                b.insert_blank_page = ib;
            }
            if let Some(cl) = p.get("continueLists").and_then(Value::as_bool) {
                b.continue_lists = cl;
            }
            b.sync_options.apply_json(p);
            save(&b)?;
            s.book = Some(b);
            ok()
        }),
        cmd!(query "book.list", "Book", [], None, "{} → {path, styleSource, pageNumbering, insertBlankPage, continueLists, syncOptions, documents: [{path, pages, firstPage}]}", has_book, |s, _| {
            let b = s.book.as_ref().ok_or_else(no_book)?;
            let mut first = 1u32;
            let mut docs = Vec::new();
            for (idx, p) in b.documents.iter().enumerate() {
                if idx > 0 {
                    first = b.page_numbering.advance(first).0;
                }
                let pages = chapter(b, p).and_then(|cp| load(&cp)).map(|d| d.page_count()).unwrap_or(0);
                docs.push(json!({"path": p, "pages": pages, "firstPage": first}));
                first = first.saturating_add(pages as u32);
            }
            Ok(json!({
                "path": b.path,
                "styleSource": b.style_source,
                "pageNumbering": b.page_numbering.as_str(),
                "insertBlankPage": b.insert_blank_page,
                "continueLists": b.continue_lists,
                "syncOptions": b.sync_options,
                "documents": docs,
            }))
        }),
        cmd!(noundo "book.paginate", "Update Numbering", [], None, "{mode?: \"continue\"|\"nextOdd\"|\"nextEven\", insertBlankPage?: bool, updateLists?: bool} — each document starts numbering according to book page numbering options and updates chapter numbers → {pages, chapters, blankPagesInserted}", has_book, |s, p| {
            paginate(s, p)
        }),
        cmd!(noundo "book.syncStyles", "Synchronize Book", [], None, "{paragraphStyles?: bool, characterStyles?: bool, objectStyles?: bool, tableStyles?: bool, cellStyles?: bool, swatches?: bool, parentPages?: bool, numberedLists?: bool, textVariables?: bool, conditionalText?: bool} — synchronize selected style and resource categories from the style source → {synced}", has_book, |s, p| {
            sync_styles(s, p)
        }),
        cmd!(query "book.preflight", "Preflight Book", [], None, "{profile?: str, minPpi?: 150} → {profile, errors, warnings, chapters}", has_book, preflight_book),
        cmd!(noundo "book.exportPdf", "Export Book to PDF…", [], None, "{path?, preset?, joboptionsPath?, standard?, pages?, spreads?, bleed?, marks?, bookmarks?, hyperlinks?, tagged?, flattenPpi?, colorConversion?, destinationProfile?, downsamplingPpi?, downsamplingThresholdPpi?, imageCompression?, jpegQuality?} — every document in order as one PDF → {path, bytes, pages, warnings}", has_book, export_book_pdf),
        cmd!(noundo "book.exportEpub", "Export Book to EPUB…", [], None, "{path?, fixedLayout?: bool, title?, author?, language?, embedFonts?: bool} — export the book as Reflowable or Fixed-Layout EPUB 3.0 → {path, bytes, chapters, warnings}", has_book, export_book_epub),
    ]
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use crate::Session;

    #[test]
    fn book_numbering_resolves_chapter_numbers_in_order() {
        let dir = std::env::temp_dir().join(format!("dc-book-chapters-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let p = |n: &str| dir.join(n).to_string_lossy().to_string();
        let mut s = Session::new();
        s.execute("book.new", &json!({"path": p("Field Guide.dcbook")})).unwrap();
        let chapters = [
            ("a.designcraft", json!({"style": "upperRoman"})),
            ("b.designcraft", json!({"source": "sameAsPrevious"})),
            ("c.designcraft", json!({})),
            ("d.designcraft", json!({"start": 7, "style": "lowerLetters"})),
            ("e.designcraft", json!({})),
        ];
        for (name, numbering) in &chapters {
            s.execute("file.new", &json!({})).unwrap();
            if numbering.as_object().is_some_and(|o| !o.is_empty()) {
                s.execute("layout.chapterNumbering", numbering).unwrap();
            }
            s.execute("file.saveAs", &json!({"path": p(name)})).unwrap();
            s.execute("book.add", &json!({"path": p(name)})).unwrap();
            s.execute("file.close", &json!({})).unwrap();
        }
        let r = s.execute("book.paginate", &json!({})).unwrap();
        assert_eq!(r["chapters"], json!(["I", "1", "2", "g", "8"]));
        assert_eq!(super::load(&p("e.designcraft")).unwrap().settings.chapter_number, 8);
        s.execute("file.open", &json!({"path": p("e.designcraft")})).unwrap();
        assert_eq!(s.execute("layout.chapterNumbering", &json!({})).unwrap()["book"], "Field Guide");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn book_paginates_syncs_and_exports() {
        let dir = std::env::temp_dir().join(format!("dc-book-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let p = |n: &str| dir.join(n).to_string_lossy().to_string();
        let mut s = Session::new();
        for (name, pages, style) in [("a.designcraft", 3, "Chapter Head"), ("b.designcraft", 2, "Other")] {
            s.execute("file.new", &json!({"pages": pages})).unwrap();
            s.execute("style.paragraph.create", &json!({"name": style, "chars": {"size": 20}})).unwrap();
            s.execute("file.saveAs", &json!({"path": p(name)})).unwrap();
            s.execute("file.close", &json!({})).unwrap();
        }
        s.execute("book.new", &json!({"path": p("book.dcbook")})).unwrap();
        s.execute("book.add", &json!({"path": p("a.designcraft")})).unwrap();
        s.execute("book.add", &json!({"path": p("b.designcraft")})).unwrap();
        let l = s.execute("book.list", &json!({})).unwrap();
        assert_eq!(l["documents"][1]["firstPage"], 4);
        assert_eq!(s.execute("book.paginate", &json!({})).unwrap()["pages"], 5);
        let b = super::load(&p("b.designcraft")).unwrap();
        assert_eq!(b.page_name(0), "4", "continues after the first document");
        assert_eq!(s.execute("book.syncStyles", &json!({})).unwrap()["synced"], 1);
        assert!(super::load(&p("b.designcraft")).unwrap().styles.para("Chapter Head").is_some());
        let r = s.execute("book.exportPdf", &json!({})).unwrap();
        assert_eq!(r["pages"], 5);
        let bytes = super::super::file::base64_decode(r["base64"].as_str().unwrap());
        assert_eq!(designcraft_render::pdf_page_count(&bytes), Some(5));
        let mut t = Session::new();
        assert_eq!(t.execute("book.open", &json!({"path": p("book.dcbook")})).unwrap()["documents"], 2);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn indb_round_trip_next_odd_blank_page_sync_options_and_fixed_epub() {
        let dir = std::env::temp_dir().join(format!("dc-indb-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let p = |n: &str| dir.join(n).to_string_lossy().to_string();
        let mut s = Session::new();
        // Chapter 1: 1 page (ends on page 1 -> next page is 2, so NextOdd advances to 3 and inserts 1 blank page!).
        s.execute("file.new", &json!({"pages": 1})).unwrap();
        s.execute("style.object.create", &json!({"name": "HeroCard"})).unwrap();
        s.execute("style.paragraph.create", &json!({"name": "SkipMe", "chars": {"size": 18}})).unwrap();
        s.execute("frame.create", &json!({"rect": [72, 72, 240, 140], "content": "text", "text": "Chapter One", "caret": false})).unwrap();
        s.execute("file.saveAs", &json!({"path": p("ch1.designcraft")})).unwrap();
        s.execute("file.close", &json!({})).unwrap();

        // Chapter 2: 2 pages.
        s.execute("file.new", &json!({"pages": 2})).unwrap();
        s.execute("frame.create", &json!({"rect": [72, 72, 240, 140], "content": "text", "text": "Chapter Two", "caret": false})).unwrap();
        s.execute("file.saveAs", &json!({"path": p("ch2.designcraft")})).unwrap();
        s.execute("file.close", &json!({})).unwrap();

        s.execute("book.new", &json!({"path": p("Anthology.indb")})).unwrap();
        s.execute("book.add", &json!({"path": p("ch1.designcraft")})).unwrap();
        s.execute("book.add", &json!({"path": p("ch2.designcraft")})).unwrap();

        // Selective sync: objectStyles = true, paragraphStyles = false.
        s.execute("book.syncStyles", &json!({"paragraphStyles": false, "objectStyles": true})).unwrap();
        let ch2_after_sync = super::load(&p("ch2.designcraft")).unwrap();
        assert!(ch2_after_sync.styles.object.iter().any(|o| o.name == "HeroCard"));
        assert!(ch2_after_sync.styles.para("SkipMe").is_none());

        // Paginate on NextOdd with Insert Blank Page.
        let pag = s.execute("book.paginate", &json!({"mode": "nextOdd", "insertBlankPage": true})).unwrap();
        assert_eq!(pag["blankPagesInserted"], 1);
        assert_eq!(pag["pages"], 4);
        assert_eq!(super::load(&p("ch1.designcraft")).unwrap().page_count(), 2);
        assert_eq!(super::load(&p("ch2.designcraft")).unwrap().page_name(0), "3");

        // Preflight Book + Fixed-Layout EPUB export.
        let pf = s.execute("book.preflight", &json!({})).unwrap();
        assert_eq!(pf["chapters"].as_array().unwrap().len(), 2);
        let epub = s.execute("book.exportEpub", &json!({"fixedLayout": true, "title": "Anthology"})).unwrap();
        assert_eq!(epub["chapters"], 4);

        // Reopen `.indb` in a fresh session and reorder chapters.
        let mut t = Session::new();
        let opened = t.execute("book.open", &json!({"path": p("Anthology.indb")})).unwrap();
        assert_eq!(opened["documents"], 2);
        assert_eq!(opened["format"], "indb");
        t.execute("book.move", &json!({"from": 0, "to": 1})).unwrap();
        assert_eq!(t.execute("book.list", &json!({})).unwrap()["styleSource"], 1);

        std::fs::remove_dir_all(&dir).ok();
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn a_source_style_defined_twice_keeps_its_last_definition() {
        let list = [("a", 1), ("b", 2), ("a", 3), ("c", 4)];
        let kept: Vec<_> = super::keep_last(&list, |x| x.0).into_iter().copied().collect();
        assert_eq!(kept, [("b", 2), ("a", 3), ("c", 4)]);
    }
}
