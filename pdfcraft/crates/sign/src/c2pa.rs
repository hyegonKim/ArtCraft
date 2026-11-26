//! Content Credentials (C2PA / ISO 19566-5 JUMBF) provenance inspection for PDF documents.
//!
//! Scans a PDF's Associated Files (`/AF`), EmbeddedFiles name tree, XMP provenance references, and
//! embedded image streams (JPEG XT `APP11` `JP` segments and raw `jumb` superboxes) for C2PA
//! manifests without mutating the document.

use pdfcraft_cos::{Dict, Document, Object, Stream};

use crate::Certificate;
use crate::der::{Tlv, tag};

/// Upper bound on decoded stream bytes inspected for a single C2PA container.
const MAX_C2PA_STREAM_BYTES: usize = 16 << 20;
/// Maximum nesting depth when walking ISO BMFF / JUMBF boxes.
const MAX_BOX_DEPTH: usize = 8;
/// Maximum nesting depth when walking CBOR maps/arrays in a C2PA claim.
const MAX_CBOR_DEPTH: usize = 6;
/// Maximum items read in a single CBOR container.
const MAX_CBOR_ITEMS: usize = 256;

/// C2PA JUMBF store UUID (`"c2pa"` prefix `63 32 70 61` + JUMBF suffix).
pub const C2PA_STORE_UUID: [u8; 16] = [
    0x63, 0x32, 0x70, 0x61, 0x00, 0x11, 0x00, 0x10, 0x80, 0x00, 0x00, 0xAA, 0x00, 0x38, 0x9B, 0x71,
];
/// C2PA Manifest box UUID (`"c2ma"`).
pub const C2PA_MANIFEST_UUID: [u8; 16] = [
    0x63, 0x32, 0x6D, 0x61, 0x00, 0x11, 0x00, 0x10, 0x80, 0x00, 0x00, 0xAA, 0x00, 0x38, 0x9B, 0x71,
];
/// C2PA Claim box UUID (`"c2cl"`).
pub const C2PA_CLAIM_UUID: [u8; 16] = [
    0x63, 0x32, 0x63, 0x6C, 0x00, 0x11, 0x00, 0x10, 0x80, 0x00, 0x00, 0xAA, 0x00, 0x38, 0x9B, 0x71,
];
/// C2PA Assertion Store UUID (`"c2as"`).
pub const C2PA_ASSERTIONS_UUID: [u8; 16] = [
    0x63, 0x32, 0x61, 0x73, 0x00, 0x11, 0x00, 0x10, 0x80, 0x00, 0x00, 0xAA, 0x00, 0x38, 0x9B, 0x71,
];
/// C2PA Claim Signature UUID (`"c2cs"`).
pub const C2PA_SIGNATURE_UUID: [u8; 16] = [
    0x63, 0x32, 0x63, 0x73, 0x00, 0x11, 0x00, 0x10, 0x80, 0x00, 0x00, 0xAA, 0x00, 0x38, 0x9B, 0x71,
];

/// Signer certificate metadata extracted from a C2PA `c2cs` signature box.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct C2paSigner {
    pub subject: String,
    pub issuer: String,
    pub serial_hex: String,
    pub valid_from: String,
    pub valid_to: String,
    pub self_signed: bool,
    pub sha256_fingerprint: String,
}

/// One C2PA manifest discovered inside a PDF document or one of its embedded assets.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct C2paManifest {
    /// Where in the PDF the manifest was located (e.g. `"Associated File (/AF): manifest.c2pa"`).
    pub source: String,
    /// Manifest JUMBF label (typically a `urn:uuid:...` or `contentauth:urn:uuid:...`).
    pub label: String,
    /// Claim generator tool/version (`claim_generator` or `claim_generator_info`).
    pub claim_generator: Option<String>,
    /// Claimed asset title (`dc:title`).
    pub title: Option<String>,
    /// Claimed MIME format (`dc:format` / `format`).
    pub format: Option<String>,
    /// Claimed instance ID (`instanceID`).
    pub instance_id: Option<String>,
    /// Hash algorithm declared in the claim (`alg`).
    pub digest_alg: Option<String>,
    /// Assertion labels listed in the manifest's assertion store (`c2pa.actions`, etc.).
    pub assertions: Vec<String>,
    /// Whether a `c2cs` COSE/CMS claim signature box is present.
    pub has_signature_box: bool,
    /// Signer certificates parsed from the `c2cs` signature payload.
    pub signers: Vec<C2paSigner>,
}

