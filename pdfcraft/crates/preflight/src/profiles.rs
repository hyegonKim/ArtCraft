//! Multi-profile preflight checks across print, archival, digital publishing, and security.
//!
//! All checks inspect the PDF object graph without mutating the document. Archival profiles reuse
//! [`crate::verify`], while digital-publishing and security-audit profiles inspect metadata,
//! language tags, tagged structure, interactive actions, embedded files, and unapplied redactions.

use pdfcraft_cos::{Dict, Document};

use crate::{Level, catalog, declared, dict_of, metadata, objects, verify};

/// Built-in preflight inspection profiles.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum PreflightProfile {
    #[default]
    PrintProduction,
    PdfA2b,
    PdfA3b,
    DigitalPublishing,
    SecurityAudit,
}

impl PreflightProfile {
    pub const ALL: [PreflightProfile; 5] = [
        PreflightProfile::PrintProduction,
        PreflightProfile::PdfA2b,
        PreflightProfile::PdfA3b,
        PreflightProfile::DigitalPublishing,
        PreflightProfile::SecurityAudit,
    ];

    pub fn id(self) -> &'static str {
        match self {
            PreflightProfile::PrintProduction => "print",
            PreflightProfile::PdfA2b => "pdfa-2b",
            PreflightProfile::PdfA3b => "pdfa-3b",
            PreflightProfile::DigitalPublishing => "digital",
            PreflightProfile::SecurityAudit => "security",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            PreflightProfile::PrintProduction => "Print production",
            PreflightProfile::PdfA2b => "PDF/A-2b archival",
            PreflightProfile::PdfA3b => "PDF/A-3b archival",
            PreflightProfile::DigitalPublishing => "Online & digital publishing",
            PreflightProfile::SecurityAudit => "Interactive actions & hidden content",
        }
    }

    pub fn description(self) -> &'static str {
        match self {
            PreflightProfile::PrintProduction => {
                "Checks effective image resolution, trim and bleed boxes, output intents, and embedded fonts."
            }
            PreflightProfile::PdfA2b => {
                "Checks ISO 19005-2 structural rules for self-contained archival documents."
            }
            PreflightProfile::PdfA3b => {
                "Checks ISO 19005-3 structural rules including associated embedded files."
            }
            PreflightProfile::DigitalPublishing => {
                "Checks document title, language tag, tagged structure, embedded fonts, and encryption."
            }
            PreflightProfile::SecurityAudit => {
                "Audits scripts, launch and external actions, attachments, unapplied redactions, and XFA."
            }
        }
    }

    pub fn pdfa_level(self) -> Option<Level> {
        match self {
            PreflightProfile::PdfA2b => Some(Level::A2b),
            PreflightProfile::PdfA3b => Some(Level::A3b),
            _ => None,
        }
    }

    pub fn from_id(id: &str) -> Option<PreflightProfile> {
        let norm = id.trim().to_ascii_lowercase().replace(['_', '/', ' '], "-");
        match norm.as_str() {
            "print" | "print-production" | "press" => Some(PreflightProfile::PrintProduction),
            "pdfa-2b" | "pdfa2b" | "a2b" | "2b" => Some(PreflightProfile::PdfA2b),
            "pdfa-3b" | "pdfa3b" | "a3b" | "3b" => Some(PreflightProfile::PdfA3b),
            "digital" | "digital-publishing" | "online" | "web" => {
                Some(PreflightProfile::DigitalPublishing)
            }
            "security" | "security-audit" | "audit" => Some(PreflightProfile::SecurityAudit),
            _ => None,
        }
    }
}

/// Severity of a single preflight finding.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PreflightSeverity {
    Error,
    Warning,
    Info,
}

impl PreflightSeverity {
    pub fn label(self) -> &'static str {
        match self {
            PreflightSeverity::Error => "error",
            PreflightSeverity::Warning => "warning",
            PreflightSeverity::Info => "info",
        }
    }
}

