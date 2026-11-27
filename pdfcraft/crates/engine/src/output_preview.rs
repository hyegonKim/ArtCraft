//! Print production ▸ Output preview: render a PDF page, inspect explicit `/DeviceCMYK`
//! operators, and compute CMYK channel separations + Total Area Coverage (`TAC %`).

use std::sync::Arc;

use pdfcraft_preflight::{CmykPlates, OutputPreviewResult, analyze_rgba_with_stream_info, inspect_page_cmyk_ink};
use pdfcraft_render::{PageRenderer, RenderConfig, RenderRequest};

use crate::{DocId, EditError, Session};

/// Default preview raster longest side (logical pixels) for interactive Output Preview.
pub const DEFAULT_PREVIEW_MAX_SIDE: u32 = 900;

impl Session {
    /// Render 0-based `page` of open document `id` and compute CMYK channel separations (`plates`)
    /// and Total Area Coverage (`TAC %`) against `ink_limit_percent` (e.g. `300.0` or `240.0`).
    pub fn output_preview(
        &self,
        id: DocId,
        page: usize,
        plates: CmykPlates,
        ink_limit_percent: f32,
        show_tac_heatmap: bool,
        max_side: u32,
    ) -> Result<OutputPreviewResult, EditError> {
        let doc = self.get(id).ok_or(EditError::NoDocument)?;
        let info = doc.info.pages.get(page).ok_or_else(|| EditError::Invalid(format!("there is no page {}", page.saturating_add(1))))?;

        let side_cap = max_side.clamp(64, 2400) as f32;
        let longest_pt = info.width.max(info.height).max(1.0);
        let scale = (side_cap / longest_pt).clamp(0.1, 4.0);

        let config = RenderConfig { password: doc.password.as_deref().map(Arc::from), ..Default::default() };
        let shot = PageRenderer::new(doc.bytes.clone(), config).render(RenderRequest { page, scale, ..Default::default() });
        if let Some(err) = shot.error {
            return Err(EditError::Invalid(format!("page {} could not be rendered: {err}", page + 1)));
        }

        let stream_info = doc.editor.as_ref().map(|ed| inspect_page_cmyk_ink(&ed.cos, page, ink_limit_percent)).unwrap_or_default();

        analyze_rgba_with_stream_info(&shot.rgba, shot.width, shot.height, plates, ink_limit_percent, show_tac_heatmap, stream_info)
            .map_err(EditError::Invalid)
    }
}