/// Full Content Credentials inspection report for a PDF document.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ContentCredentialsReport {
    /// External or active-manifest URI declared in XMP (`dcterms:provenance` or `c2pa:manifest`).
    pub xmp_provenance: Option<String>,
    /// Discovered C2PA manifests across document attachments and embedded images.
    pub manifests: Vec<C2paManifest>,
}

impl ContentCredentialsReport {
    pub fn has_credentials(&self) -> bool {
        !self.manifests.is_empty() || self.xmp_provenance.is_some()
    }

    pub fn summary_text(&self) -> String {
        if !self.has_credentials() {
            return "No Content Credentials (C2PA manifests or XMP provenance links) were found in this PDF.".into();
        }
        let mut out = String::new();
        if let Some(uri) = &self.xmp_provenance {
            out.push_str(&format!("XMP provenance reference: {uri}\n"));
        }
        out.push_str(&format!("C2PA manifests found: {}\n", self.manifests.len()));
        for (idx, m) in self.manifests.iter().enumerate() {
            out.push_str(&format!(
                "\n{}. {} ({})\n",
                idx + 1,
                if m.label.is_empty() { "unnamed-manifest" } else { &m.label },
                m.source
            ));
            if let Some(generator) = &m.claim_generator {
                out.push_str(&format!("   Generator: {generator}\n"));
            }
            if let Some(title) = &m.title {
                out.push_str(&format!("   Title: {title}\n"));
            }
            if !m.assertions.is_empty() {
                out.push_str(&format!("   Assertions: {}\n", m.assertions.join(", ")));
            }
            if let Some(signer) = m.signers.first() {
                out.push_str(&format!(
                    "   Signer: {} (issuer: {})\n",
                    signer.subject, signer.issuer
                ));
            } else if m.has_signature_box {
                out.push_str("   Signature box: present (certificate chain not embedded in DER)\n");
            }
        }
        out
    }
}

/// Inspects a PDF COS [`Document`] for C2PA Content Credentials.
pub fn inspect_content_credentials(doc: &Document) -> ContentCredentialsReport {
    let mut report = ContentCredentialsReport {
        xmp_provenance: find_xmp_provenance(doc),
        manifests: Vec::new(),
    };
    let mut visited_streams = std::collections::HashSet::new();
    scan_associated_files(doc, &mut visited_streams, &mut report.manifests);
    scan_object_streams(doc, &mut visited_streams, &mut report.manifests);
    report
}

fn find_xmp_provenance(doc: &Document) -> Option<String> {
    let root = doc.root()?;
    let cat = doc.get(root).as_dict()?.clone();
    let meta = doc.resolve(cat.get(b"Metadata")?);
    let Object::Stream(stream) = &*meta else { return None };
    let bytes = stream.decoded_within(MAX_C2PA_STREAM_BYTES).ok()?;
    let text = String::from_utf8_lossy(&bytes);
    extract_xml_attr_or_tag(&text, "dcterms:provenance")
        .or_else(|| extract_xml_attr_or_tag(&text, "c2pa:manifest"))
}

fn extract_xml_attr_or_tag(xml: &str, key: &str) -> Option<String> {
    let attr_pat = format!("{key}=\"");
    if let Some(pos) = xml.find(&attr_pat) {
        let rest = xml.get(pos + attr_pat.len()..)?;
        let end = rest.find('"')?;
        let val = rest.get(..end)?.trim();
        if !val.is_empty() {
            return Some(val.to_string());
        }
    }
    let open_tag = format!("<{key}>");
    let close_tag = format!("</{key}>");
    let pos = xml.find(&open_tag)?;
    let rest = xml.get(pos + open_tag.len()..)?;
    let end = rest.find(&close_tag)?;
    let val = rest.get(..end)?.trim();
    (!val.is_empty()).then(|| val.to_string())
}