/// One issue or observation produced by a preflight profile run.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PreflightFinding {
    pub severity: PreflightSeverity,
    pub code: &'static str,
    pub clause: &'static str,
    pub message: String,
    /// 1-based page index when the finding applies to a specific page.
    pub page: Option<usize>,
}

/// Complete report for a preflight profile run.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PreflightSummary {
    pub profile: PreflightProfile,
    pub errors: usize,
    pub warnings: usize,
    pub infos: usize,
    pub findings: Vec<PreflightFinding>,
}

impl PreflightSummary {
    pub fn from_findings(profile: PreflightProfile, mut findings: Vec<PreflightFinding>) -> Self {
        findings.sort_by(|a, b| {
            a.severity
                .cmp(&b.severity)
                .then(a.page.cmp(&b.page))
                .then(a.code.cmp(b.code))
                .then(a.message.cmp(&b.message))
        });
        findings.dedup();
        let errors = findings
            .iter()
            .filter(|f| f.severity == PreflightSeverity::Error)
            .count();
        let warnings = findings
            .iter()
            .filter(|f| f.severity == PreflightSeverity::Warning)
            .count();
        let infos = findings
            .iter()
            .filter(|f| f.severity == PreflightSeverity::Info)
            .count();
        Self {
            profile,
            errors,
            warnings,
            infos,
            findings,
        }
    }

    /// Formats a plain-text report without claiming external standard certification.
    pub fn report_text(&self) -> String {
        let mut out = format!(
            "Preflight profile: {} ({})\nSummary: {} error(s), {} warning(s), {} info note(s)\n",
            self.profile.label(),
            self.profile.id(),
            self.errors,
            self.warnings,
            self.infos
        );
        if self.findings.is_empty() {
            out.push_str("Result: structural check found no problems for this profile.\n");
            return out;
        }
        for f in &self.findings {
            let loc = match f.page {
                Some(p) => format!(" (page {p})"),
                None => String::new(),
            };
            let clause = if f.clause.is_empty() {
                String::new()
            } else {
                format!(" [{}]", f.clause)
            };
            out.push_str(&format!(
                "- [{}]{}{} {}\n",
                f.severity.label(),
                clause,
                loc,
                f.message
            ));
        }
        out
    }
}

/// Runs the object-graph checks for `profile` on `doc`.
pub fn run_profile(doc: &Document, profile: PreflightProfile) -> PreflightSummary {
    let findings = match profile {
        PreflightProfile::PdfA2b => pdfa_findings(doc, Level::A2b),
        PreflightProfile::PdfA3b => pdfa_findings(doc, Level::A3b),
        PreflightProfile::PrintProduction => print_graph_findings(doc),
        PreflightProfile::DigitalPublishing => digital_findings(doc),
        PreflightProfile::SecurityAudit => security_findings(doc),
    };
    PreflightSummary::from_findings(profile, findings)
}

fn pdfa_findings(doc: &Document, level: Level) -> Vec<PreflightFinding> {
    let code = match level {
        Level::A2b => "pdfa-2b",
        Level::A3b => "pdfa-3b",
    };
    verify(doc, level)
        .into_iter()
        .map(|issue| PreflightFinding {
            severity: PreflightSeverity::Error,
            code,
            clause: issue.clause,
            message: issue.message,
            page: issue.page.map(|p| p + 1),
        })
        .collect()
}

fn print_graph_findings(doc: &Document) -> Vec<PreflightFinding> {
    let mut out = Vec::new();
    if doc.trailer().get(b"Encrypt").is_some() {
        out.push(PreflightFinding {
            severity: PreflightSeverity::Error,
            code: "print-encrypted",
            clause: "Encrypt",
            message: "The document is encrypted; prepress workflows require an unencrypted file.".into(),
            page: None,
        });
    }
    let decl = declared(doc);
    if decl.output_intents.is_empty() {
        out.push(PreflightFinding {
            severity: PreflightSeverity::Warning,
            code: "print-output-intent",
            clause: "OutputIntents",
            message: "No output intent condition is declared for press colour management.".into(),
            page: None,
        });
    }
    for issue in verify(doc, Level::A2b) {
        if issue.pattern.contains("isn't embedded") {
            out.push(PreflightFinding {
                severity: PreflightSeverity::Error,
                code: "print-font",
                clause: issue.clause,
                message: issue.message,
                page: issue.page.map(|p| p + 1),
            });
        } else if issue.pattern.contains("transparency") {
            out.push(PreflightFinding {
                severity: PreflightSeverity::Warning,
                code: "print-transparency",
                clause: issue.clause,
                message: issue.message,
                page: issue.page.map(|p| p + 1),
            });
        }
    }
    out
}

