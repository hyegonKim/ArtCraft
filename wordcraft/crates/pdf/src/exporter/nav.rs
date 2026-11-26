use super::{
    Annotation, ClKind, DocPath, Exporter, Fit, HashMap, HashSet, LineEnd, LinkAnnotation, Outline, OutlineNode, Placed, Point, StoryRef, Target,
    XyzDestination, krilla, write,
};

/// A node of the PDF outline (`/Outlines`) made from the document's headings and/or bookmarks.
#[derive(Clone, Debug, PartialEq)]
pub struct OutlineEntry {
    pub title: String,
    pub level: u8,
    pub page: usize,
    pub x: f32,
    pub y: f32,
    pub children: Vec<OutlineEntry>,
}

impl Exporter<'_> {
    /// Where a bookmark is: (output page, x, y) — the page and line its start lands on, which
    /// in a paragraph split across pages need not be the paragraph's first page.
    pub(crate) fn bookmark(&self, name: &str) -> Option<(usize, f32, f32)> {
        let (exact, folded) = self.bookmarks.get_or_init(|| {
            let (mut exact, mut folded) = (HashMap::new(), HashMap::new());
            for (n, pos) in self.doc.bookmarks() {
                folded.entry(n.to_ascii_lowercase()).or_insert_with(|| pos.clone());
                exact.entry(n).or_insert(pos);
            }
            (exact, folded)
        });
        let pos = exact.get(name).or_else(|| folded.get(&name.to_ascii_lowercase()))?.clone();
        if let Some(c) = self.lay.caret(&pos)
            && let Some((out, fit)) = self.out_index.get(&c.page)
        {
            // The left edge of the paragraph's piece on that page, at the top of the line.
            let x = self
                .lay
                .index
                .get(&(pos.story, pos.path.clone()))
                .into_iter()
                .flatten()
                .filter(|(pi, _)| *pi == c.page)
                .find_map(|(pi, ii)| match self.lay.pages.get(*pi).and_then(|p| p.items.get(*ii)) {
                    Some(Placed::Lines { x, .. }) => Some(*x),
                    _ => None,
                })
                .unwrap_or(c.x);
            let (x, y) = fit.point(x, c.top);
            return Some((*out, x, y));
        }
        self.locate(pos.story, &pos.path)
    }

    /// The first output page showing a paragraph, with the top of its first line.
    fn locate(&self, story: StoryRef, path: &DocPath) -> Option<(usize, f32, f32)> {
        let places = self.lay.index.get(&(story, path.clone()))?;
        for (pi, ii) in places {
            let Some((out, fit)) = self.out_index.get(pi) else { continue };
            let Some(page) = self.lay.pages.get(*pi) else { continue };
            let (x, y) = match page.items.get(*ii) {
                Some(Placed::Lines { x, y, .. }) => fit.point(*x, *y),
                _ => (0.0, 0.0),
            };
            return Some((*out, x, y));
        }
        None
    }

    pub(crate) fn annotations(&self, fit: Fit) -> Vec<Annotation> {
        let mut v = Vec::new();
        for (x0, y0, x1, y1, target) in &self.links {
            let ((x0, y0), (x1, y1)) = (fit.point(*x0, *y0), fit.point(*x1, *y1));
            let Some(rect) = krilla::geom::Rect::from_ltrb(x0, y0, x1, y1) else { continue };
            let t = if let Some(name) = target.strip_prefix('#') {
                match self.bookmark(name) {
                    Some((page, x, y)) => Target::Destination(XyzDestination::new(page, Point::from_xy(x, y)).into()),
                    None => continue,
                }
            } else {
                // Unsafe targets (javascript:, file:, shares, other schemes) get no link; the text stays.
                match write::uri_target(target) {
                    Some(t) => t,
                    None => continue,
                }
            };
            v.push(Annotation::new_link(LinkAnnotation::new(rect, t), Some(target.clone())));
        }
        v
    }

    /// The outline entries from body headings (outline levels 1–9) and/or the document's bookmarks
    /// on the exported pages, in document order.
    pub(crate) fn outline_entries(&self, pages: &[usize]) -> Vec<OutlineEntry> {
        // (output page, outside the body, path, offset): the order the document reads in.
        type Key = (usize, bool, DocPath, usize);
        // A `None` level is a bookmark: it nests under the heading before it.
        let mut found: Vec<(Key, Option<u8>, String, usize, f32, f32)> = Vec::new();
        if self.opts.heading_bookmarks {
            for pi in pages {
                let Some((out, fit)) = self.out_index.get(pi) else { continue };
                let Some(page) = self.lay.pages.get(*pi) else { continue };
                for it in &page.items {
                    let Placed::Lines { story: StoryRef::Body, path, para, l0, l1, x, y, .. } = it else { continue };
                    let Some(p) = self.doc.para(StoryRef::Body, path) else { continue };
                    let Some(level) = self.doc.styles.resolve_para(&p.props).outline_level else { continue };
                    // A leading page break can be placed on the previous page without the heading.
                    let first = para
                        .lines
                        .iter()
                        .take_while(|line| {
                            line.end == LineEnd::PageBreak
                                && para
                                    .clusters
                                    .get(line.c0..line.c1)
                                    .is_some_and(|cs| cs.iter().all(|c| matches!(c.kind, ClKind::PageBreak | ClKind::Marker)))
                        })
                        .count();
                    if !(*l0..*l1).contains(&first) {
                        continue;
                    }
                    let Some(line) = para.lines.get(first) else { continue };
                    let Some(piece) = para.lines.get(*l0) else { continue };
                    let title: String = self.title_text(p, para).split_whitespace().collect::<Vec<_>>().join(" ").chars().take(200).collect();
                    if title.is_empty() {
                        continue;
                    }
                    let (x, y) = fit.point(*x, *y + line.top - piece.top);
                    found.push(((*out, false, path.clone(), 0), Some(level.min(8)), title, *out, x, y));
                }
            }
        }
        if self.opts.word_bookmarks {
            // A name's first bookmark is the one links go to, so it is the one listed.
            let mut seen = HashSet::new();
            for (name, pos) in self.doc.bookmarks() {
                if name.starts_with('_') || !seen.insert(name.clone()) {
                    continue;
                }
                let title: String = name.split_whitespace().collect::<Vec<_>>().join(" ").chars().take(200).collect();
                if title.is_empty() {
                    continue;
                }
                if let Some((out, x, y)) = self.bookmark(&name) {
                    found.push(((out, pos.story != StoryRef::Body, pos.path, pos.off), None, title, out, x, y));
                }
            }
        }
        if found.is_empty() {
            return Vec::new();
        }
        // Stable, so a heading stays before a bookmark at its start.
        found.sort_by(|(a, ..), (b, ..)| a.cmp(b));
        let mut heading: Option<u8> = None;
        let flat: Vec<(u8, String, usize, f32, f32)> = found
            .into_iter()
            .map(|(_, level, title, out, x, y)| {
                let level = match level {
                    Some(l) => {
                        heading = Some(l);
                        l
                    }
                    None => heading.map_or(0, |l| l.saturating_add(1).min(8)),
                };
                (level, title, out, x, y)
            })
            .collect();
        fn build(flat: &[(u8, String, usize, f32, f32)], i: &mut usize, level: u8) -> Vec<OutlineEntry> {
            let mut out = Vec::new();
            while let Some((l, title, page, x, y)) = flat.get(*i) {
                if *l < level {
                    break;
                }
                *i += 1;
                let children = if flat.get(*i).is_some_and(|n| n.0 > *l) { build(flat, i, l + 1) } else { Vec::new() };
                out.push(OutlineEntry { title: title.clone(), level: *l, page: *page, x: *x, y: *y, children });
            }
            out
        }
        let mut entries = Vec::new();
        let mut i = 0;
        while i < flat.len() {
            let before = i;
            entries.extend(build(&flat, &mut i, 0));
            if i == before {
                i += 1;
            }
        }
        entries
    }

    /// The outline from body headings (outline levels 1–9) and/or Word bookmarks on the exported pages.
    pub(crate) fn outline(&self, pages: &[usize]) -> Option<Outline> {
        let entries = self.outline_entries(pages);
        if entries.is_empty() {
            return None;
        }
        fn to_node(entry: OutlineEntry) -> OutlineNode {
            let mut node = OutlineNode::new(entry.title, XyzDestination::new(entry.page, Point::from_xy(entry.x, entry.y)));
            for c in entry.children {
                node.push_child(to_node(c));
            }
            node
        }
        let mut o = Outline::new();
        for e in entries {
            o.push_child(to_node(e));
        }
        Some(o)
    }
}