fn scan_associated_files(
    doc: &Document,
    visited: &mut std::collections::HashSet<u32>,
    out: &mut Vec<C2paManifest>,
) {
    let Some(root) = doc.root() else { return };
    let Some(cat) = doc.get(root).as_dict().cloned() else { return };
    let Some(af_list) = cat.get(b"AF").map(|o| doc.resolve(o)) else { return };
    let Some(items) = af_list.as_array() else { return };
    for entry in items {
        let resolved = doc.resolve(entry);
        let Some(filespec) = resolved.as_dict() else { continue };
        inspect_filespec(doc, filespec, "Associated File (/AF)", visited, out);
    }
}

fn inspect_filespec(
    doc: &Document,
    filespec: &Dict,
    origin: &str,
    visited: &mut std::collections::HashSet<u32>,
    out: &mut Vec<C2paManifest>,
) {
    let filename = filespec
        .get(b"UF")
        .or_else(|| filespec.get(b"F"))
        .and_then(|v| doc.resolve(v).as_string().map(|s| s.to_text()))
        .unwrap_or_default();
    let Some(ef) = filespec.get(b"EF").and_then(|v| doc.resolve(v).as_dict().cloned()) else {
        return;
    };
    let Some(stream_obj) = ef.get(b"UF").or_else(|| ef.get(b"F")) else {
        return;
    };
    if let Some(r) = stream_obj.as_ref()
        && !visited.insert(r.num)
    {
        return;
    }
    let resolved = doc.resolve(stream_obj);
    let Object::Stream(stream) = &*resolved else { return };
    let source = if filename.is_empty() {
        origin.to_string()
    } else {
        format!("{origin}: {filename}")
    };
    inspect_stream_bytes(stream, &source, out);
}

fn scan_object_streams(
    doc: &Document,
    visited: &mut std::collections::HashSet<u32>,
    out: &mut Vec<C2paManifest>,
) {
    for num in doc.object_numbers() {
        let r = pdfcraft_cos::ObjRef::new(num, doc.generation(num));
        let obj = doc.get(r);
        if let Some(dict) = obj.as_dict()
            && dict.name(b"Type") == Some(b"Filespec")
        {
            inspect_filespec(doc, dict, "Embedded File", visited, out);
            continue;
        }
        if !visited.insert(num) {
            continue;
        }
        let Object::Stream(stream) = &*obj else { continue };
        let subtype = stream.dict.name(b"Subtype");
        if subtype == Some(b"Image") {
            let source = format!("Embedded Image (object {num} 0 R)");
            inspect_image_stream(stream, &source, out);
        } else if subtype == Some(b"application#2Fc2pa") || subtype == Some(b"application/c2pa") {
            let source = format!("C2PA Stream (object {num} 0 R)");
            inspect_stream_bytes(stream, &source, out);
        }
    }
}

fn inspect_stream_bytes(stream: &Stream, source: &str, out: &mut Vec<C2paManifest>) {
    let Ok(bytes) = stream.decoded_within(MAX_C2PA_STREAM_BYTES) else { return };
    parse_c2pa_jumbf_bytes(&bytes, source, out);
}

fn inspect_image_stream(stream: &Stream, source: &str, out: &mut Vec<C2paManifest>) {
    // For DCTDecode JPEGs, the raw stream data before DCT decoding holds APP11 JUMBF segments.
    if stream.raw.starts_with(&[0xFF, 0xD8])
        && let Some(jumbf) = extract_jpeg_app11_jumbf(&stream.raw)
    {
        parse_c2pa_jumbf_bytes(&jumbf, source, out);
        return;
    }
    let Ok(bytes) = stream.decoded_within(MAX_C2PA_STREAM_BYTES) else { return };
    if bytes.starts_with(&[0xFF, 0xD8])
        && let Some(jumbf) = extract_jpeg_app11_jumbf(&bytes)
    {
        parse_c2pa_jumbf_bytes(&jumbf, source, out);
    } else if bytes.starts_with(b"\x89PNG\r\n\x1a\n")
        && let Some(jumbf) = extract_png_cabx_jumbf(&bytes)
    {
        parse_c2pa_jumbf_bytes(&jumbf, source, out);
    }
}