fn digital_findings(doc: &Document) -> Vec<PreflightFinding> {
    let mut out = Vec::new();
    let cat = catalog(doc);
    if !has_document_title(doc) {
        out.push(PreflightFinding {
            severity: PreflightSeverity::Warning,
            code: "digital-title",
            clause: "Title",
            message: "Document metadata has no non-empty title.".into(),
            page: None,
        });
    }
    let has_lang = cat
        .get(b"Lang")
        .and_then(|o| doc.resolve(o).as_string().map(|s| !s.to_text().trim().is_empty()))
        .unwrap_or(false);
    if !has_lang {
        out.push(PreflightFinding {
            severity: PreflightSeverity::Warning,
            code: "digital-lang",
            clause: "Lang",
            message: "Catalog has no primary language (/Lang) tag for screen readers.".into(),
            page: None,
        });
    }
    let has_tags = cat.get(b"StructTreeRoot").is_some();
    if !has_tags {
        out.push(PreflightFinding {
            severity: PreflightSeverity::Info,
            code: "digital-tags",
            clause: "StructTreeRoot",
            message: "Document has no tagged structure tree (/StructTreeRoot).".into(),
            page: None,
        });
    }
    for issue in verify(doc, Level::A2b) {
        if issue.pattern.contains("isn't embedded") {
            out.push(PreflightFinding {
                severity: PreflightSeverity::Error,
                code: "digital-font",
                clause: issue.clause,
                message: issue.message,
                page: issue.page.map(|p| p + 1),
            });
        }
    }
    if doc.trailer().get(b"Encrypt").is_some() {
        out.push(PreflightFinding {
            severity: PreflightSeverity::Info,
            code: "digital-encrypted",
            clause: "Encrypt",
            message: "Document is encrypted with access permissions.".into(),
            page: None,
        });
    }
    out
}

fn has_document_title(doc: &Document) -> bool {
    let info_title = doc
        .trailer()
        .get(b"Info")
        .and_then(|o| doc.resolve(o).as_dict().cloned())
        .and_then(|d| {
            d.get(b"Title")
                .and_then(|t| doc.resolve(t).as_string().map(|s| !s.to_text().trim().is_empty()))
        })
        .unwrap_or(false);
    if info_title {
        return true;
    }
    metadata(doc)
        .is_some_and(|xmp| xmp.contains(":title") && !xmp.contains("<dc:title/>"))
}

fn security_findings(doc: &Document) -> Vec<PreflightFinding> {
    let mut out = Vec::new();
    let cat = catalog(doc);
    check_catalog_security(doc, &cat, &mut out);
    check_page_redactions(doc, &mut out);
    for (_, obj) in objects(doc) {
        let Some(dict) = dict_of(&obj) else { continue };
        check_object_security(doc, dict, &mut out);
    }
    if doc.trailer().get(b"Info").is_some() || cat.get(b"Metadata").is_some() {
        out.push(PreflightFinding {
            severity: PreflightSeverity::Info,
            code: "security-metadata",
            clause: "Metadata",
            message: "Document contains Info or XMP metadata streams; sanitize before sharing if sensitive.".into(),
            page: None,
        });
    }
    out
}

