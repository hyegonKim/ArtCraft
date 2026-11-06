//! Smart Text Reflow triggers compare composition data, not allocation/cache identity.
//! Explicit page deletion is handled by the caller: it keeps text as overset until a
//! subsequent composition input changes. No serialized or sticky suppression state is needed.

use designcraft_doc::{Document, Story, StoryId};

/// Story revision and link/editorial metadata are not layout inputs. A command may replace
/// an Arc or bump a cache revision while leaving every text/layout value unchanged.
fn same_story(a: &Story, b: &Story) -> bool {
    a.text == b.text
        && a.paras == b.paras
        && a.chars == b.chars
        && a.frames == b.frames
        && a.direction == b.direction
        && a.vertical == b.vertical
        && a.optical_margin == b.optical_margin
        && a.tables == b.tables
        && a.notes == b.notes
        && a.endnotes == b.endnotes
        && a.anchors == b.anchors
        && a.xrefs == b.xrefs
        && a.index_refs == b.index_refs
        && a.objects == b.objects
}

fn story_changed(a: &Document, b: &Document, sid: StoryId) -> bool {
    match (a.story(sid), b.story(sid)) {
        (Some(old), Some(new)) => !same_story(old, new) || styles_changed(a, b, new),
        (None, None) => false,
        _ => true,
    }
}

/// Resolve only typography the primary story actually names. Creating an unused paragraph,
/// character or object style is not a text edit and must not resurrect a deleted page.
fn styles_changed(a: &Document, b: &Document, story: &Story) -> bool {
    if a.styles == b.styles {
        return false;
    }
    if a.styles.composite_fonts != b.styles.composite_fonts || a.styles.mojikumi_tables != b.styles.mojikumi_tables {
        return true;
    }
    for para in &story.paras {
        let (old_para, old_chars) = a.styles.resolve_para(para);
        let (new_para, new_chars) = b.styles.resolve_para(para);
        if old_para != new_para || old_chars != new_chars {
            return true;
        }
        let mut names = [new_para.drop_cap_style.as_str(), new_para.bullet_char_style.as_str(), new_para.number_char_style.as_str()]
            .into_iter()
            .chain(new_para.nested_styles.iter().map(|s| s.style.as_str()))
            .chain(new_para.grep_styles.iter().map(|s| s.style.as_str()))
            .chain(new_para.nested_line_styles.iter().map(|s| s.style.as_str()));
        if names.any(|name| a.styles.char_style_attrs(name) != b.styles.char_style_attrs(name)) {
            return true;
        }
    }
    let names: std::collections::BTreeSet<_> = story.chars.iter().map(|run| run.format.style.as_str()).collect();
    if names.into_iter().any(|name| a.styles.char_style_attrs(name) != b.styles.char_style_attrs(name)) {
        return true;
    }
    // Table/note content has its own paragraph and character style references. Keep those
    // catalogs as dependencies when present, rather than missing a nested typography edit.
    (!story.tables.is_empty() || !story.notes.is_empty() || !story.endnotes.is_empty())
        && (a.styles.paragraph != b.styles.paragraph
            || a.styles.character != b.styles.character
            || a.styles.cell != b.styles.cell
            || a.styles.table != b.styles.table)
}

pub(super) fn inputs_changed(a: &Document, b: &Document) -> bool {
    let Some(sid) = b.settings.primary_story else { return false };
    let Some(story) = b.story(sid) else { return false };
    if a.settings.primary_story != Some(sid)
        || story_changed(a, b, sid)
        || a.font_scope != b.font_scope
        || a.conditions != b.conditions
        || a.hyphenation_exceptions != b.hyphenation_exceptions
        || a.text_variables != b.text_variables
        || a.sections != b.sections
        || a.xref_formats != b.xref_formats
        || a.footnote_options != b.footnote_options
        || a.endnote_options != b.endnote_options
        || a.settings.advanced_type != b.settings.advanced_type
        || a.settings.glyph_fallback != b.settings.glyph_fallback
        || a.settings.lists != b.settings.lists
        || a.settings.chapter_number != b.settings.chapter_number
        || a.settings.chapter_style != b.settings.chapter_style
    {
        return true;
    }
    // The composer's own frame specifications include text area/options, transformed
    // outlines, page/grid/spine context, visible wrap geometry and clipping contours.
    // A frame or background fill alone is absent from these inputs and does not reflow.
    if designcraft_compose::frame_specs(a, sid) != designcraft_compose::frame_specs(b, sid) {
        return true;
    }
    let old_neighbours = designcraft_compose::wrap::neighbours(a, sid);
    let neighbours = designcraft_compose::wrap::neighbours(b, sid);
    if old_neighbours != neighbours
        || neighbours.iter().any(|n| story_changed(a, b, *n) || designcraft_compose::frame_specs(a, *n) != designcraft_compose::frame_specs(b, *n))
    {
        return true;
    }
    // Running headers, cross-references and continuing note/list numbering can depend on
    // other stories. Ordinary primary text does not depend on unrelated story edits.
    let other_stories = designcraft_compose::vars::has_running(b, story)
        || designcraft_compose::xref::has_xrefs(story)
        || !story.notes.is_empty()
        || !story.endnotes.is_empty()
        || !b.settings.lists.is_empty();
    if other_stories
        && (a.stories.len() != b.stories.len()
            || a.stories.keys().any(|n| {
                *n != sid && (story_changed(a, b, *n) || designcraft_compose::frame_specs(a, *n) != designcraft_compose::frame_specs(b, *n))
            }))
    {
        return true;
    }
    designcraft_compose::vars::has_vars(story)
        && (a.page_count() != b.page_count()
            || a.title != b.title
            || a.file_info != b.file_info
            || a.created != b.created
            || a.modified != b.modified)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn equal_data_with_new_allocations_and_revisions_does_not_trigger_reflow() {
        let a = Document::new(&designcraft_doc::build::NewDocument { primary_text_frame: true, ..Default::default() });
        let mut b = a.clone();
        let sid = b.settings.primary_story.expect("primary story");
        let story = b.story_mut(sid).expect("story");
        story.rev = story.rev.wrapping_add(1);
        b.styles = std::sync::Arc::new((*b.styles).clone());
        assert!(!inputs_changed(&a, &b), "cache identity/revision alone is not a layout change");
    }
}