/// Reassembles ISO 19566-5 JPEG XT `APP11` (`0xFFEB`, `"JP"`) JUMBF payloads.
pub fn extract_jpeg_app11_jumbf(jpeg: &[u8]) -> Option<Vec<u8>> {
    if !jpeg.starts_with(&[0xFF, 0xD8]) {
        return None;
    }
    let mut pos = 2usize;
    let mut assembled = Vec::new();
    while let Some(&marker_prefix) = jpeg.get(pos) {
        if marker_prefix != 0xFF {
            break;
        }
        while jpeg.get(pos) == Some(&0xFF) {
            pos += 1;
        }
        let Some(&marker) = jpeg.get(pos) else { break };
        pos += 1;
        if marker == 0xD9 || marker == 0xDA {
            break;
        }
        let Some(len_bytes) = jpeg.get(pos..pos + 2) else { break };
        let seg_len = u16::from_be_bytes([len_bytes[0], len_bytes[1]]) as usize;
        if seg_len < 2 {
            break;
        }
        let Some(payload) = jpeg.get(pos + 2..pos + seg_len) else { break };
        pos += seg_len;
        // ISO 19566-5 APP11 header: CI="JP" (2B), En (2B), Z (4B), then LBox+TBox ('jumb')...
        if marker == 0xEB
            && payload.starts_with(b"JP")
            && let Some(after_hdr) = payload.get(8..)
            && assembled.len().saturating_add(after_hdr.len()) <= MAX_C2PA_STREAM_BYTES
        {
            if assembled.is_empty() {
                assembled.extend_from_slice(after_hdr);
            } else if let Some(box_body) = strip_outer_jumb_header(after_hdr) {
                assembled.extend_from_slice(box_body);
            }
        }
    }
    (!assembled.is_empty()).then_some(assembled)
}

fn strip_outer_jumb_header(buf: &[u8]) -> Option<&[u8]> {
    if buf.get(4..8) == Some(b"jumb") {
        let len = u32::from_be_bytes([*buf.first()?, *buf.get(1)?, *buf.get(2)?, *buf.get(3)?]);
        if len == 1 {
            buf.get(16..)
        } else {
            buf.get(8..)
        }
    } else {
        Some(buf)
    }
}

fn extract_png_cabx_jumbf(png: &[u8]) -> Option<Vec<u8>> {
    let mut pos = 8usize;
    while let Some(hdr) = png.get(pos..pos + 8) {
        let len = u32::from_be_bytes([hdr[0], hdr[1], hdr[2], hdr[3]]) as usize;
        let kind = &hdr[4..8];
        let data_start = pos.checked_add(8)?;
        let data_end = data_start.checked_add(len)?;
        let chunk_data = png.get(data_start..data_end)?;
        if kind == b"caBX" && len <= MAX_C2PA_STREAM_BYTES {
            return Some(chunk_data.to_vec());
        }
        pos = data_end.checked_add(4)?;
    }
    None
}

#[derive(Clone, Copy)]
struct BmffBox<'a> {
    kind: [u8; 4],
    body: &'a [u8],
}

fn next_bmff_box(input: &[u8]) -> Option<(BmffBox<'_>, &[u8])> {
    if input.len() < 8 {
        return None;
    }
    let size32 = u32::from_be_bytes([input[0], input[1], input[2], input[3]]) as usize;
    let kind = [input[4], input[5], input[6], input[7]];
    let (hdr_len, total_len) = match size32 {
        0 => (8usize, input.len()),
        1 => {
            let xl = input.get(8..16)?;
            let size64 = u64::from_be_bytes([
                xl[0], xl[1], xl[2], xl[3], xl[4], xl[5], xl[6], xl[7],
            ]);
            let total = usize::try_from(size64).ok()?;
            if total < 16 {
                return None;
            }
            (16usize, total)
        }
        n if n < 8 => return None,
        n => (8usize, n),
    };
    let box_slice = input.get(..total_len)?;
    let body = box_slice.get(hdr_len..)?;
    let rest = input.get(total_len..)?;
    Some((BmffBox { kind, body }, rest))
}

#[derive(Default)]
struct JumdHeader {
    uuid: [u8; 16],
    label: String,
}

fn parse_jumd(body: &[u8]) -> Option<JumdHeader> {
    let uuid_slice = body.get(..16)?;
    let mut uuid = [0u8; 16];
    uuid.copy_from_slice(uuid_slice);
    let toggles = *body.get(16)?;
    let mut label = String::new();
    // Bit 0 (0x01) or bit 1 (0x02): ISO 19566-5 label present as NUL-terminated UTF-8.
    if toggles & 0x03 != 0
        && let Some(after) = body.get(17..)
    {
        let end = after.iter().position(|&b| b == 0).unwrap_or(after.len());
        if let Some(slice) = after.get(..end) {
            label = String::from_utf8_lossy(slice).into_owned();
        }
    }
    Some(JumdHeader { uuid, label })
}