fn check_catalog_security(doc: &Document, cat: &Dict, out: &mut Vec<PreflightFinding>) {
    if cat.get(b"OpenAction").is_some() {
        out.push(PreflightFinding {
            severity: PreflightSeverity::Warning,
            code: "security-open-action",
            clause: "OpenAction",
            message: "Catalog defines an automatic /OpenAction.".into(),
            page: None,
        });
    }
    if let Some(names) = cat.get(b"Names").and_then(|n| doc.resolve(n).as_dict().cloned())
        && names.get(b"JavaScript").is_some()
    {
        out.push(PreflightFinding {
            severity: PreflightSeverity::Error,
            code: "security-javascript",
            clause: "JavaScript",
            message: "Document name tree contains embedded JavaScript.".into(),
            page: None,
        });
    }
    if let Some(form) = cat.get(b"AcroForm").and_then(|f| doc.resolve(f).as_dict().cloned())
        && form.get(b"XFA").is_some()
    {
        out.push(PreflightFinding {
            severity: PreflightSeverity::Warning,
            code: "security-xfa",
            clause: "XFA",
            message: "Document contains an interactive XFA form packet.".into(),
            page: None,
        });
    }
}

fn check_page_redactions(doc: &Document, out: &mut Vec<PreflightFinding>) {
    for (idx, page) in pdfcraft_model::pages(doc).into_iter().enumerate() {
        let Some(annots) = page.dict.get(b"Annots").map(|a| doc.resolve(a)) else {
            continue;
        };
        let Some(list) = annots.as_array() else { continue };
        for item in list {
            let resolved = doc.resolve(item);
            let Some(d) = resolved.as_dict() else { continue };
            if d.name(b"Subtype") == Some(b"Redact") {
                out.push(PreflightFinding {
                    severity: PreflightSeverity::Error,
                    code: "security-unapplied-redact",
                    clause: "Redact",
                    message: "Unapplied redaction mark found; underlying content has not been removed yet.".into(),
                    page: Some(idx + 1),
                });
            }
        }
    }
}

