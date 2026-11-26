//! Offline extractive document summarization with TF-IDF key terms and lexical-centrality ranking.
//!
//! Runs locally without network access or external AI providers, respecting the PDF's copying
//! permission bit (`Document::check_copying`).

use std::collections::{HashMap, HashSet};

use pdfcraft_render::{PageRenderer, text::TextFilter};
use unicode_segmentation::UnicodeSegmentation;

use crate::{Document, EditError, export::ExportSource};

/// Maximum candidate sentences compared pairwise for lexical centrality.
const MAX_CANDIDATES: usize = 360;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SummaryOptions {
    pub max_points: usize,
}

impl Default for SummaryOptions {
    fn default() -> Self {
        Self { max_points: 5 }
    }
}

/// One extractive key point tied to its 0-based source page.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SummaryPoint {
    /// Zero-based page index in the document.
    pub page: usize,
    pub text: String,
    /// Relative relevance score scaled to `0..=1000`.
    pub score_milli: u32,
}

impl SummaryPoint {
    pub fn page_number(&self) -> usize {
        self.page.saturating_add(1)
    }
}

/// Deterministic local summary of a document or page range.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DocumentSummary {
    pub page_count: usize,
    pub pages_without_text: usize,
    pub word_count: usize,
    pub reading_minutes: usize,
    pub key_terms: Vec<String>,
    pub points: Vec<SummaryPoint>,
}

impl DocumentSummary {
    pub fn to_markdown(&self, title: &str) -> String {
        let heading = if title.trim().is_empty() {
            "Document Summary"
        } else {
            title.trim()
        };
        let mut out = format!(
            "# {heading}\n\n- Pages analyzed: {}\n- Words: {}\n- Estimated reading time: {} min\n",
            self.page_count, self.word_count, self.reading_minutes
        );
        if !self.key_terms.is_empty() {
            out.push_str(&format!("- Key terms: {}\n", self.key_terms.join(", ")));
        }
        out.push('\n');
        if self.points.is_empty() {
            out.push_str("_No extractable sentences found. Scanned pages may require OCR first._\n");
            return out;
        }
        out.push_str("## Key Points\n\n");
        for pt in &self.points {
            out.push_str(&format!("- **(p. {})** {}\n", pt.page_number(), pt.text));
        }
        out
    }
}

/// Summarizes `pages` from `source`, checking `progress` between pages for cancellation.
pub fn summarize(
    source: ExportSource,
    pages: &[usize],
    options: SummaryOptions,
    progress: impl FnMut(usize) -> bool,
) -> Result<DocumentSummary, String> {
    crate::guard(|| summarize_inner(source, pages, options, progress))?
}

impl Document {
    /// Summarizes the document (or `pages`, 0-based) locally when text copying is permitted.
    pub fn summarize(
        &self,
        pages: Option<&[usize]>,
        max_points: usize,
    ) -> Result<DocumentSummary, EditError> {
        self.check_copying()?;
        let source = self.export_source();
        let all_pages: Vec<usize>;
        let target_pages = match pages {
            Some(list) => list,
            None => {
                all_pages = (0..source.pages).collect();
                &all_pages
            }
        };
        summarize(source, target_pages, SummaryOptions { max_points }, |_| true)
            .map_err(EditError::Invalid)
    }
}

fn summarize_inner(
    source: ExportSource,
    pages: &[usize],
    options: SummaryOptions,
    mut progress: impl FnMut(usize) -> bool,
) -> Result<DocumentSummary, String> {
    if pages.iter().any(|&p| p >= source.pages) {
        return Err("the summary page range is out of bounds".into());
    }
    if !progress(0) {
        return Err("summary cancelled".into());
    }
    let mut renderer = PageRenderer::new(source.bytes, source.config);
    let mut page_texts = Vec::new();
    let mut seen = HashSet::new();
    let mut word_count = 0usize;
    let mut pages_without_text = 0usize;
    for &page in pages {
        if !seen.insert(page) {
            continue;
        }
        if !progress(page_texts.len()) {
            return Err("summary cancelled".into());
        }
        let filter = TextFilter {
            tables: false,
            code: false,
            images: false,
            painted: false,
            mcids: HashSet::new(),
        };
        let text = renderer
            .filtered_text(page, filter)
            .map_err(|e| format!("page {}: {e}", page.saturating_add(1)))?
            .plain_text();
        let words = text.unicode_words().count();
        if words == 0 {
            pages_without_text += 1;
        }
        word_count = word_count.saturating_add(words);
        page_texts.push((page, text));
    }
    if !progress(page_texts.len()) {
        return Err("summary cancelled".into());
    }
    Ok(summarize_pages(&page_texts, word_count, pages_without_text, options.max_points))
}