/// Parses ISO 19566-5 JUMBF bytes and appends discovered [`C2paManifest`] entries to `out`.
pub fn parse_c2pa_jumbf_bytes(bytes: &[u8], source: &str, out: &mut Vec<C2paManifest>) {
    let mut rest = bytes;
    while let Some((b, next)) = next_bmff_box(rest) {
        rest = next;
        if &b.kind == b"jumb" {
            walk_jumb_superbox(b.body, source, 0, out);
        }
    }
}

fn walk_jumb_superbox(body: &[u8], source: &str, depth: usize, out: &mut Vec<C2paManifest>) {
    if depth >= MAX_BOX_DEPTH {
        return;
    }
    let Some((first, mut rest)) = next_bmff_box(body) else { return };
    if &first.kind != b"jumd" {
        return;
    }
    let Some(hdr) = parse_jumd(first.body) else { return };
    if hdr.uuid == C2PA_STORE_UUID || hdr.label == "c2pa" {
        while let Some((child, next)) = next_bmff_box(rest) {
            rest = next;
            if &child.kind == b"jumb" {
                walk_jumb_superbox(child.body, source, depth + 1, out);
            }
        }
        return;
    }
    if is_manifest_box(&hdr) {
        let mut manifest = C2paManifest {
            source: source.to_string(),
            label: hdr.label,
            ..C2paManifest::default()
        };
        populate_manifest(&mut manifest, rest, depth + 1);
        out.push(manifest);
    }
}

fn is_manifest_box(hdr: &JumdHeader) -> bool {
    hdr.uuid == C2PA_MANIFEST_UUID
        || hdr.uuid[..4] == *b"c2ma"
        || hdr.label.starts_with("urn:uuid:")
        || hdr.label.contains(":urn:uuid:")
        || hdr.label.starts_with("c2pa.manifest")
}

fn populate_manifest(manifest: &mut C2paManifest, mut body: &[u8], depth: usize) {
    if depth >= MAX_BOX_DEPTH {
        return;
    }
    while let Some((child, next)) = next_bmff_box(body) {
        body = next;
        if &child.kind != b"jumb" {
            continue;
        }
        let Some((desc_box, content_rest)) = next_bmff_box(child.body) else {
            continue;
        };
        if &desc_box.kind != b"jumd" {
            continue;
        }
        let Some(hdr) = parse_jumd(desc_box.body) else { continue };
        if hdr.uuid == C2PA_CLAIM_UUID || hdr.label.starts_with("c2pa.claim") {
            read_claim_boxes(manifest, content_rest);
        } else if hdr.uuid == C2PA_ASSERTIONS_UUID || hdr.label.starts_with("c2pa.assertions") {
            read_assertion_store(manifest, content_rest, depth + 1);
        } else if hdr.uuid == C2PA_SIGNATURE_UUID || hdr.label.starts_with("c2pa.signature") {
            manifest.has_signature_box = true;
            read_signature_boxes(manifest, content_rest);
        }
    }
}

fn read_claim_boxes(manifest: &mut C2paManifest, mut rest: &[u8]) {
    while let Some((b, next)) = next_bmff_box(rest) {
        rest = next;
        if &b.kind == b"cbor" {
            let mut pairs = Vec::new();
            let _ = parse_cbor_item(b.body, 0, None, &mut pairs);
            apply_claim_pairs(manifest, pairs);
        }
    }
}

fn apply_claim_pairs(manifest: &mut C2paManifest, pairs: Vec<(String, String)>) {
    for (k, v) in pairs {
        if v.is_empty() {
            continue;
        }
        match k.as_str() {
            "claim_generator" if manifest.claim_generator.is_none() => {
                manifest.claim_generator = Some(v);
            }
            "name" if manifest.claim_generator.is_none() => {
                manifest.claim_generator = Some(v);
            }
            "dc:title" | "title" if manifest.title.is_none() => {
                manifest.title = Some(v);
            }
            "dc:format" | "format" if manifest.format.is_none() => {
                manifest.format = Some(v);
            }
            "instanceID" | "instance_id" if manifest.instance_id.is_none() => {
                manifest.instance_id = Some(v);
            }
            "alg" if manifest.digest_alg.is_none() => {
                manifest.digest_alg = Some(v);
            }
            _ => {}
        }
    }
}