fn check_object_security(doc: &Document, dict: &Dict, out: &mut Vec<PreflightFinding>) {
    if dict.name(b"Type") == Some(b"Filespec") && dict.get(b"EF").is_some() {
        let name = dict
            .get(b"UF")
            .or_else(|| dict.get(b"F"))
            .and_then(|v| doc.resolve(v).as_string().map(|s| s.to_text()))
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "unnamed attachment".into());
        out.push(PreflightFinding {
            severity: PreflightSeverity::Warning,
            code: "security-attachment",
            clause: "EmbeddedFiles",
            message: format!("Embedded file attachment present: {name}."),
            page: None,
        });
    }
    match dict.name(b"S") {
        Some(b"JavaScript") => out.push(PreflightFinding {
            severity: PreflightSeverity::Error,
            code: "security-javascript",
            clause: "JavaScript",
            message: "Action executes JavaScript.".into(),
            page: None,
        }),
        Some(b"Launch") => out.push(PreflightFinding {
            severity: PreflightSeverity::Error,
            code: "security-launch",
            clause: "Launch",
            message: "Action launches an external application or file.".into(),
            page: None,
        }),
        Some(b"SubmitForm") | Some(b"ImportData") => out.push(PreflightFinding {
            severity: PreflightSeverity::Warning,
            code: "security-form-action",
            clause: "FormAction",
            message: "Action transmits or imports form data externally.".into(),
            page: None,
        }),
        Some(b"URI") => out.push(PreflightFinding {
            severity: PreflightSeverity::Info,
            code: "security-uri",
            clause: "URI",
            message: "Document contains external URI link action(s).".into(),
            page: None,
        }),
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pdfcraft_cos::{Object, PdfString};

    #[test]
    fn profile_lookup_accepts_short_and_long_ids() {
        assert_eq!(PreflightProfile::from_id("print"), Some(PreflightProfile::PrintProduction));
        assert_eq!(PreflightProfile::from_id("pdfa-2b"), Some(PreflightProfile::PdfA2b));
        assert_eq!(PreflightProfile::from_id("3b"), Some(PreflightProfile::PdfA3b));
        assert_eq!(PreflightProfile::from_id("digital"), Some(PreflightProfile::DigitalPublishing));
        assert_eq!(PreflightProfile::from_id("security"), Some(PreflightProfile::SecurityAudit));
        assert_eq!(PreflightProfile::from_id("unknown"), None);
    }

    #[test]
    fn security_audit_detects_javascript_and_unapplied_redaction() {
        let mut doc = Document::new_empty();
        let js_action = doc.add(Object::Dict({
            let mut d = Dict::new();
            d.set(b"Type".to_vec(), Object::name("Action"));
            d.set(b"S".to_vec(), Object::name("JavaScript"));
            d.set(b"JS".to_vec(), Object::String(PdfString::text("app.alert(1)")));
            d
        }));
        let redact_annot = doc.add(Object::Dict({
            let mut d = Dict::new();
            d.set(b"Type".to_vec(), Object::name("Annot"));
            d.set(b"Subtype".to_vec(), Object::name("Redact"));
            d.set(
                b"Rect".to_vec(),
                Object::Array(vec![
                    Object::Int(0),
                    Object::Int(0),
                    Object::Int(50),
                    Object::Int(20),
                ]),
            );
            d
        }));
        let pages_ref = doc
            .root()
            .and_then(|r| doc.get(r).as_dict().and_then(|c| c.get(b"Pages").and_then(Object::as_ref)));
        assert!(pages_ref.is_some());
        if let Some(pages) = pages_ref {
            let mut page = Dict::new();
            page.set(b"Type".to_vec(), Object::name("Page"));
            page.set(b"Parent".to_vec(), Object::Ref(pages));
            page.set(
                b"MediaBox".to_vec(),
                Object::Array(vec![
                    Object::Int(0),
                    Object::Int(0),
                    Object::Int(612),
                    Object::Int(792),
                ]),
            );
            page.set(b"Annots".to_vec(), Object::Array(vec![Object::Ref(redact_annot)]));
            page.set(b"AA".to_vec(), Object::Ref(js_action));
            let page_ref = doc.add(Object::Dict(page));
            let _ = doc.update_dict(pages, |d| {
                d.set(b"Count".to_vec(), Object::Int(1));
                d.set(b"Kids".to_vec(), Object::Array(vec![Object::Ref(page_ref)]));
            });
        }

        let summary = run_profile(&doc, PreflightProfile::SecurityAudit);
        assert!(summary.errors >= 2, "{summary:?}");
        assert!(summary.findings.iter().any(|f| f.code == "security-javascript"));
        assert!(
            summary
                .findings
                .iter()
                .any(|f| f.code == "security-unapplied-redact" && f.page == Some(1))
        );
        let text = summary.report_text();
        assert!(!text.contains("conforms to"));
        assert!(!text.contains("now conforms"));
    }

    #[test]
    fn digital_publishing_flags_missing_title_and_lang_until_set() {
        let mut doc = Document::new_empty();
        let before = run_profile(&doc, PreflightProfile::DigitalPublishing);
        assert!(before.findings.iter().any(|f| f.code == "digital-title"));
        assert!(before.findings.iter().any(|f| f.code == "digital-lang"));

        if let Some(root) = doc.root() {
            let _ = doc.update_dict(root, |d| {
                d.set(b"Lang".to_vec(), Object::String(PdfString::text("ko-KR")));
                d.set(b"StructTreeRoot".to_vec(), Object::Dict(Dict::new()));
            });
        }
        let info = doc.add(Object::Dict({
            let mut d = Dict::new();
            d.set(b"Title".to_vec(), Object::String(PdfString::text("연간 보고서")));
            d
        }));
        doc.trailer_mut().set(b"Info".to_vec(), Object::Ref(info));

        let after = run_profile(&doc, PreflightProfile::DigitalPublishing);
        assert!(!after.findings.iter().any(|f| f.code == "digital-title"));
        assert!(!after.findings.iter().any(|f| f.code == "digital-lang"));
        assert!(!after.findings.iter().any(|f| f.code == "digital-tags"));
    }
}