#[derive(Clone)]
struct Candidate {
    page: usize,
    order: usize,
    page_lead: bool,
    doc_lead: bool,
    text: String,
    tf: HashMap<String, f64>,
}

pub(crate) fn summarize_pages(
    page_texts: &[(usize, String)],
    word_count: usize,
    pages_without_text: usize,
    max_points: usize,
) -> DocumentSummary {
    let max_k = max_points.clamp(1, 20);
    let reading_minutes = if word_count == 0 {
        0
    } else {
        word_count.div_ceil(200)
    };
    let (candidates, term_scores) = collect_candidates_and_terms(page_texts);
    let key_terms = top_terms(&term_scores, 8);
    let points = select_top_sentences(&candidates, &term_scores, max_k);
    DocumentSummary {
        page_count: page_texts.len(),
        pages_without_text,
        word_count,
        reading_minutes,
        key_terms,
        points,
    }
}

fn collect_candidates_and_terms(
    page_texts: &[(usize, String)],
) -> (Vec<Candidate>, HashMap<String, f64>) {
    let mut tf_total: HashMap<String, usize> = HashMap::new();
    let mut df_chunks: HashMap<String, usize> = HashMap::new();
    let mut candidates = Vec::new();
    let mut order = 0usize;

    for (page_idx, &(page, ref raw)) in page_texts.iter().enumerate() {
        let mut chunk_seen = HashSet::new();
        for tok in raw.unicode_words().filter_map(normalize_token) {
            *tf_total.entry(tok.clone()).or_insert(0) += 1;
            chunk_seen.insert(tok);
        }
        for tok in chunk_seen {
            *df_chunks.entry(tok).or_insert(0) += 1;
        }
        for (sent_idx, sentence) in split_sentences(raw).into_iter().enumerate() {
            if candidates.len() >= MAX_CANDIDATES {
                break;
            }
            let mut tf = HashMap::new();
            for tok in sentence.unicode_words().filter_map(normalize_token) {
                *tf.entry(tok).or_insert(0.0) += 1.0;
            }
            if tf.is_empty() {
                continue;
            }
            candidates.push(Candidate {
                page,
                order,
                page_lead: sent_idx == 0,
                doc_lead: page_idx == 0 && sent_idx < 2,
                text: sentence,
                tf,
            });
            order += 1;
        }
    }

    let n_chunks = page_texts.len().max(1) as f64;
    let mut weights = HashMap::new();
    for (term, count) in tf_total {
        let df = *df_chunks.get(&term).unwrap_or(&1) as f64;
        let idf = 1.0 + ((n_chunks + 1.0) / (df + 1.0)).ln();
        weights.insert(term, (count as f64) * idf);
    }
    (candidates, weights)
}

fn top_terms(weights: &HashMap<String, f64>, limit: usize) -> Vec<String> {
    let mut list: Vec<(&String, f64)> = weights.iter().map(|(k, &v)| (k, v)).collect();
    list.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.0.cmp(b.0)));
    list.into_iter().take(limit).map(|(k, _)| k.clone()).collect()
}