fn read_assertion_store(manifest: &mut C2paManifest, mut rest: &[u8], depth: usize) {
    if depth >= MAX_BOX_DEPTH {
        return;
    }
    while let Some((b, next)) = next_bmff_box(rest) {
        rest = next;
        if &b.kind == b"jumb"
            && let Some((desc, _)) = next_bmff_box(b.body)
            && &desc.kind == b"jumd"
            && let Some(hdr) = parse_jumd(desc.body)
            && !hdr.label.is_empty()
            && !manifest.assertions.contains(&hdr.label)
        {
            manifest.assertions.push(hdr.label);
        }
    }
}

fn read_signature_boxes(manifest: &mut C2paManifest, mut rest: &[u8]) {
    extract_der_certificates(rest, &mut manifest.signers);
    while let Some((b, next)) = next_bmff_box(rest) {
        rest = next;
        if &b.kind == b"cbor" || &b.kind == b"bfdb" || &b.kind == b"c2cs" {
            extract_der_certificates(b.body, &mut manifest.signers);
        }
    }
}

/// Scans `payload` (COSE_Sign1 `x5chain` bstr or CMS SignedData) for DER X.509 certificates.
fn extract_der_certificates(payload: &[u8], out: &mut Vec<C2paSigner>) {
    let limit = payload.len().min(512 * 1024);
    let mut idx = 0usize;
    while idx + 128 < limit {
        if payload.get(idx) == Some(&tag::SEQUENCE)
            && matches!(payload.get(idx + 1), Some(&0x81 | &0x82))
            && let Some(slice) = payload.get(idx..)
            && let Ok((tlv, _)) = Tlv::parse(slice)
            && tlv.raw.len() >= 128
            && let Ok(cert) = Certificate::parse(tlv.raw)
        {
            let fp = cert.fingerprint();
            if !out.iter().any(|s| s.sha256_fingerprint == fp) {
                out.push(C2paSigner {
                    subject: cert.display_name(),
                    issuer: cert.issuer.display_name(),
                    serial_hex: cert.serial_hex(),
                    valid_from: cert.not_before.to_pdf(),
                    valid_to: cert.not_after.to_pdf(),
                    self_signed: cert.subject.raw == cert.issuer.raw,
                    sha256_fingerprint: fp,
                });
            }
            idx = idx.saturating_add(tlv.raw.len());
            continue;
        }
        idx += 1;
    }
}

/// Bounded RFC 8949 CBOR reader extracting `(key, text_value)` pairs from C2PA claims.
fn parse_cbor_item<'a>(
    input: &'a [u8],
    depth: usize,
    current_key: Option<&str>,
    pairs: &mut Vec<(String, String)>,
) -> Option<(Option<String>, &'a [u8])> {
    if depth > MAX_CBOR_DEPTH {
        return None;
    }
    let &first = input.first()?;
    let major = first >> 5;
    let info = first & 0x1F;
    let (arg, rest) = read_cbor_arg(info, input.get(1..)?)?;
    match major {
        0 | 1 => Some((None, rest)),
        2 => {
            let len = usize::try_from(arg).ok()?;
            Some((None, rest.get(len..)?))
        }
        3 => {
            let len = usize::try_from(arg).ok()?;
            let text_bytes = rest.get(..len)?;
            let val = String::from_utf8_lossy(text_bytes).into_owned();
            if let Some(k) = current_key {
                pairs.push((k.to_string(), val.clone()));
            }
            Some((Some(val), rest.get(len..)?))
        }
        4 => {
            let count = usize::try_from(arg).ok()?.min(MAX_CBOR_ITEMS);
            let mut cur = rest;
            for _ in 0..count {
                cur = parse_cbor_item(cur, depth + 1, None, pairs)?.1;
            }
            Some((None, cur))
        }
        5 => {
            let count = usize::try_from(arg).ok()?.min(MAX_CBOR_ITEMS);
            let mut cur = rest;
            for _ in 0..count {
                let (key_opt, after_k) = parse_cbor_item(cur, depth + 1, None, pairs)?;
                let (_, after_v) = parse_cbor_item(after_k, depth + 1, key_opt.as_deref(), pairs)?;
                cur = after_v;
            }
            Some((None, cur))
        }
        6 => parse_cbor_item(rest, depth + 1, current_key, pairs),
        7 => Some((None, rest)),
        _ => None,
    }
}