fn select_top_sentences(
    candidates: &[Candidate],
    term_weights: &HashMap<String, f64>,
    max_k: usize,
) -> Vec<SummaryPoint> {
    if candidates.is_empty() {
        return Vec::new();
    }
    let norms: Vec<f64> = candidates
        .iter()
        .map(|c| vector_norm(&c.tf, term_weights))
        .collect();
    let mut raw_scores = vec![0.0f64; candidates.len()];
    for (i, cand) in candidates.iter().enumerate() {
        let mut centrality = 0.0;
        for (j, other) in candidates.iter().enumerate() {
            if i != j {
                centrality += cosine(&cand.tf, &other.tf, norms[i], norms[j], term_weights);
            }
        }
        let term_sum: f64 = cand
            .tf
            .iter()
            .map(|(t, &cnt)| cnt * term_weights.get(t).copied().unwrap_or(1.0))
            .sum();
        let len_penalty = (cand.tf.len() as f64).sqrt().max(1.5);
        let mut score = centrality + (term_sum / len_penalty) * 0.25;
        if cand.doc_lead {
            score *= 1.20;
        } else if cand.page_lead {
            score *= 1.08;
        }
        raw_scores[i] = score;
    }

    let mut ranked: Vec<usize> = (0..candidates.len()).collect();
    ranked.sort_by(|&a, &b| {
        raw_scores[b]
            .total_cmp(&raw_scores[a])
            .then(candidates[a].order.cmp(&candidates[b].order))
    });

    let max_score = raw_scores
        .iter()
        .copied()
        .fold(0.0f64, f64::max)
        .max(1e-6);
    let mut chosen: Vec<usize> = Vec::new();
    for idx in ranked {
        if chosen.len() >= max_k {
            break;
        }
        let redundant = chosen.iter().any(|&prev| {
            cosine(
                &candidates[idx].tf,
                &candidates[prev].tf,
                norms[idx],
                norms[prev],
                term_weights,
            ) > 0.65
        });
        if !redundant {
            chosen.push(idx);
        }
    }
    chosen.sort_by_key(|&idx| candidates[idx].order);
    chosen
        .into_iter()
        .map(|idx| SummaryPoint {
            page: candidates[idx].page,
            text: candidates[idx].text.clone(),
            score_milli: ((raw_scores[idx] / max_score) * 1000.0).round() as u32,
        })
        .collect()
}

fn vector_norm(tf: &HashMap<String, f64>, weights: &HashMap<String, f64>) -> f64 {
    tf.iter()
        .map(|(t, &c)| {
            let w = c * weights.get(t).copied().unwrap_or(1.0);
            w * w
        })
        .sum::<f64>()
        .sqrt()
}

fn cosine(
    a: &HashMap<String, f64>,
    b: &HashMap<String, f64>,
    norm_a: f64,
    norm_b: f64,
    weights: &HashMap<String, f64>,
) -> f64 {
    if norm_a <= 1e-9 || norm_b <= 1e-9 {
        return 0.0;
    }
    let (small, large) = if a.len() <= b.len() { (a, b) } else { (b, a) };
    let dot: f64 = small
        .iter()
        .filter_map(|(t, &ca)| {
            let cb = *large.get(t)?;
            let idf = weights.get(t).copied().unwrap_or(1.0);
            Some((ca * idf) * (cb * idf))
        })
        .sum();
    dot / (norm_a * norm_b)
}

fn split_sentences(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    for block in text.split("\n\n") {
        let mut current = String::new();
        for ch in block.chars() {
            let normalized = if ch.is_whitespace() { ' ' } else { ch };
            if normalized == ' ' && current.ends_with(' ') {
                continue;
            }
            current.push(normalized);
            if matches!(ch, '.' | '!' | '?' | '。' | '！' | '？') && is_informative_sentence(&current) {
                out.push(current.trim().to_string());
                current.clear();
            }
        }
        if is_informative_sentence(&current) {
            out.push(current.trim().to_string());
        }
    }
    out
}

fn is_informative_sentence(s: &str) -> bool {
    let trimmed = s.trim();
    let chars = trimmed.chars().filter(|c| !c.is_whitespace()).count();
    let words = trimmed.unicode_words().count();
    chars >= 16 && chars <= 420 && (words >= 4 || trimmed.chars().any(is_cjk_or_hangul))
}

fn is_cjk_or_hangul(c: char) -> bool {
    matches!(
        c as u32,
        0xAC00..=0xD7A3 | 0x3040..=0x30FF | 0x4E00..=0x9FFF
    )
}

fn normalize_token(word: &str) -> Option<String> {
    let lower = word.trim_matches(|c: char| !c.is_alphanumeric()).to_lowercase();
    if lower.is_empty() || lower.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    let stripped = strip_korean_josa(&lower);
    let char_len = stripped.chars().count();
    let min_len = if stripped.chars().any(is_cjk_or_hangul) { 2 } else { 3 };
    if char_len < min_len || is_stopword(stripped) {
        return None;
    }
    Some(stripped.to_string())
}

fn strip_korean_josa(token: &str) -> &str {
    if !token.chars().any(|c| matches!(c as u32, 0xAC00..=0xD7A3)) {
        return token;
    }
    for suffix in ["에서", "으로", "에게", "까지", "부터", "은", "는", "이", "가", "을", "를", "의", "에", "로", "와", "과", "도"] {
        if let Some(stem) = token.strip_suffix(suffix)
            && stem.chars().count() >= 2
        {
            return stem;
        }
    }
    token
}

fn is_stopword(token: &str) -> bool {
    const EN_KO_STOPWORDS: &[&str] = &[
        "the", "and", "for", "that", "this", "with", "from", "are", "was", "were", "have", "has",
        "had", "not", "but", "you", "your", "all", "can", "will", "into", "more", "other", "than",
        "then", "them", "they", "their", "there", "these", "those", "which", "when", "where",
        "what", "who", "whom", "whose", "been", "being", "also", "such", "only", "each", "some",
        "over", "under", "between", "after", "before", "about", "through", "during", "within",
        "without", "because", "while", "should", "would", "could", "may", "might", "must", "shall",
        "있으며", "있습니다", "있다", "없는", "없다", "대한", "위해", "통해", "따라", "그리고",
        "그러나", "또한", "경우", "것이다", "수행", "관한", "해당",
    ];
    EN_KO_STOPWORDS.contains(&token)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Session;

    #[test]
    fn summarizes_multi_page_english_document_with_page_citations() {
        let text = concat!(
            "Quarterly security architecture review covers cryptographic signing and archival storage. ",
            "All production documents require embedded fonts and PDF/A verification before release.\u{c}",
            "Content Credentials attach JUMBF provenance manifests to verify media origin. ",
            "Automated preflight profiles detect unapplied redactions and external scripts early."
        );
        let session = Session::new();
        let bytes = session.create_from_text("Architecture Review", text);
        assert!(bytes.is_ok());
        let Ok(bytes) = bytes else { return };
        let mut session = Session::new();
        let opened = session.open_new("review.pdf", std::sync::Arc::new(bytes.to_vec()));
        assert!(opened.is_ok());
        let Ok(id) = opened else { return };
        let Some(doc) = session.get(id) else { return };

        let summary = doc.summarize(None, 3);
        assert!(summary.is_ok());
        let Ok(summary) = summary else { return };
        assert_eq!(summary.page_count, 2);
        assert!(summary.word_count >= 30);
        assert_eq!(summary.reading_minutes, 1);
        assert!(!summary.key_terms.is_empty());
        assert!(!summary.points.is_empty());
        assert!(summary.points.len() <= 3);
        let md = summary.to_markdown("Architecture Review");
        assert!(md.contains("# Architecture Review"));
        assert!(md.contains("(p. 1)") || md.contains("(p. 2)"));
    }

    #[test]
    fn summarizes_korean_sentences_and_stems_hangul_particles() {
        let pages = vec![
            (
                0,
                "디지털 서명 검증은 문서의 무결성과 작성자 신원을 보장합니다. 프리플라이트 검사는 인쇄 해상도와 글꼴 임베딩 오류를 사전에 탐지합니다.".to_string(),
            ),
            (
                1,
                "콘텐츠 자격 증명은 이미지와 문서의 출처 이력을 투명하게 기록합니다. 베이츠 번호 매기기는 법률 문서 관리에서 핵심적인 역할을 담당합니다.".to_string(),
            ),
        ];
        let summary = summarize_pages(&pages, 42, 0, 2);
        assert_eq!(summary.points.len(), 2);
        assert!(!summary.key_terms.is_empty());
        assert!(summary.key_terms.iter().any(|t| t.contains("문서") || t.contains("서명") || t.contains("프리플라이트")));
    }

    #[test]
    fn empty_pages_produce_clean_zero_point_summary() {
        let summary = summarize_pages(&[(0, "   \n  ".into())], 0, 1, 5);
        assert_eq!(summary.page_count, 1);
        assert_eq!(summary.pages_without_text, 1);
        assert_eq!(summary.reading_minutes, 0);
        assert!(summary.points.is_empty());
    }
}