fn read_cbor_arg(info: u8, rest: &[u8]) -> Option<(u64, &[u8])> {
    match info {
        0..=23 => Some((u64::from(info), rest)),
        24 => Some((u64::from(*rest.first()?), rest.get(1..)?)),
        25 => {
            let b = rest.get(..2)?;
            Some((u64::from(u16::from_be_bytes([b[0], b[1]])), rest.get(2..)?))
        }
        26 => {
            let b = rest.get(..4)?;
            Some((
                u64::from(u32::from_be_bytes([b[0], b[1], b[2], b[3]])),
                rest.get(4..)?,
            ))
        }
        27 => {
            let b = rest.get(..8)?;
            Some((
                u64::from_be_bytes([b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7]]),
                rest.get(8..)?,
            ))
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Name, PrivateKey, Time};
    use pdfcraft_cos::PdfString;

    fn bmff_box(kind: &[u8; 4], body: &[u8]) -> Vec<u8> {
        let len = (8 + body.len()) as u32;
        let mut out = Vec::with_capacity(8 + body.len());
        out.extend_from_slice(&len.to_be_bytes());
        out.extend_from_slice(kind);
        out.extend_from_slice(body);
        out
    }

    fn jumbf_superbox(uuid: [u8; 16], label: &str, children: &[Vec<u8>]) -> Vec<u8> {
        let mut jumd_body = Vec::new();
        jumd_body.extend_from_slice(&uuid);
        jumd_body.push(0x03);
        jumd_body.extend_from_slice(label.as_bytes());
        jumd_body.push(0);
        let mut super_body = bmff_box(b"jumd", &jumd_body);
        for c in children {
            super_body.extend_from_slice(c);
        }
        bmff_box(b"jumb", &super_body)
    }

    fn cbor_text(s: &str) -> Vec<u8> {
        let b = s.as_bytes();
        let mut out = Vec::new();
        if b.len() < 24 {
            out.push(0x60 | (b.len() as u8));
        } else {
            out.push(0x78);
            out.push(b.len() as u8);
        }
        out.extend_from_slice(b);
        out
    }

    fn sample_c2pa_store_bytes(cert_der: &[u8]) -> Vec<u8> {
        // CBOR map(3): {"claim_generator": "PdfCraft/0.21", "dc:title": "Press Proof", "alg": "es256"}
        let mut cbor_map = vec![0xA3];
        for (k, v) in [
            ("claim_generator", "PdfCraft/0.21"),
            ("dc:title", "Press Proof"),
            ("alg", "es256"),
        ] {
            cbor_map.extend_from_slice(&cbor_text(k));
            cbor_map.extend_from_slice(&cbor_text(v));
        }
        let claim = jumbf_superbox(
            C2PA_CLAIM_UUID,
            "c2pa.claim",
            &[bmff_box(b"cbor", &cbor_map)],
        );
        let act = jumbf_superbox([0u8; 16], "c2pa.actions", &[bmff_box(b"cbor", &[0xA0])]);
        let hash = jumbf_superbox([0u8; 16], "c2pa.hash.data", &[bmff_box(b"cbor", &[0xA0])]);
        let assertions = jumbf_superbox(C2PA_ASSERTIONS_UUID, "c2pa.assertions", &[act, hash]);

        let mut cose_like = vec![0xD2, 0x84, 0x59];
        cose_like.extend_from_slice(&(cert_der.len() as u16).to_be_bytes());
        cose_like.extend_from_slice(cert_der);
        let signature = jumbf_superbox(
            C2PA_SIGNATURE_UUID,
            "c2pa.signature",
            &[bmff_box(b"cbor", &cose_like)],
        );

        let manifest = jumbf_superbox(
            C2PA_MANIFEST_UUID,
            "urn:uuid:12345678-1234-5678-9abc-def012345678",
            &[assertions, claim, signature],
        );
        jumbf_superbox(C2PA_STORE_UUID, "c2pa", &[manifest])
    }

    #[test]
    fn empty_document_reports_no_credentials() {
        let doc = Document::new_empty();
        let report = inspect_content_credentials(&doc);
        assert!(!report.has_credentials());
        assert!(report.manifests.is_empty());
    }

    #[test]
    fn inspects_associated_file_c2pa_manifest_and_signer_certificate() {
        let key = PrivateKey::generate_p256();
        assert!(key.is_ok());
        let Ok(key) = key else { return };
        let subject = Name::build("Newsroom Camera", "", "Example Press", "", "KR");
        let start = Time {
            year: 2026,
            month: 1,
            day: 1,
            hour: 0,
            minute: 0,
            second: 0,
        };
        let cert = Certificate::self_signed(&subject, &key, start, 3, &[0x01, 0x02]);
        assert!(cert.is_ok());
        let Ok(cert) = cert else { return };

        let store_bytes = sample_c2pa_store_bytes(&cert.raw);
        let mut doc = Document::new_empty();
        let mut stream_dict = Dict::new();
        stream_dict.set(b"Subtype".to_vec(), Object::name("application#2Fc2pa"));
        let ef_stream = doc.add(Object::Stream(Stream::from_raw(stream_dict, store_bytes)));

        let mut ef_dict = Dict::new();
        ef_dict.set(b"UF".to_vec(), Object::Ref(ef_stream));
        ef_dict.set(b"F".to_vec(), Object::Ref(ef_stream));

        let mut filespec = Dict::new();
        filespec.set(b"Type".to_vec(), Object::name("Filespec"));
        filespec.set(b"AFRelationship".to_vec(), Object::name("Provenance"));
        filespec.set(b"UF".to_vec(), Object::String(PdfString::text("manifest.c2pa")));
        filespec.set(b"EF".to_vec(), Object::Dict(ef_dict));
        let fs_ref = doc.add(Object::Dict(filespec));

        if let Some(root) = doc.root() {
            let _ = doc.update_dict(root, |cat| {
                cat.set(b"AF".to_vec(), Object::Array(vec![Object::Ref(fs_ref)]));
            });
        }

        let report = inspect_content_credentials(&doc);
        assert!(report.has_credentials());
        assert_eq!(report.manifests.len(), 1);
        let m = &report.manifests[0];
        assert_eq!(m.label, "urn:uuid:12345678-1234-5678-9abc-def012345678");
        assert!(m.source.contains("manifest.c2pa"), "{}", m.source);
        assert_eq!(m.claim_generator.as_deref(), Some("PdfCraft/0.21"));
        assert_eq!(m.title.as_deref(), Some("Press Proof"));
        assert_eq!(m.digest_alg.as_deref(), Some("es256"));
        assert_eq!(m.assertions, vec!["c2pa.actions", "c2pa.hash.data"]);
        assert!(m.has_signature_box);
        assert_eq!(m.signers.len(), 1);
        assert_eq!(m.signers[0].subject, "Newsroom Camera");
        assert!(m.signers[0].self_signed);
    }

    #[test]
    fn reassembles_jpeg_app11_jumbf_and_rejects_truncated_boxes_safely() {
        let store = sample_c2pa_store_bytes(&[]);
        let mut jpeg = vec![0xFF, 0xD8, 0xFF, 0xEB];
        let seg_len = (2 + 8 + store.len()) as u16;
        jpeg.extend_from_slice(&seg_len.to_be_bytes());
        jpeg.extend_from_slice(b"JP\x00\x01\x00\x00\x00\x01");
        jpeg.extend_from_slice(&store);
        jpeg.extend_from_slice(&[0xFF, 0xD9]);

        let extracted = extract_jpeg_app11_jumbf(&jpeg);
        assert!(extracted.is_some());
        let mut out = Vec::new();
        if let Some(bytes) = extracted {
            parse_c2pa_jumbf_bytes(&bytes, "JPEG", &mut out);
        }
        assert_eq!(out.len(), 1);

        // Truncated and adversarial box lengths never panic.
        let mut malformed = Vec::new();
        parse_c2pa_jumbf_bytes(&[0xFF, 0xFF, 0xFF, 0xFF, b'j', b'u', b'm', b'b'], "bad", &mut malformed);
        parse_c2pa_jumbf_bytes(&[0, 0, 0, 1, b'j', b'u', b'm', b'b', 0xFF], "badxl", &mut malformed);
        assert!(malformed.is_empty());
    }
}
