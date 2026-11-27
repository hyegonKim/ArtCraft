//! Content Authenticity (`ACAI v0.5.3` / C2PA v2.1) provenance credentials and
//! the 19-app Craft Suite `Vulcan` cross-app message bus and deep-link router.

use std::collections::BTreeSet;
use std::fmt;

#[cfg(test)]
use strum::EnumCount;
#[cfg(test)]
use strum::EnumIter;

pub const C2PA_SPEC_VERSION: &str = "2.1";
pub const DEFAULT_CLAIM_GENERATOR: &str = "ArtCraftX/ACAI-0.5.3";
pub const VULCAN_PROTOCOL_VERSION: &str = "vulcan/2.0";
pub const DEEP_LINK_PREFIX: &str = "craft://open?";

const IPTC_SOURCE_PREFIX: &str = "http://cv.iptc.org/newscodes/digitalsourcetype/";
const XPACKET_BEGIN: &str = "<?xpacket begin=\"\u{feff}\" id=\"W5M0MpCehiHzreSzNTczkc9d\"?>";
const XPACKET_END: &str = "<?xpacket end=\"w\"?>";
const FNV1A_64_OFFSET_BASIS: u64 = 0xcbf2_9ce4_8422_2325;
const FNV1A_64_PRIME: u64 = 0x0100_0000_01b3;

const SHA256_INITIAL: [u32; 8] = [0x6a09_e667, 0xbb67_ae85, 0x3c6e_f372, 0xa54f_f53a, 0x510e_527f, 0x9b05_688c, 0x1f83_d9ab, 0x5be0_cd19];

const SHA256_K: [u32; 64] = [0x428a_2f98, 0x7137_4491, 0xb5c0_fbcf, 0xe9b5_dba5, 0x3956_c25b, 0x59f1_11f1, 0x923f_82a4, 0xab1c_5ed5, 0xd807_aa98, 0x1283_5b01, 0x2431_85be, 0x550c_7dc3, 0x72be_5d74, 0x80de_b1fe, 0x9bdc_06a7, 0xc19b_f174, 0xe49b_69c1, 0xefbe_4786, 0x0fc1_9dc6, 0x240c_a1cc, 0x2de9_2c6f, 0x4a74_84aa, 0x5cb0_a9dc, 0x76f9_88da, 0x983e_5152, 0xa831_c66d, 0xb003_27c8, 0xbf59_7fc7, 0xc6e0_0bf3, 0xd5a7_9147, 0x06ca_6351, 0x1429_2967, 0x27b7_0a85, 0x2e1b_2138, 0x4d2c_6dfc, 0x5338_0d13, 0x650a_7354, 0x766a_0abb, 0x81c2_c92e, 0x9272_2c85, 0xa2bf_e8a1, 0xa81a_664b, 0xc24b_8b70, 0xc76c_51a3, 0xd192_e819, 0xd699_0624, 0xf40e_3585, 0x106a_a070, 0x19a4_c116, 0x1e37_6c08, 0x2748_774c, 0x34b0_bcb5, 0x391c_0cb3, 0x4ed8_aa4a, 0x5b9c_ca4f, 0x682e_6ff3, 0x748f_82ee, 0x78a5_636f, 0x84c8_7814, 0x8cc7_0208, 0x90be_fffa, 0xa450_6ceb, 0xbef9_a3f7, 0xc671_78f2];

// Strongly-typed entity identifiers per core_types rules.
define_id!(pub struct C2paManifestId => "c2pa");
define_id!(pub struct VulcanMessageId => "vulcan");

/// C2PA v2.1 Content Credentials manifest tracking creation provenance, ingredient lineage,
/// hard content binding digests, cryptographic HMAC-SHA256 signatures, and XMP serialization.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct C2paContentCredentials {
  pub spec_version: String,
  pub manifest_id: C2paManifestId,
  pub claim_generator: String,
  pub digital_source_type: DigitalSourceType,
  pub actions: Vec<C2paAction>,
  pub ingredients: Vec<C2paIngredient>,
  pub binding: Option<ContentBindingDigest>,
  pub signature: Option<C2paSignature>,
}

/// Standard C2PA v2.1 assertion action names (`c2pa.actions`).
#[cfg_attr(test, derive(EnumIter, EnumCount))]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Ord, PartialOrd, Serialize, Deserialize)]
pub enum C2paActionKind {
  #[serde(rename = "c2pa.created")]
  Created,
  #[serde(rename = "c2pa.edited")]
  Edited,
  #[serde(rename = "c2pa.placed")]
  Placed,
  #[serde(rename = "c2pa.color_adjustments")]
  ColorAdjustments,
  #[serde(rename = "c2pa.transcoded")]
  Transcoded,
  #[serde(rename = "c2pa.repackaged")]
  Repackaged,
}

/// IPTC / C2PA v2.1 Digital Source Type classification for algorithmic vs. captured media.
#[cfg_attr(test, derive(EnumIter, EnumCount))]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Ord, PartialOrd, Serialize, Deserialize)]
pub enum DigitalSourceType {
  #[serde(rename = "trainedAlgorithmicMedia")]
  TrainedAlgorithmicMedia,
  #[serde(rename = "compositeWithTrainedAlgorithmicMedia")]
  CompositeWithTrainedAlgorithmicMedia,
  #[serde(rename = "digitalCapture")]
  DigitalCapture,
}

/// One recorded provenance event in `C2paContentCredentials::actions`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct C2paAction {
  pub action: C2paActionKind,
  pub software_agent: String,
  pub when_utc: String,
  pub digital_source_type: Option<DigitalSourceType>,
  pub description: Option<String>,
}

/// Relationship of a parent or placed asset inside `C2paContentCredentials::ingredients`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum IngredientRelationship {
  #[serde(rename = "parentOf")]
  ParentOf,
  #[serde(rename = "componentOf")]
  ComponentOf,
  #[serde(rename = "inputTo")]
  InputTo,
}

/// Parent or component asset linked in the C2PA ingredient chain (`c2pa.ingredient.v3`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct C2paIngredient {
  pub title: String,
  pub format: String,
  pub instance_id: String,
  pub relationship: IngredientRelationship,
  pub content_sha256: String,
  pub parent_manifest_id: Option<C2paManifestId>,
}

/// Deterministic dual-digest content binding (`c2pa.hash.data`) over asset payload bytes.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContentBindingDigest {
  pub sha256_hex: String,
  pub fnv1a64_hex: String,
  pub byte_length: u64,
}

/// Cryptographic claim signature binding the C2PA manifest metadata and content digest.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct C2paSignature {
  pub signer_id: String,
  pub hmac_sha256_hex: String,
  pub manifest_fnv1a64_hex: String,
}

/// All 19 applications in the Craft Suite with bidirectional Adobe Creative Cloud SAPCode routing.
#[cfg_attr(test, derive(EnumIter, EnumCount))]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Ord, PartialOrd, Serialize, Deserialize)]
pub enum CraftAppTarget {
  #[serde(rename = "artcraft")]
  ArtCraft,
  #[serde(rename = "photocraft")]
  PhotoCraft,
  #[serde(rename = "vectorcraft")]
  VectorCraft,
  #[serde(rename = "filmcraft")]
  FilmCraft,
  #[serde(rename = "lightcraft")]
  LightCraft,
  #[serde(rename = "pdfcraft")]
  PdfCraft,
  #[serde(rename = "effectcraft")]
  EffectCraft,
  #[serde(rename = "anicraft")]
  AniCraft,
  #[serde(rename = "designcraft")]
  DesignCraft,
  #[serde(rename = "figcraft")]
  FigCraft,
  #[serde(rename = "soundcraft")]
  SoundCraft,
  #[serde(rename = "wordcraft")]
  WordCraft,
  #[serde(rename = "deckcraft")]
  DeckCraft,
  #[serde(rename = "cadcraft")]
  CadCraft,
  #[serde(rename = "gridcraft")]
  GridCraft,
  #[serde(rename = "texturecraft")]
  TextureCraft,
  #[serde(rename = "materialcraft")]
  MaterialCraft,
  #[serde(rename = "solidcraft")]
  SolidCraft,
  #[serde(rename = "craftanything")]
  CraftAnything,
}

/// Parsed `craft://open?app=...&path=...` cross-app launch deep link.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CraftDeepLink {
  pub app: CraftAppTarget,
  pub path: String,
  pub action: Option<String>,
}

/// Clean-room Adobe `Vulcan` cross-app IPC message envelope for the 19-app Craft Suite.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct VulcanEnvelope {
  pub message_id: VulcanMessageId,
  pub protocol_version: String,
  pub sender: CraftAppTarget,
  pub recipient: Option<CraftAppTarget>,
  pub timestamp_ms: u64,
  pub payload: VulcanPayload,
}

/// Strongly-typed cross-application payload dispatched over the `Vulcan` message bus.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum VulcanPayload {
  OpenDocument { path: String, read_only: bool, page_or_frame: Option<u32> },
  PlaceAsset { asset_path: String, mime_type: String, x: f64, y: f64, credentials_id: Option<C2paManifestId> },
  SyncSwatchPalette { palette_name: String, colors_srgb_hex: Vec<String> },
  ApplyBrandKit { kit_id: String, primary_font_family: String, accent_hex: String, logo_path: Option<String> },
  UpdateLinkedSmartObject { link_id: String, rendered_path: String, content_sha256: String },
}

// Implement Eq manually for VulcanPayload (f64 coordinates checked via finite IEEE bits after validation).
impl Eq for VulcanPayload {}

/// Errors produced by C2PA signing/verification, XMP serialization, deep-link parsing, or Vulcan validation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ProvenanceError {
  EmptySecretKey,
  EmptySignerId,
  UnsignedCredentials,
  DigestMismatch,
  SignatureMismatch,
  UnknownAction(String),
  UnknownDigitalSourceType(String),
  UnknownIngredientRelationship(String),
  UnknownAppTarget(String),
  InvalidSha256Hex(String),
  InvalidXmpPacket(String),
  InvalidDeepLink(String),
  InvalidVulcanPayload(String),
}

impl fmt::Display for ProvenanceError {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      Self::EmptySecretKey => f.write_str("signing key must not be empty"),
      Self::EmptySignerId => f.write_str("signer identifier must not be empty"),
      Self::UnsignedCredentials => f.write_str("C2PA content credentials have no binding or signature"),
      Self::DigestMismatch => f.write_str("payload digest does not match C2PA content binding"),
      Self::SignatureMismatch => f.write_str("C2PA claim signature verification failed"),
      Self::UnknownAction(val) => write!(f, "unknown C2PA action '{val}'"),
      Self::UnknownDigitalSourceType(val) => write!(f, "unknown C2PA digital source type '{val}'"),
      Self::UnknownIngredientRelationship(val) => write!(f, "unknown C2PA ingredient relationship '{val}'"),
      Self::UnknownAppTarget(val) => write!(f, "unknown Craft Suite application or SAPCode '{val}'"),
      Self::InvalidSha256Hex(val) => write!(f, "expected 64 lowercase hex digits for SHA-256, got '{val}'"),
      Self::InvalidXmpPacket(reason) => write!(f, "invalid Adobe XMP provenance packet: {reason}"),
      Self::InvalidDeepLink(reason) => write!(f, "invalid craft:// deep link: {reason}"),
      Self::InvalidVulcanPayload(reason) => write!(f, "invalid Vulcan cross-app payload: {reason}"),
    }
  }
}

impl std::error::Error for ProvenanceError {}

impl C2paActionKind {
  pub const ALL: [Self; 6] = [Self::Created, Self::Edited, Self::Placed, Self::ColorAdjustments, Self::Transcoded, Self::Repackaged];

  pub fn as_str(self) -> &'static str {
    match self {
      Self::Created => "c2pa.created",
      Self::Edited => "c2pa.edited",
      Self::Placed => "c2pa.placed",
      Self::ColorAdjustments => "c2pa.color_adjustments",
      Self::Transcoded => "c2pa.transcoded",
      Self::Repackaged => "c2pa.repackaged",
    }
  }

  pub fn from_c2pa_str(value: &str) -> Result<Self, ProvenanceError> {
    match value {
      "c2pa.created" => Ok(Self::Created),
      "c2pa.edited" => Ok(Self::Edited),
      "c2pa.placed" => Ok(Self::Placed),
      "c2pa.color_adjustments" => Ok(Self::ColorAdjustments),
      "c2pa.transcoded" => Ok(Self::Transcoded),
      "c2pa.repackaged" => Ok(Self::Repackaged),
      _ => Err(ProvenanceError::UnknownAction(value.to_string())),
    }
  }
}

impl DigitalSourceType {
  pub const ALL: [Self; 3] = [Self::TrainedAlgorithmicMedia, Self::CompositeWithTrainedAlgorithmicMedia, Self::DigitalCapture];

  pub fn as_str(self) -> &'static str {
    match self {
      Self::TrainedAlgorithmicMedia => "trainedAlgorithmicMedia",
      Self::CompositeWithTrainedAlgorithmicMedia => "compositeWithTrainedAlgorithmicMedia",
      Self::DigitalCapture => "digitalCapture",
    }
  }

  pub fn iptc_uri(self) -> &'static str {
    match self {
      Self::TrainedAlgorithmicMedia => "http://cv.iptc.org/newscodes/digitalsourcetype/trainedAlgorithmicMedia",
      Self::CompositeWithTrainedAlgorithmicMedia => "http://cv.iptc.org/newscodes/digitalsourcetype/compositeWithTrainedAlgorithmicMedia",
      Self::DigitalCapture => "http://cv.iptc.org/newscodes/digitalsourcetype/digitalCapture",
    }
  }

  pub fn parse_source_type(value: &str) -> Result<Self, ProvenanceError> {
    let trimmed = value.strip_prefix(IPTC_SOURCE_PREFIX).unwrap_or(value);
    match trimmed {
      "trainedAlgorithmicMedia" => Ok(Self::TrainedAlgorithmicMedia),
      "compositeWithTrainedAlgorithmicMedia" => Ok(Self::CompositeWithTrainedAlgorithmicMedia),
      "digitalCapture" => Ok(Self::DigitalCapture),
      _ => Err(ProvenanceError::UnknownDigitalSourceType(value.to_string())),
    }
  }
}

impl IngredientRelationship {
  pub fn as_str(self) -> &'static str {
    match self {
      Self::ParentOf => "parentOf",
      Self::ComponentOf => "componentOf",
      Self::InputTo => "inputTo",
    }
  }

  pub fn from_rel_str(value: &str) -> Result<Self, ProvenanceError> {
    match value {
      "parentOf" => Ok(Self::ParentOf),
      "componentOf" => Ok(Self::ComponentOf),
      "inputTo" => Ok(Self::InputTo),
      _ => Err(ProvenanceError::UnknownIngredientRelationship(value.to_string())),
    }
  }
}

impl C2paAction {
  pub fn new(kind: C2paActionKind, agent: impl Into<String>, timestamp_utc: impl Into<String>, source_type: Option<DigitalSourceType>, detail: Option<String>) -> Self {
    Self { action: kind, software_agent: agent.into(), when_utc: timestamp_utc.into(), digital_source_type: source_type, description: detail }
  }

  pub fn for_app(kind: C2paActionKind, app: CraftAppTarget, timestamp_utc: impl Into<String>, source_type: Option<DigitalSourceType>) -> Self {
    Self::new(kind, format!("{} ({})", app.display_name(), app.sap_code()), timestamp_utc, source_type, None)
  }
}

impl C2paIngredient {
  pub fn try_new(asset_title: impl Into<String>, mime_format: impl Into<String>, iid: impl Into<String>, rel: IngredientRelationship, digest_hex: impl Into<String>, parent_manifest: Option<C2paManifestId>) -> Result<Self, ProvenanceError> {
    let sha_str = digest_hex.into();
    if !is_lower_sha256_hex(&sha_str) {
      return Err(ProvenanceError::InvalidSha256Hex(sha_str));
    }
    Ok(Self { title: asset_title.into(), format: mime_format.into(), instance_id: iid.into(), relationship: rel, content_sha256: sha_str, parent_manifest_id: parent_manifest })
  }
}

impl ContentBindingDigest {
  /// Compute deterministic FIPS 180-4 SHA-256 and 64-bit FNV-1a digests over `payload`.
  pub fn compute(payload: &[u8]) -> Self {
    let sha_bytes = sha256_digest(payload);
    let fnv_val = fnv1a_64(payload);
    Self { sha256_hex: hex_encode(&sha_bytes), fnv1a64_hex: format!("{fnv_val:016x}"), byte_length: payload.len() as u64 }
  }

  /// Verify that `payload` matches every digest and length field in constant time.
  pub fn matches_payload(&self, payload: &[u8]) -> bool {
    let actual = Self::compute(payload);
    actual.byte_length == self.byte_length && constant_time_eq(actual.sha256_hex.as_bytes(), self.sha256_hex.as_bytes()) && constant_time_eq(actual.fnv1a64_hex.as_bytes(), self.fnv1a64_hex.as_bytes())
  }
}

impl C2paContentCredentials {
  pub fn new(generator: impl Into<String>, source_type: DigitalSourceType) -> Self {
    Self::with_manifest_id(C2paManifestId::generate(), generator, source_type)
  }

  pub fn with_manifest_id(id_val: C2paManifestId, generator: impl Into<String>, source_type: DigitalSourceType) -> Self {
    Self { spec_version: C2PA_SPEC_VERSION.to_string(), manifest_id: id_val, claim_generator: generator.into(), digital_source_type: source_type, actions: Vec::new(), ingredients: Vec::new(), binding: None, signature: None }
  }

  pub fn manifest_urn(&self) -> String {
    format!("urn:c2pa:{}", self.manifest_id.as_str())
  }

  pub fn record_action(&mut self, act: C2paAction) {
    self.actions.push(act);
    // Mutating claim assertions invalidates any prior signature until re-signed.
    self.signature = None;
  }

  pub fn add_ingredient(&mut self, ing: C2paIngredient) {
    self.ingredients.push(ing);
    self.signature = None;
  }

  pub fn parent_ingredients(&self) -> Vec<&C2paIngredient> {
    self.ingredients.iter().filter(|i| i.relationship == IngredientRelationship::ParentOf).collect()
  }

  /// Compute the hard content binding over `payload` and sign the canonical C2PA claim
  /// using HMAC-SHA256 + FNV-1a-64.
  pub fn sign_payload(&mut self, payload: &[u8], secret_key: &[u8], signer: impl Into<String>) -> Result<C2paSignature, ProvenanceError> {
    if secret_key.is_empty() {
      return Err(ProvenanceError::EmptySecretKey);
    }
    let signer_str = signer.into();
    if signer_str.trim().is_empty() {
      return Err(ProvenanceError::EmptySignerId);
    }

    let computed_binding = ContentBindingDigest::compute(payload);
    self.binding = Some(computed_binding.clone());

    let canonical = self.canonical_claim_bytes(&computed_binding, &signer_str);
    let mac = hmac_sha256(secret_key, &canonical);
    let fnv = fnv1a_64(&canonical);

    let sig = C2paSignature { signer_id: signer_str, hmac_sha256_hex: hex_encode(&mac), manifest_fnv1a64_hex: format!("{fnv:016x}") };
    self.signature = Some(sig.clone());
    Ok(sig)
  }

  /// Verify both the content binding digest (`SHA-256` + `FNV-1a` + byte length) and the
  /// cryptographic HMAC-SHA256 claim signature over the manifest metadata.
  pub fn verify_payload(&self, payload: &[u8], secret_key: &[u8]) -> Result<(), ProvenanceError> {
    if secret_key.is_empty() {
      return Err(ProvenanceError::EmptySecretKey);
    }
    let (Some(bound), Some(sig)) = (&self.binding, &self.signature) else {
      return Err(ProvenanceError::UnsignedCredentials);
    };

    if !bound.matches_payload(payload) {
      return Err(ProvenanceError::DigestMismatch);
    }

    let canonical = self.canonical_claim_bytes(bound, &sig.signer_id);
    let expected_mac_hex = hex_encode(&hmac_sha256(secret_key, &canonical));
    let expected_fnv_hex = format!("{:016x}", fnv1a_64(&canonical));

    let mac_ok = constant_time_eq(expected_mac_hex.as_bytes(), sig.hmac_sha256_hex.as_bytes());
    let fnv_ok = constant_time_eq(expected_fnv_hex.as_bytes(), sig.manifest_fnv1a64_hex.as_bytes());
    if !mac_ok || !fnv_ok {
      return Err(ProvenanceError::SignatureMismatch);
    }
    Ok(())
  }

  /// Serialize this C2PA credential set into an Adobe XMP metadata packet containing
  /// `<dcterms:provenance>` and `<c2pa:manifest>`.
  pub fn to_xmp_packet(&self) -> Result<String, ProvenanceError> {
    let mut xml = String::with_capacity(1024);
    xml.push_str(XPACKET_BEGIN);
    xml.push('\n');
    xml.push_str("<x:xmpmeta xmlns:x=\"adobe:ns:meta/\">\n");
    xml.push_str("  <rdf:RDF xmlns:rdf=\"http://www.w3.org/1999/02/22-rdf-syntax-ns#\" xmlns:dcterms=\"http://purl.org/dc/terms/\" xmlns:c2pa=\"http://c2pa.org/manifest/2.1/\">\n");
    xml.push_str("    <rdf:Description rdf:about=\"\">\n");
    xml.push_str(&format!("      <dcterms:provenance>{}</dcterms:provenance>\n", xml_escape(&self.manifest_urn())));
    xml.push_str(&format!("      <c2pa:manifest specVersion=\"{}\" manifestId=\"{}\" claimGenerator=\"{}\" digitalSourceType=\"{}\">\n", xml_escape(&self.spec_version), xml_escape(self.manifest_id.as_str()), xml_escape(&self.claim_generator), xml_escape(self.digital_source_type.iptc_uri()),));

    for act in &self.actions {
      xml.push_str(&format!("        <c2pa:action kind=\"{}\" agent=\"{}\" when=\"{}\" sourceType=\"{}\" description=\"{}\"/>\n", xml_escape(act.action.as_str()), xml_escape(&act.software_agent), xml_escape(&act.when_utc), xml_escape(act.digital_source_type.map_or("", DigitalSourceType::as_str)), xml_escape(act.description.as_deref().unwrap_or("")),));
    }

    for ing in &self.ingredients {
      xml.push_str(&format!("        <c2pa:ingredient title=\"{}\" format=\"{}\" instanceId=\"{}\" relationship=\"{}\" sha256=\"{}\" parentManifestId=\"{}\"/>\n", xml_escape(&ing.title), xml_escape(&ing.format), xml_escape(&ing.instance_id), xml_escape(ing.relationship.as_str()), xml_escape(&ing.content_sha256), xml_escape(ing.parent_manifest_id.as_ref().map_or("", C2paManifestId::as_str)),));
    }

    if let Some(bound) = &self.binding {
      xml.push_str(&format!("        <c2pa:binding sha256=\"{}\" fnv1a64=\"{}\" byteLength=\"{}\"/>\n", xml_escape(&bound.sha256_hex), xml_escape(&bound.fnv1a64_hex), bound.byte_length,));
    }

    if let Some(sig) = &self.signature {
      xml.push_str(&format!("        <c2pa:signature signerId=\"{}\" hmacSha256=\"{}\" manifestFnv1a64=\"{}\"/>\n", xml_escape(&sig.signer_id), xml_escape(&sig.hmac_sha256_hex), xml_escape(&sig.manifest_fnv1a64_hex),));
    }

    xml.push_str("      </c2pa:manifest>\n");
    xml.push_str("    </rdf:Description>\n");
    xml.push_str("  </rdf:RDF>\n");
    xml.push_str("</x:xmpmeta>\n");
    xml.push_str(XPACKET_END);
    Ok(xml)
  }

  /// Parse and validate an Adobe XMP provenance packet produced by [`Self::to_xmp_packet`].
  pub fn from_xmp_packet(xmp: &str) -> Result<Self, ProvenanceError> {
    if !xmp.contains("<x:xmpmeta") || !xmp.contains("</x:xmpmeta>") {
      return Err(ProvenanceError::InvalidXmpPacket("missing <x:xmpmeta> envelope".to_string()));
    }

    let prov_urn = extract_element_text(xmp, "dcterms:provenance").ok_or_else(|| ProvenanceError::InvalidXmpPacket("missing <dcterms:provenance>".to_string()))?;
    let prov_id_raw = prov_urn.strip_prefix("urn:c2pa:").ok_or_else(|| ProvenanceError::InvalidXmpPacket("dcterms:provenance must start with urn:c2pa:".to_string()))?;

    let (manifest_open_tag, manifest_inner) = extract_open_tag_and_body(xmp, "c2pa:manifest").ok_or_else(|| ProvenanceError::InvalidXmpPacket("missing <c2pa:manifest>".to_string()))?;

    let spec_ver = extract_xml_attr(manifest_open_tag, "specVersion").ok_or_else(|| ProvenanceError::InvalidXmpPacket("missing specVersion".to_string()))?;
    let manifest_id_attr = extract_xml_attr(manifest_open_tag, "manifestId").ok_or_else(|| ProvenanceError::InvalidXmpPacket("missing manifestId".to_string()))?;
    if manifest_id_attr != prov_id_raw {
      return Err(ProvenanceError::InvalidXmpPacket("dcterms:provenance URN disagrees with c2pa:manifest manifestId".to_string()));
    }
    let parsed_manifest_id: C2paManifestId = manifest_id_attr.parse().map_err(|e| ProvenanceError::InvalidXmpPacket(format!("bad manifestId: {e}")))?;

    let generator = extract_xml_attr(manifest_open_tag, "claimGenerator").ok_or_else(|| ProvenanceError::InvalidXmpPacket("missing claimGenerator".to_string()))?;
    let dst_raw = extract_xml_attr(manifest_open_tag, "digitalSourceType").ok_or_else(|| ProvenanceError::InvalidXmpPacket("missing digitalSourceType".to_string()))?;
    let dst = DigitalSourceType::parse_source_type(&dst_raw)?;

    let mut parsed_actions = Vec::new();
    for tag in find_self_closing_tags(manifest_inner, "c2pa:action") {
      let kind_str = extract_xml_attr(tag, "kind").ok_or_else(|| ProvenanceError::InvalidXmpPacket("action missing kind".to_string()))?;
      let agent = extract_xml_attr(tag, "agent").ok_or_else(|| ProvenanceError::InvalidXmpPacket("action missing agent".to_string()))?;
      let when = extract_xml_attr(tag, "when").ok_or_else(|| ProvenanceError::InvalidXmpPacket("action missing when".to_string()))?;
      let src_opt = match extract_xml_attr(tag, "sourceType") {
        Some(s) if !s.is_empty() => Some(DigitalSourceType::parse_source_type(&s)?),
        _ => None,
      };
      let desc_opt = extract_xml_attr(tag, "description").filter(|s| !s.is_empty());
      parsed_actions.push(C2paAction::new(C2paActionKind::from_c2pa_str(&kind_str)?, agent, when, src_opt, desc_opt));
    }

    let mut parsed_ingredients = Vec::new();
    for tag in find_self_closing_tags(manifest_inner, "c2pa:ingredient") {
      let title = extract_xml_attr(tag, "title").ok_or_else(|| ProvenanceError::InvalidXmpPacket("ingredient missing title".to_string()))?;
      let fmt_val = extract_xml_attr(tag, "format").ok_or_else(|| ProvenanceError::InvalidXmpPacket("ingredient missing format".to_string()))?;
      let iid = extract_xml_attr(tag, "instanceId").ok_or_else(|| ProvenanceError::InvalidXmpPacket("ingredient missing instanceId".to_string()))?;
      let rel_str = extract_xml_attr(tag, "relationship").ok_or_else(|| ProvenanceError::InvalidXmpPacket("ingredient missing relationship".to_string()))?;
      let sha = extract_xml_attr(tag, "sha256").ok_or_else(|| ProvenanceError::InvalidXmpPacket("ingredient missing sha256".to_string()))?;
      let parent_id = match extract_xml_attr(tag, "parentManifestId") {
        Some(s) if !s.is_empty() => Some(s.parse::<C2paManifestId>().map_err(|e| ProvenanceError::InvalidXmpPacket(format!("bad parentManifestId: {e}")))?),
        _ => None,
      };
      parsed_ingredients.push(C2paIngredient::try_new(title, fmt_val, iid, IngredientRelationship::from_rel_str(&rel_str)?, sha, parent_id)?);
    }

    let parsed_binding = if let Some(b_tag) = find_self_closing_tags(manifest_inner, "c2pa:binding").into_iter().next() {
      let sha = extract_xml_attr(b_tag, "sha256").ok_or_else(|| ProvenanceError::InvalidXmpPacket("binding missing sha256".to_string()))?;
      if !is_lower_sha256_hex(&sha) {
        return Err(ProvenanceError::InvalidSha256Hex(sha));
      }
      let fnv = extract_xml_attr(b_tag, "fnv1a64").ok_or_else(|| ProvenanceError::InvalidXmpPacket("binding missing fnv1a64".to_string()))?;
      let len_str = extract_xml_attr(b_tag, "byteLength").ok_or_else(|| ProvenanceError::InvalidXmpPacket("binding missing byteLength".to_string()))?;
      let byte_len = len_str.parse::<u64>().map_err(|_| ProvenanceError::InvalidXmpPacket("invalid binding byteLength".to_string()))?;
      Some(ContentBindingDigest { sha256_hex: sha, fnv1a64_hex: fnv, byte_length: byte_len })
    } else {
      None
    };

    let parsed_signature = if let Some(s_tag) = find_self_closing_tags(manifest_inner, "c2pa:signature").into_iter().next() {
      let signer = extract_xml_attr(s_tag, "signerId").ok_or_else(|| ProvenanceError::InvalidXmpPacket("signature missing signerId".to_string()))?;
      let hmac = extract_xml_attr(s_tag, "hmacSha256").ok_or_else(|| ProvenanceError::InvalidXmpPacket("signature missing hmacSha256".to_string()))?;
      let m_fnv = extract_xml_attr(s_tag, "manifestFnv1a64").ok_or_else(|| ProvenanceError::InvalidXmpPacket("signature missing manifestFnv1a64".to_string()))?;
      Some(C2paSignature { signer_id: signer, hmac_sha256_hex: hmac, manifest_fnv1a64_hex: m_fnv })
    } else {
      None
    };

    Ok(Self { spec_version: spec_ver, manifest_id: parsed_manifest_id, claim_generator: generator, digital_source_type: dst, actions: parsed_actions, ingredients: parsed_ingredients, binding: parsed_binding, signature: parsed_signature })
  }

  fn canonical_claim_bytes(&self, bound: &ContentBindingDigest, signer: &str) -> Vec<u8> {
    let mut out = String::with_capacity(512);
    out.push_str(&format!("v={}|id={}|gen={}|dst={}|signer={}|sha256={}|fnv={}|len={}", self.spec_version, self.manifest_id.as_str(), self.claim_generator, self.digital_source_type.as_str(), signer, bound.sha256_hex, bound.fnv1a64_hex, bound.byte_length,));
    for act in &self.actions {
      out.push_str(&format!("|act:{}:{}:{}:{}:{}", act.action.as_str(), act.software_agent, act.when_utc, act.digital_source_type.map_or("-", DigitalSourceType::as_str), act.description.as_deref().unwrap_or("-"),));
    }
    for ing in &self.ingredients {
      out.push_str(&format!("|ing:{}:{}:{}:{}:{}:{}", ing.title, ing.format, ing.instance_id, ing.relationship.as_str(), ing.content_sha256, ing.parent_manifest_id.as_ref().map_or("-", C2paManifestId::as_str),));
    }
    out.into_bytes()
  }
}

impl CraftAppTarget {
  /// Every application in the 19-app Craft Suite.
  pub const ALL: [Self; 19] = [Self::ArtCraft, Self::PhotoCraft, Self::VectorCraft, Self::FilmCraft, Self::LightCraft, Self::PdfCraft, Self::EffectCraft, Self::AniCraft, Self::DesignCraft, Self::FigCraft, Self::SoundCraft, Self::WordCraft, Self::DeckCraft, Self::CadCraft, Self::GridCraft, Self::TextureCraft, Self::MaterialCraft, Self::SolidCraft, Self::CraftAnything];

  pub fn all_variants() -> BTreeSet<Self> {
    BTreeSet::from(Self::ALL)
  }

  /// Lowercase URI and config slug (e.g. `"photocraft"`).
  pub fn slug(self) -> &'static str {
    match self {
      Self::ArtCraft => "artcraft",
      Self::PhotoCraft => "photocraft",
      Self::VectorCraft => "vectorcraft",
      Self::FilmCraft => "filmcraft",
      Self::LightCraft => "lightcraft",
      Self::PdfCraft => "pdfcraft",
      Self::EffectCraft => "effectcraft",
      Self::AniCraft => "anicraft",
      Self::DesignCraft => "designcraft",
      Self::FigCraft => "figcraft",
      Self::SoundCraft => "soundcraft",
      Self::WordCraft => "wordcraft",
      Self::DeckCraft => "deckcraft",
      Self::CadCraft => "cadcraft",
      Self::GridCraft => "gridcraft",
      Self::TextureCraft => "texturecraft",
      Self::MaterialCraft => "materialcraft",
      Self::SolidCraft => "solidcraft",
      Self::CraftAnything => "craftanything",
    }
  }

  /// Human-readable Craft Suite product title (e.g. `"PhotoCraft"`).
  pub fn display_name(self) -> &'static str {
    match self {
      Self::ArtCraft => "ArtCraft",
      Self::PhotoCraft => "PhotoCraft",
      Self::VectorCraft => "VectorCraft",
      Self::FilmCraft => "FilmCraft",
      Self::LightCraft => "LightCraft",
      Self::PdfCraft => "PdfCraft",
      Self::EffectCraft => "EffectCraft",
      Self::AniCraft => "AniCraft",
      Self::DesignCraft => "DesignCraft",
      Self::FigCraft => "FigCraft",
      Self::SoundCraft => "SoundCraft",
      Self::WordCraft => "WordCraft",
      Self::DeckCraft => "DeckCraft",
      Self::CadCraft => "CadCraft",
      Self::GridCraft => "GridCraft",
      Self::TextureCraft => "TextureCraft",
      Self::MaterialCraft => "MaterialCraft",
      Self::SolidCraft => "SolidCraft",
      Self::CraftAnything => "CraftAnything",
    }
  }

  /// Canonical Adobe Creative Cloud / Vulcan SAPCode equivalent for cross-app routing.
  pub fn sap_code(self) -> &'static str {
    match self {
      Self::ArtCraft => "ACCC",
      Self::PhotoCraft => "PHSP",
      Self::VectorCraft => "ILST",
      Self::FilmCraft => "PPRO",
      Self::LightCraft => "LTRM",
      Self::PdfCraft => "APRO",
      Self::EffectCraft => "AEFT",
      Self::AniCraft => "FLPR",
      Self::DesignCraft => "IDSN",
      Self::FigCraft => "SPRK",
      Self::SoundCraft => "AUDT",
      Self::WordCraft => "AICY",
      Self::DeckCraft => "PSXUWP",
      Self::CadCraft => "CADC",
      Self::GridCraft => "GRID",
      Self::TextureCraft => "SBSTP",
      Self::MaterialCraft => "SBSTD",
      Self::SolidCraft => "SHPR",
      Self::CraftAnything => "CANY",
    }
  }

  /// Parse a Craft Suite app from its lowercase/camel slug (e.g. `"photocraft"`, `"PhotoCraft"`).
  pub fn from_slug(value: &str) -> Result<Self, ProvenanceError> {
    let normalized: String = value.trim().chars().filter(|c| *c != '-' && *c != '_').map(|c| c.to_ascii_lowercase()).collect();
    match normalized.as_str() {
      "artcraft" | "artcraftx" => Ok(Self::ArtCraft),
      "photocraft" => Ok(Self::PhotoCraft),
      "vectorcraft" => Ok(Self::VectorCraft),
      "filmcraft" => Ok(Self::FilmCraft),
      "lightcraft" => Ok(Self::LightCraft),
      "pdfcraft" => Ok(Self::PdfCraft),
      "effectcraft" => Ok(Self::EffectCraft),
      "anicraft" => Ok(Self::AniCraft),
      "designcraft" => Ok(Self::DesignCraft),
      "figcraft" => Ok(Self::FigCraft),
      "soundcraft" => Ok(Self::SoundCraft),
      "wordcraft" => Ok(Self::WordCraft),
      "deckcraft" => Ok(Self::DeckCraft),
      "cadcraft" => Ok(Self::CadCraft),
      "gridcraft" => Ok(Self::GridCraft),
      "texturecraft" => Ok(Self::TextureCraft),
      "materialcraft" => Ok(Self::MaterialCraft),
      "solidcraft" => Ok(Self::SolidCraft),
      "craftanything" => Ok(Self::CraftAnything),
      _ => Err(ProvenanceError::UnknownAppTarget(value.to_string())),
    }
  }

  /// Map an Adobe Creative Cloud / Substance / Vulcan SAPCode (`PHSP`, `ILST`, `PPRO`, `LTRM`,
  /// `APRO`, `AEFT`, `FLPR`, `IDSN`, `SPRK`, `AUDT`, `AICY`, `PSXUWP`, `SHPR`, `SBSTP`, `SBSTD`, etc.)
  /// to its [`CraftAppTarget`].
  pub fn from_sap_code(sap: &str) -> Result<Self, ProvenanceError> {
    match sap.trim().to_ascii_uppercase().as_str() {
      "ACCC" | "ACFT" | "CCXP" => Ok(Self::ArtCraft),
      "PHSP" | "PS" => Ok(Self::PhotoCraft),
      "ILST" | "AI" => Ok(Self::VectorCraft),
      "PPRO" | "PP" => Ok(Self::FilmCraft),
      "LTRM" | "LRCC" | "KBRG" => Ok(Self::LightCraft),
      "APRO" | "ACRO" => Ok(Self::PdfCraft),
      "AEFT" | "AE" => Ok(Self::EffectCraft),
      "FLPR" | "AN" | "CHAR" => Ok(Self::AniCraft),
      "IDSN" | "ID" => Ok(Self::DesignCraft),
      "SPRK" | "XD" => Ok(Self::FigCraft),
      "AUDT" | "AU" => Ok(Self::SoundCraft),
      "AICY" | "IC" => Ok(Self::WordCraft),
      "PSXUWP" | "PRSN" => Ok(Self::DeckCraft),
      "CADC" | "A3D" => Ok(Self::CadCraft),
      "GRID" | "CFML" => Ok(Self::GridCraft),
      "SBSTP" => Ok(Self::TextureCraft),
      "SBSTD" | "SBSTA" => Ok(Self::MaterialCraft),
      "SHPR" | "SBSTS" => Ok(Self::SolidCraft),
      "CANY" | "UXPD" => Ok(Self::CraftAnything),
      _ => Err(ProvenanceError::UnknownAppTarget(sap.to_string())),
    }
  }

  /// Resolve either a Craft slug or an Adobe SAPCode.
  pub fn parse_app_or_sap(token: &str) -> Result<Self, ProvenanceError> {
    Self::from_slug(token).or_else(|_| Self::from_sap_code(token))
  }
}

impl fmt::Display for CraftAppTarget {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.write_str(self.slug())
  }
}

impl CraftDeepLink {
  pub fn new(target_app: CraftAppTarget, file_path: impl Into<String>) -> Self {
    Self { app: target_app, path: file_path.into(), action: None }
  }

  pub fn with_action(mut self, action_name: impl Into<String>) -> Self {
    self.action = Some(action_name.into());
    self
  }

  /// Build a percent-encoded `craft://open?app=<slug>&path=<encoded_path>[&action=...]` URI.
  pub fn to_uri(&self) -> Result<String, ProvenanceError> {
    validate_non_empty_clean_str(&self.path, "deep-link path").map_err(ProvenanceError::InvalidDeepLink)?;
    let mut uri = format!("{DEEP_LINK_PREFIX}app={}&path={}", self.app.slug(), percent_encode_query_value(&self.path),);
    if let Some(act) = &self.action {
      validate_non_empty_clean_str(act, "deep-link action").map_err(ProvenanceError::InvalidDeepLink)?;
      uri.push_str("&action=");
      uri.push_str(&percent_encode_query_value(act));
    }
    Ok(uri)
  }

  /// Convenience builder returning a formatted `craft://open?app=...&path=...` string.
  pub fn build(target_app: CraftAppTarget, file_path: &str) -> Result<String, ProvenanceError> {
    Self::new(target_app, file_path).to_uri()
  }

  /// Parse and validate a `craft://open?app=...&path=...` URI (accepting slugs or SAPCodes in `app`).
  pub fn parse(uri: &str) -> Result<Self, ProvenanceError> {
    let query = uri.strip_prefix(DEEP_LINK_PREFIX).ok_or_else(|| ProvenanceError::InvalidDeepLink("URI must start with craft://open?".to_string()))?;

    let mut found_app: Option<CraftAppTarget> = None;
    let mut found_path: Option<String> = None;
    let mut found_action: Option<String> = None;

    for pair in query.split('&') {
      if pair.is_empty() {
        continue;
      }
      let (raw_key, raw_val) = pair.split_once('=').ok_or_else(|| ProvenanceError::InvalidDeepLink(format!("malformed query parameter '{pair}'")))?;
      let decoded_val = percent_decode_query_value(raw_val)?;
      match raw_key {
        "app" => {
          found_app = Some(CraftAppTarget::parse_app_or_sap(&decoded_val)?);
        },
        "path" => {
          validate_non_empty_clean_str(&decoded_val, "deep-link path").map_err(ProvenanceError::InvalidDeepLink)?;
          found_path = Some(decoded_val);
        },
        "action" => {
          validate_non_empty_clean_str(&decoded_val, "deep-link action").map_err(ProvenanceError::InvalidDeepLink)?;
          found_action = Some(decoded_val);
        },
        _ => {},
      }
    }

    let app_target = found_app.ok_or_else(|| ProvenanceError::InvalidDeepLink("missing 'app' query parameter".to_string()))?;
    let doc_path = found_path.ok_or_else(|| ProvenanceError::InvalidDeepLink("missing 'path' query parameter".to_string()))?;

    Ok(Self { app: app_target, path: doc_path, action: found_action })
  }
}

impl VulcanEnvelope {
  /// Create a point-to-point cross-app Vulcan message directed to `target_app`.
  pub fn new_directed(from_app: CraftAppTarget, target_app: CraftAppTarget, sent_at_ms: u64, body: VulcanPayload) -> Self {
    Self { message_id: VulcanMessageId::generate(), protocol_version: VULCAN_PROTOCOL_VERSION.to_string(), sender: from_app, recipient: Some(target_app), timestamp_ms: sent_at_ms, payload: body }
  }

  /// Create a suite-wide broadcast Vulcan message (e.g. `SyncSwatchPalette`, `ApplyBrandKit`).
  pub fn new_broadcast(from_app: CraftAppTarget, sent_at_ms: u64, body: VulcanPayload) -> Self {
    Self { message_id: VulcanMessageId::generate(), protocol_version: VULCAN_PROTOCOL_VERSION.to_string(), sender: from_app, recipient: None, timestamp_ms: sent_at_ms, payload: body }
  }

  /// Canonical Vulcan message bus topic (`vulcan.SuiteMessage.<SAP|*>.\<Verb\>`).
  pub fn vulcan_channel(&self) -> String {
    let target_code = self.recipient.map_or("*", CraftAppTarget::sap_code);
    format!("vulcan.SuiteMessage.{target_code}.{}", self.payload.verb())
  }

  /// True when `app` should receive this envelope (directed match, or suite broadcast excluding sender).
  pub fn routes_to(&self, app: CraftAppTarget) -> bool {
    match self.recipient {
      Some(explicit) => explicit == app,
      None => app != self.sender,
    }
  }

  /// Validate envelope protocol version and payload invariants (hex colors, finite coordinates, SHA-256).
  pub fn validate(&self) -> Result<(), ProvenanceError> {
    if self.protocol_version != VULCAN_PROTOCOL_VERSION {
      return Err(ProvenanceError::InvalidVulcanPayload(format!("unsupported protocol version '{}'", self.protocol_version)));
    }
    self.payload.validate()
  }

  /// Convert directed `OpenDocument` / `PlaceAsset` messages into a `craft://open?...` deep link.
  pub fn to_deep_link(&self) -> Result<Option<String>, ProvenanceError> {
    let Some(target) = self.recipient else {
      return Ok(None);
    };
    match &self.payload {
      VulcanPayload::OpenDocument { path, .. } => CraftDeepLink::new(target, path.clone()).with_action("open").to_uri().map(Some),
      VulcanPayload::PlaceAsset { asset_path, .. } => CraftDeepLink::new(target, asset_path.clone()).with_action("place").to_uri().map(Some),
      _ => Ok(None),
    }
  }
}

impl VulcanPayload {
  pub fn verb(&self) -> &'static str {
    match self {
      Self::OpenDocument { .. } => "OpenDocument",
      Self::PlaceAsset { .. } => "PlaceAsset",
      Self::SyncSwatchPalette { .. } => "SyncSwatchPalette",
      Self::ApplyBrandKit { .. } => "ApplyBrandKit",
      Self::UpdateLinkedSmartObject { .. } => "UpdateLinkedSmartObject",
    }
  }

  pub fn validate(&self) -> Result<(), ProvenanceError> {
    match self {
      Self::OpenDocument { path, .. } => validate_non_empty_clean_str(path, "OpenDocument.path").map_err(ProvenanceError::InvalidVulcanPayload),
      Self::PlaceAsset { asset_path, mime_type, x, y, .. } => {
        validate_non_empty_clean_str(asset_path, "PlaceAsset.asset_path").map_err(ProvenanceError::InvalidVulcanPayload)?;
        if !mime_type.contains('/') || mime_type.trim().len() < 3 {
          return Err(ProvenanceError::InvalidVulcanPayload(format!("invalid MIME type '{mime_type}'")));
        }
        if !x.is_finite() || !y.is_finite() {
          return Err(ProvenanceError::InvalidVulcanPayload("PlaceAsset coordinates must be finite".to_string()));
        }
        Ok(())
      },
      Self::SyncSwatchPalette { palette_name, colors_srgb_hex } => {
        validate_non_empty_clean_str(palette_name, "SyncSwatchPalette.palette_name").map_err(ProvenanceError::InvalidVulcanPayload)?;
        if colors_srgb_hex.is_empty() {
          return Err(ProvenanceError::InvalidVulcanPayload("SyncSwatchPalette requires at least one color".to_string()));
        }
        for color in colors_srgb_hex {
          if !is_rgb_hex_color(color) {
            return Err(ProvenanceError::InvalidVulcanPayload(format!("invalid sRGB hex color '{color}'")));
          }
        }
        Ok(())
      },
      Self::ApplyBrandKit { kit_id, primary_font_family, accent_hex, logo_path } => {
        validate_non_empty_clean_str(kit_id, "ApplyBrandKit.kit_id").map_err(ProvenanceError::InvalidVulcanPayload)?;
        validate_non_empty_clean_str(primary_font_family, "ApplyBrandKit.primary_font_family").map_err(ProvenanceError::InvalidVulcanPayload)?;
        if !is_rgb_hex_color(accent_hex) {
          return Err(ProvenanceError::InvalidVulcanPayload(format!("invalid brand accent hex color '{accent_hex}'")));
        }
        if let Some(lp) = logo_path {
          validate_non_empty_clean_str(lp, "ApplyBrandKit.logo_path").map_err(ProvenanceError::InvalidVulcanPayload)?;
        }
        Ok(())
      },
      Self::UpdateLinkedSmartObject { link_id, rendered_path, content_sha256 } => {
        validate_non_empty_clean_str(link_id, "UpdateLinkedSmartObject.link_id").map_err(ProvenanceError::InvalidVulcanPayload)?;
        validate_non_empty_clean_str(rendered_path, "UpdateLinkedSmartObject.rendered_path").map_err(ProvenanceError::InvalidVulcanPayload)?;
        if !is_lower_sha256_hex(content_sha256) {
          return Err(ProvenanceError::InvalidSha256Hex(content_sha256.clone()));
        }
        Ok(())
      },
    }
  }
}

/// Compute 64-bit FNV-1a deterministic non-cryptographic digest.
pub fn fnv1a_64(data: &[u8]) -> u64 {
  let mut hash = FNV1A_64_OFFSET_BASIS;
  for &byte in data {
    hash ^= u64::from(byte);
    hash = hash.wrapping_mul(FNV1A_64_PRIME);
  }
  hash
}

/// Compute FIPS 180-4 SHA-256 digest in pure Rust with zero external crate dependencies.
pub fn sha256_digest(data: &[u8]) -> [u8; 32] {
  let mut state = SHA256_INITIAL;
  let bit_len = (data.len() as u64).wrapping_mul(8);

  let (blocks, rem) = data.as_chunks::<64>();
  for block in blocks {
    sha256_compress(&mut state, block);
  }

  let mut tail = [0u8; 128];
  tail[..rem.len()].copy_from_slice(rem);
  tail[rem.len()] = 0x80;

  if rem.len() < 56 {
    tail[56..64].copy_from_slice(&bit_len.to_be_bytes());
    let mut first = [0u8; 64];
    first.copy_from_slice(&tail[..64]);
    sha256_compress(&mut state, &first);
  } else {
    tail[120..128].copy_from_slice(&bit_len.to_be_bytes());
    let mut first = [0u8; 64];
    let mut second = [0u8; 64];
    first.copy_from_slice(&tail[..64]);
    second.copy_from_slice(&tail[64..128]);
    sha256_compress(&mut state, &first);
    sha256_compress(&mut state, &second);
  }

  let mut digest = [0u8; 32];
  for (i, &word) in state.iter().enumerate() {
    digest[i * 4..(i + 1) * 4].copy_from_slice(&word.to_be_bytes());
  }
  digest
}

/// RFC 2104 HMAC-SHA256 message authentication code.
pub fn hmac_sha256(key: &[u8], message: &[u8]) -> [u8; 32] {
  let mut key_block = [0u8; 64];
  if key.len() > 64 {
    let hashed_key = sha256_digest(key);
    key_block[..32].copy_from_slice(&hashed_key);
  } else {
    key_block[..key.len()].copy_from_slice(key);
  }

  let mut ipad = [0x36u8; 64];
  let mut opad = [0x5cu8; 64];
  for i in 0..64 {
    ipad[i] ^= key_block[i];
    opad[i] ^= key_block[i];
  }

  let mut inner_input = Vec::with_capacity(64 + message.len());
  inner_input.extend_from_slice(&ipad);
  inner_input.extend_from_slice(message);
  let inner_digest = sha256_digest(&inner_input);

  let mut outer_input = [0u8; 96];
  outer_input[..64].copy_from_slice(&opad);
  outer_input[64..96].copy_from_slice(&inner_digest);
  sha256_digest(&outer_input)
}

fn sha256_compress(state: &mut [u32; 8], block: &[u8; 64]) {
  let mut w = [0u32; 64];
  for i in 0..16 {
    let b0 = block[i * 4];
    let b1 = block[i * 4 + 1];
    let b2 = block[i * 4 + 2];
    let b3 = block[i * 4 + 3];
    w[i] = u32::from_be_bytes([b0, b1, b2, b3]);
  }
  for i in 16..64 {
    let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
    let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
    w[i] = w[i - 16].wrapping_add(s0).wrapping_add(w[i - 7]).wrapping_add(s1);
  }

  let mut a = state[0];
  let mut b = state[1];
  let mut c = state[2];
  let mut d = state[3];
  let mut e = state[4];
  let mut f = state[5];
  let mut g = state[6];
  let mut h = state[7];

  for i in 0..64 {
    let big_s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
    let ch = (e & f) ^ ((!e) & g);
    let temp1 = h.wrapping_add(big_s1).wrapping_add(ch).wrapping_add(SHA256_K[i]).wrapping_add(w[i]);
    let big_s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
    let maj = (a & b) ^ (a & c) ^ (b & c);
    let temp2 = big_s0.wrapping_add(maj);

    h = g;
    g = f;
    f = e;
    e = d.wrapping_add(temp1);
    d = c;
    c = b;
    b = a;
    a = temp1.wrapping_add(temp2);
  }

  state[0] = state[0].wrapping_add(a);
  state[1] = state[1].wrapping_add(b);
  state[2] = state[2].wrapping_add(c);
  state[3] = state[3].wrapping_add(d);
  state[4] = state[4].wrapping_add(e);
  state[5] = state[5].wrapping_add(f);
  state[6] = state[6].wrapping_add(g);
  state[7] = state[7].wrapping_add(h);
}

fn hex_encode(bytes: &[u8]) -> String {
  let mut out = String::with_capacity(bytes.len() * 2);
  for &b in bytes {
    out.push_str(&format!("{b:02x}"));
  }
  out
}

fn constant_time_eq(lhs: &[u8], rhs: &[u8]) -> bool {
  if lhs.len() != rhs.len() {
    return false;
  }
  let mut diff = 0u8;
  for (&a, &b) in lhs.iter().zip(rhs.iter()) {
    diff |= a ^ b;
  }
  diff == 0
}

fn is_lower_sha256_hex(text: &str) -> bool {
  text.len() == 64 && text.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

fn is_rgb_hex_color(text: &str) -> bool {
  let Some(hex) = text.strip_prefix('#') else {
    return false;
  };
  (hex.len() == 6 || hex.len() == 8) && hex.bytes().all(|b| b.is_ascii_hexdigit())
}

fn validate_non_empty_clean_str(value: &str, label: &str) -> Result<(), String> {
  if value.trim().is_empty() {
    return Err(format!("{label} must not be empty"));
  }
  if value.chars().any(char::is_control) {
    return Err(format!("{label} must not contain control characters"));
  }
  Ok(())
}

fn percent_encode_query_value(input: &str) -> String {
  let mut out = String::with_capacity(input.len());
  for &b in input.as_bytes() {
    if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~' | b'/') {
      out.push(char::from(b));
    } else {
      out.push_str(&format!("%{b:02X}"));
    }
  }
  out
}

fn percent_decode_query_value(encoded: &str) -> Result<String, ProvenanceError> {
  let bytes = encoded.as_bytes();
  let mut out = Vec::with_capacity(bytes.len());
  let mut idx = 0;
  while idx < bytes.len() {
    match bytes[idx] {
      b'%' => {
        let Some(hex_pair) = bytes.get(idx + 1..idx + 3) else {
          return Err(ProvenanceError::InvalidDeepLink("truncated percent-escape".to_string()));
        };
        let hex_str = std::str::from_utf8(hex_pair).map_err(|_| ProvenanceError::InvalidDeepLink("invalid percent-escape".to_string()))?;
        let val = u8::from_str_radix(hex_str, 16).map_err(|_| ProvenanceError::InvalidDeepLink("non-hex percent-escape".to_string()))?;
        out.push(val);
        idx += 3;
      },
      b'+' => {
        out.push(b' ');
        idx += 1;
      },
      other => {
        out.push(other);
        idx += 1;
      },
    }
  }
  String::from_utf8(out).map_err(|_| ProvenanceError::InvalidDeepLink("decoded query value is not valid UTF-8".to_string()))
}

fn xml_escape(input: &str) -> String {
  let mut out = String::with_capacity(input.len());
  for ch in input.chars() {
    match ch {
      '&' => out.push_str("&amp;"),
      '<' => out.push_str("&lt;"),
      '>' => out.push_str("&gt;"),
      '"' => out.push_str("&quot;"),
      '\'' => out.push_str("&apos;"),
      other => out.push(other),
    }
  }
  out
}

fn xml_unescape(input: &str) -> Result<String, ProvenanceError> {
  let mut out = String::with_capacity(input.len());
  let mut rest = input;
  while let Some(amp_idx) = rest.find('&') {
    out.push_str(&rest[..amp_idx]);
    let after_amp = &rest[amp_idx + 1..];
    let Some(semi_idx) = after_amp.find(';') else {
      return Err(ProvenanceError::InvalidXmpPacket("unterminated XML entity".to_string()));
    };
    let entity = &after_amp[..semi_idx];
    match entity {
      "amp" => out.push('&'),
      "lt" => out.push('<'),
      "gt" => out.push('>'),
      "quot" => out.push('"'),
      "apos" => out.push('\''),
      _ => {
        return Err(ProvenanceError::InvalidXmpPacket(format!("unsupported XML entity '&{entity};'")));
      },
    }
    rest = &after_amp[semi_idx + 1..];
  }
  out.push_str(rest);
  Ok(out)
}

fn extract_element_text(xml: &str, tag_name: &str) -> Option<String> {
  let open = format!("<{tag_name}>");
  let close = format!("</{tag_name}>");
  let start = xml.find(&open)? + open.len();
  let end = xml[start..].find(&close)? + start;
  xml_unescape(xml[start..end].trim()).ok()
}

fn extract_open_tag_and_body<'a>(xml: &'a str, tag_name: &str) -> Option<(&'a str, &'a str)> {
  let prefix = format!("<{tag_name}");
  let tag_start = xml.find(&prefix)?;
  let after_prefix = &xml[tag_start..];
  let open_end_rel = after_prefix.find('>')?;
  let open_tag = &after_prefix[..=open_end_rel];
  let body_start = tag_start + open_end_rel + 1;
  let close = format!("</{tag_name}>");
  let body_end = xml[body_start..].find(&close)? + body_start;
  Some((open_tag, &xml[body_start..body_end]))
}

fn find_self_closing_tags<'a>(xml: &'a str, tag_name: &str) -> Vec<&'a str> {
  let needle = format!("<{tag_name} ");
  let mut results = Vec::new();
  let mut cursor = xml;
  while let Some(pos) = cursor.find(&needle) {
    let from_tag = &cursor[pos..];
    let Some(end_rel) = from_tag.find("/>") else {
      break;
    };
    results.push(&from_tag[..end_rel + 2]);
    cursor = &from_tag[end_rel + 2..];
  }
  results
}

fn extract_xml_attr(tag: &str, attr_name: &str) -> Option<String> {
  let pattern = format!("{attr_name}=\"");
  let start = tag.find(&pattern)? + pattern.len();
  let end = tag[start..].find('"')? + start;
  xml_unescape(&tag[start..end]).ok()
}

#[cfg(test)]
mod tests {
  use super::*;
  use strum::IntoEnumIterator;

  const TEST_SECRET: &[u8] = b"artcraft-acai-c2pa-signing-secret-v2";
  const SAMPLE_IMAGE_BYTES: &[u8] = b"\x89PNG\r\n\x1a\nArtCraftGeneratedAssetPayload";

  mod crypto_and_digests {
    use super::*;

    #[test]
    fn sha256_matches_nist_fips_180_4_vectors() {
      assert_eq!(hex_encode(&sha256_digest(b"")), "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855");
      assert_eq!(hex_encode(&sha256_digest(b"abc")), "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad");
      assert_eq!(hex_encode(&sha256_digest(b"abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq")), "248d6a61d20638b8e5c026930c3e6039a33ce45964ff2167f6ecedd419db06c1");
    }

    #[test]
    fn fnv1a_64_matches_standard_reference_vectors() {
      assert_eq!(fnv1a_64(b""), 0xcbf2_9ce4_8422_2325);
      assert_eq!(fnv1a_64(b"foobar"), 0x8594_4171_f739_67e8);
    }

    #[test]
    fn content_binding_digest_is_deterministic_and_detects_single_bit_flip() {
      let d1 = ContentBindingDigest::compute(SAMPLE_IMAGE_BYTES);
      let d2 = ContentBindingDigest::compute(SAMPLE_IMAGE_BYTES);
      assert_eq!(d1, d2);
      assert!(d1.matches_payload(SAMPLE_IMAGE_BYTES));

      let mut mutated = SAMPLE_IMAGE_BYTES.to_vec();
      mutated[10] ^= 0x01;
      assert!(!d1.matches_payload(&mutated));
    }

    #[test]
    fn hmac_sha256_handles_keys_longer_than_block_size_rfc_4231_tc6() {
      let long_key = [0xaau8; 131];
      let msg = b"Test Using Larger Than Block-Size Key - Hash Key First";
      assert_eq!(hex_encode(&hmac_sha256(&long_key, msg)), "60e431591ee0b67f0d8a26aacbf5b77f8e0bc6213728c5140546040f0ee37f54");
    }

    #[test]
    fn typed_c2pa_and_vulcan_ids_enforce_prefix_and_ulid_rules() {
      let c_id = C2paManifestId::generate();
      let v_id = VulcanMessageId::generate();
      assert!(c_id.as_str().starts_with("c2pa_"));
      assert!(v_id.as_str().starts_with("vulcan_"));
      assert_eq!(c_id.as_str().parse::<C2paManifestId>().unwrap(), c_id);
      assert_eq!(v_id.as_str().parse::<VulcanMessageId>().unwrap(), v_id);
      assert!(c_id.as_str().parse::<VulcanMessageId>().is_err());
    }
  }

  mod c2pa_credentials_and_xmp {
    use super::*;

    #[test]
    fn all_c2pa_actions_round_trip_strings_and_serde() {
      assert_eq!(C2paActionKind::ALL.len(), C2paActionKind::iter().count());
      for kind in C2paActionKind::ALL {
        let s = kind.as_str();
        assert!(s.starts_with("c2pa."));
        assert_eq!(C2paActionKind::from_c2pa_str(s).unwrap(), kind);
        let json = serde_json::to_string(&kind).unwrap();
        assert_eq!(serde_json::from_str::<C2paActionKind>(&json).unwrap(), kind);
      }
      assert!(matches!(C2paActionKind::from_c2pa_str("c2pa.unknown"), Err(ProvenanceError::UnknownAction(_))));
    }

    #[test]
    fn digital_source_types_parse_both_short_codes_and_iptc_uris() {
      assert_eq!(DigitalSourceType::ALL.len(), DigitalSourceType::iter().count());
      for src in DigitalSourceType::ALL {
        assert_eq!(DigitalSourceType::parse_source_type(src.as_str()).unwrap(), src);
        assert_eq!(DigitalSourceType::parse_source_type(src.iptc_uri()).unwrap(), src);
      }
      assert!(DigitalSourceType::parse_source_type("invalidSource").is_err());
    }

    #[test]
    fn sign_and_verify_payload_succeeds_and_catches_tampering() {
      let mut creds = build_sample_credentials();
      let sig = creds.sign_payload(SAMPLE_IMAGE_BYTES, TEST_SECRET, "CN=ArtCraft Content Authenticity").unwrap();
      assert_eq!(sig.hmac_sha256_hex.len(), 64);
      assert_eq!(sig.manifest_fnv1a64_hex.len(), 16);
      assert_eq!(creds.verify_payload(SAMPLE_IMAGE_BYTES, TEST_SECRET), Ok(()));

      // Wrong key fails signature check
      assert_eq!(creds.verify_payload(SAMPLE_IMAGE_BYTES, b"wrong-key"), Err(ProvenanceError::SignatureMismatch));

      // Tampered payload bytes fail digest check
      assert_eq!(creds.verify_payload(b"tampered-image-bytes", TEST_SECRET), Err(ProvenanceError::DigestMismatch));
    }

    #[test]
    fn mutating_manifest_after_signing_clears_signature_or_fails_tamper_check() {
      let mut creds = build_sample_credentials();
      creds.sign_payload(SAMPLE_IMAGE_BYTES, TEST_SECRET, "ArtCraftCA").unwrap();

      // Direct field tamper without re-signing is caught by HMAC verification
      let mut tampered = creds.clone();
      tampered.digital_source_type = DigitalSourceType::DigitalCapture;
      assert_eq!(tampered.verify_payload(SAMPLE_IMAGE_BYTES, TEST_SECRET), Err(ProvenanceError::SignatureMismatch));

      // Appending a new action via API invalidates stale signature
      creds.record_action(C2paAction::for_app(C2paActionKind::Repackaged, CraftAppTarget::PdfCraft, "2026-11-27T18:00:00Z", None));
      assert_eq!(creds.verify_payload(SAMPLE_IMAGE_BYTES, TEST_SECRET), Err(ProvenanceError::UnsignedCredentials));
    }

    #[test]
    fn sign_rejects_empty_key_or_blank_signer() {
      let mut creds = build_sample_credentials();
      assert_eq!(creds.sign_payload(SAMPLE_IMAGE_BYTES, b"", "Signer"), Err(ProvenanceError::EmptySecretKey));
      assert_eq!(creds.sign_payload(SAMPLE_IMAGE_BYTES, TEST_SECRET, "   "), Err(ProvenanceError::EmptySignerId));
    }

    #[test]
    fn ingredient_chain_tracks_parent_and_validates_sha256() {
      let parent_id = C2paManifestId::generate();
      let parent_ing = C2paIngredient::try_new("CameraRawBase.dng", "image/x-adobe-dng", "xmp:iid:001", IngredientRelationship::ParentOf, "a".repeat(64), Some(parent_id.clone())).unwrap();
      let comp_ing = C2paIngredient::try_new("AI_Sky_Layer.png", "image/png", "xmp:iid:002", IngredientRelationship::ComponentOf, "b".repeat(64), None).unwrap();

      let mut creds = C2paContentCredentials::new(DEFAULT_CLAIM_GENERATOR, DigitalSourceType::CompositeWithTrainedAlgorithmicMedia);
      creds.add_ingredient(parent_ing);
      creds.add_ingredient(comp_ing);
      assert_eq!(creds.parent_ingredients().len(), 1);
      assert_eq!(creds.parent_ingredients()[0].parent_manifest_id.as_ref(), Some(&parent_id));

      assert!(matches!(C2paIngredient::try_new("Bad.png", "image/png", "id", IngredientRelationship::InputTo, "not-a-sha256", None,), Err(ProvenanceError::InvalidSha256Hex(_))));
    }

    #[test]
    fn xmp_packet_round_trips_special_characters_and_preserves_valid_signature() {
      let mut creds = build_sample_credentials();
      creds.record_action(C2paAction::new(C2paActionKind::ColorAdjustments, "LightCraft & PhotoCraft <HDR>", "2026-11-27T17:45:00Z", Some(DigitalSourceType::CompositeWithTrainedAlgorithmicMedia), Some("Exposure +0.75 EV & \"Filmic\" curve 'A'".to_string())));
      creds.sign_payload(SAMPLE_IMAGE_BYTES, TEST_SECRET, "ArtCraft Root <CA>").unwrap();

      let xmp = creds.to_xmp_packet().unwrap();
      assert!(xmp.contains("<dcterms:provenance>urn:c2pa:c2pa_"));
      assert!(xmp.contains("<c2pa:manifest "));

      let parsed = C2paContentCredentials::from_xmp_packet(&xmp).unwrap();
      assert_eq!(parsed, creds);
      assert_eq!(parsed.verify_payload(SAMPLE_IMAGE_BYTES, TEST_SECRET), Ok(()));
    }

    #[test]
    fn xmp_parser_rejects_mismatched_urn_and_malformed_packets() {
      let creds = build_sample_credentials();
      let xmp = creds.to_xmp_packet().unwrap();
      let forged = xmp.replace(creds.manifest_id.as_str(), C2paManifestId::generate().as_str());
      // Replacing both keeps consistency, so corrupt only dcterms:provenance
      let broken_urn = xmp.replace(&format!("<dcterms:provenance>{}</dcterms:provenance>", creds.manifest_urn()), "<dcterms:provenance>urn:c2pa:c2pa_00000000000000000000000000</dcterms:provenance>");
      assert!(C2paContentCredentials::from_xmp_packet(&forged).is_ok());
      assert!(matches!(C2paContentCredentials::from_xmp_packet(&broken_urn), Err(ProvenanceError::InvalidXmpPacket(_))));
      assert!(C2paContentCredentials::from_xmp_packet("<not-xmp/>").is_err());
    }
  }

  mod craft_apps_and_vulcan_bus {
    use super::*;

    #[test]
    fn covers_all_nineteen_craft_suite_apps_with_unique_slugs_and_sap_codes() {
      assert_eq!(CraftAppTarget::ALL.len(), 19);
      assert_eq!(CraftAppTarget::iter().count(), 19);
      assert_eq!(CraftAppTarget::all_variants().len(), 19);

      let mut slugs = BTreeSet::new();
      let mut saps = BTreeSet::new();
      for app in CraftAppTarget::ALL {
        assert!(slugs.insert(app.slug()), "duplicate slug {}", app.slug());
        assert!(saps.insert(app.sap_code()), "duplicate canonical SAPCode {}", app.sap_code());
        assert_eq!(CraftAppTarget::from_slug(app.slug()).unwrap(), app);
        assert_eq!(CraftAppTarget::from_slug(app.display_name()).unwrap(), app);
        assert_eq!(CraftAppTarget::from_sap_code(app.sap_code()).unwrap(), app);
      }
    }

    #[test]
    fn maps_every_adobe_creative_cloud_and_substance_sap_code() {
      let sap_table = [("PHSP", CraftAppTarget::PhotoCraft), ("ILST", CraftAppTarget::VectorCraft), ("PPRO", CraftAppTarget::FilmCraft), ("LTRM", CraftAppTarget::LightCraft), ("LRCC", CraftAppTarget::LightCraft), ("APRO", CraftAppTarget::PdfCraft), ("AEFT", CraftAppTarget::EffectCraft), ("FLPR", CraftAppTarget::AniCraft), ("IDSN", CraftAppTarget::DesignCraft), ("SPRK", CraftAppTarget::FigCraft), ("XD", CraftAppTarget::FigCraft), ("AUDT", CraftAppTarget::SoundCraft), ("AICY", CraftAppTarget::WordCraft), ("PSXUWP", CraftAppTarget::DeckCraft), ("SHPR", CraftAppTarget::SolidCraft), ("SBSTP", CraftAppTarget::TextureCraft), ("SBSTD", CraftAppTarget::MaterialCraft)];
      for (sap, expected) in sap_table {
        assert_eq!(CraftAppTarget::from_sap_code(sap).unwrap(), expected, "{sap}");
      }
      assert!(CraftAppTarget::from_sap_code("NOPE").is_err());
    }

    #[test]
    fn deep_link_builder_and_parser_round_trip_unicode_and_spaces() {
      let path = "/Projects/2026 Campaign/Hero Shot #1 (Final).psd";
      let uri = CraftDeepLink::build(CraftAppTarget::PhotoCraft, path).unwrap();
      assert!(uri.starts_with("craft://open?app=photocraft&path="));

      let parsed = CraftDeepLink::parse(&uri).unwrap();
      assert_eq!(parsed.app, CraftAppTarget::PhotoCraft);
      assert_eq!(parsed.path, path);
      assert_eq!(parsed.action, None);

      // Also accepts SAPCode in `app=` query parameter
      let sap_uri = "craft://open?app=ILST&path=/assets/logo%20mark.svg&action=edit";
      let parsed_sap = CraftDeepLink::parse(sap_uri).unwrap();
      assert_eq!(parsed_sap.app, CraftAppTarget::VectorCraft);
      assert_eq!(parsed_sap.path, "/assets/logo mark.svg");
      assert_eq!(parsed_sap.action.as_deref(), Some("edit"));
    }

    #[test]
    fn deep_link_rejects_wrong_scheme_missing_params_and_bad_escapes() {
      assert!(CraftDeepLink::parse("https://open?app=photocraft&path=/a").is_err());
      assert!(CraftDeepLink::parse("craft://open?app=photocraft").is_err());
      assert!(CraftDeepLink::parse("craft://open?path=/a.png").is_err());
      assert!(CraftDeepLink::parse("craft://open?app=photocraft&path=%ZZ").is_err());
      assert!(CraftDeepLink::build(CraftAppTarget::ArtCraft, "   ").is_err());
    }

    #[test]
    fn vulcan_directed_open_document_and_place_asset_routing() {
      let c2pa_id = C2paManifestId::generate();
      let place_env = VulcanEnvelope::new_directed(CraftAppTarget::ArtCraft, CraftAppTarget::FilmCraft, 1_764_264_000_000, VulcanPayload::PlaceAsset { asset_path: "/renders/scene_01.mp4".to_string(), mime_type: "video/mp4".to_string(), x: 1920.0, y: 1080.0, credentials_id: Some(c2pa_id) });

      assert_eq!(place_env.validate(), Ok(()));
      assert_eq!(place_env.vulcan_channel(), "vulcan.SuiteMessage.PPRO.PlaceAsset");
      assert!(place_env.routes_to(CraftAppTarget::FilmCraft));
      assert!(!place_env.routes_to(CraftAppTarget::ArtCraft));
      assert!(!place_env.routes_to(CraftAppTarget::PhotoCraft));

      let deep = place_env.to_deep_link().unwrap().unwrap();
      assert_eq!(deep, "craft://open?app=filmcraft&path=/renders/scene_01.mp4&action=place");
    }

    #[test]
    fn vulcan_broadcast_swatch_palette_and_brand_kit_reach_all_other_apps() {
      let swatch_env = VulcanEnvelope::new_broadcast(CraftAppTarget::DesignCraft, 1_764_264_100_000, VulcanPayload::SyncSwatchPalette { palette_name: "Autumn Editorial".to_string(), colors_srgb_hex: vec!["#1E293B".to_string(), "#F97316".to_string(), "#FAF8F5FF".to_string()] });
      assert_eq!(swatch_env.validate(), Ok(()));
      assert_eq!(swatch_env.vulcan_channel(), "vulcan.SuiteMessage.*.SyncSwatchPalette");
      assert!(!swatch_env.routes_to(CraftAppTarget::DesignCraft));
      assert_eq!(CraftAppTarget::ALL.into_iter().filter(|&app| swatch_env.routes_to(app)).count(), 18);

      let brand_env = VulcanEnvelope::new_broadcast(CraftAppTarget::FigCraft, 1_764_264_200_000, VulcanPayload::ApplyBrandKit { kit_id: "brand-storytold-2026".to_string(), primary_font_family: "Source Serif 4".to_string(), accent_hex: "#3B82F6".to_string(), logo_path: Some("/brand/mark.svg".to_string()) });
      assert_eq!(brand_env.validate(), Ok(()));
      assert_eq!(brand_env.to_deep_link().unwrap(), None);
    }

    #[test]
    fn vulcan_update_linked_smart_object_and_serde_round_trip() {
      let digest = hex_encode(&sha256_digest(SAMPLE_IMAGE_BYTES));
      let smart_env = VulcanEnvelope::new_directed(CraftAppTarget::VectorCraft, CraftAppTarget::PhotoCraft, 1_764_264_300_000, VulcanPayload::UpdateLinkedSmartObject { link_id: "so-badge-01".to_string(), rendered_path: "/cache/so-badge-01.png".to_string(), content_sha256: digest });
      assert_eq!(smart_env.validate(), Ok(()));
      assert_eq!(smart_env.vulcan_channel(), "vulcan.SuiteMessage.PHSP.UpdateLinkedSmartObject");

      let json = serde_json::to_string(&smart_env).unwrap();
      let decoded: VulcanEnvelope = serde_json::from_str(&json).unwrap();
      assert_eq!(decoded, smart_env);
    }

    #[test]
    fn vulcan_validation_rejects_bad_colors_nan_coords_and_bad_digests() {
      let bad_swatch = VulcanEnvelope::new_broadcast(CraftAppTarget::ArtCraft, 1, VulcanPayload::SyncSwatchPalette { palette_name: "Broken".to_string(), colors_srgb_hex: vec!["not-a-color".to_string()] });
      assert!(matches!(bad_swatch.validate(), Err(ProvenanceError::InvalidVulcanPayload(_))));

      let nan_place = VulcanEnvelope::new_directed(CraftAppTarget::ArtCraft, CraftAppTarget::PhotoCraft, 1, VulcanPayload::PlaceAsset { asset_path: "/a.png".to_string(), mime_type: "image/png".to_string(), x: f64::NAN, y: 0.0, credentials_id: None });
      assert!(matches!(nan_place.validate(), Err(ProvenanceError::InvalidVulcanPayload(_))));

      let bad_smart = VulcanEnvelope::new_directed(CraftAppTarget::TextureCraft, CraftAppTarget::SolidCraft, 1, VulcanPayload::UpdateLinkedSmartObject { link_id: "l1".to_string(), rendered_path: "/tex/albedo.exr".to_string(), content_sha256: "ABCD".to_string() });
      assert!(matches!(bad_smart.validate(), Err(ProvenanceError::InvalidSha256Hex(_))));
    }

    #[test]
    fn open_document_payload_supports_page_targeting_and_deep_link() {
      let open_env = VulcanEnvelope::new_directed(CraftAppTarget::WordCraft, CraftAppTarget::DeckCraft, 99, VulcanPayload::OpenDocument { path: "/decks/Q4 Review.deck".to_string(), read_only: true, page_or_frame: Some(12) });
      assert_eq!(open_env.validate(), Ok(()));
      assert_eq!(open_env.vulcan_channel(), "vulcan.SuiteMessage.PSXUWP.OpenDocument");
      let uri = open_env.to_deep_link().unwrap().unwrap();
      let reparsed = CraftDeepLink::parse(&uri).unwrap();
      assert_eq!(reparsed.app, CraftAppTarget::DeckCraft);
      assert_eq!(reparsed.path, "/decks/Q4 Review.deck");
      assert_eq!(reparsed.action.as_deref(), Some("open"));
    }

    #[test]
    fn vulcan_envelope_rejects_unsupported_protocol_version() {
      let mut env = VulcanEnvelope::new_directed(CraftAppTarget::CadCraft, CraftAppTarget::GridCraft, 10, VulcanPayload::OpenDocument { path: "/cad/part.step".to_string(), read_only: false, page_or_frame: None });
      env.protocol_version = "vulcan/0.9".to_string();
      assert!(matches!(env.validate(), Err(ProvenanceError::InvalidVulcanPayload(_))));
    }

    #[test]
    fn unsigned_credentials_round_trip_xmp_and_report_unsigned_on_verify() {
      let creds = C2paContentCredentials::new(DEFAULT_CLAIM_GENERATOR, DigitalSourceType::DigitalCapture);
      assert_eq!(creds.verify_payload(SAMPLE_IMAGE_BYTES, TEST_SECRET), Err(ProvenanceError::UnsignedCredentials));
      let xmp = creds.to_xmp_packet().unwrap();
      let restored = C2paContentCredentials::from_xmp_packet(&xmp).unwrap();
      assert_eq!(restored, creds);
    }
  }

  fn build_sample_credentials() -> C2paContentCredentials {
    let mut creds = C2paContentCredentials::new(DEFAULT_CLAIM_GENERATOR, DigitalSourceType::TrainedAlgorithmicMedia);
    creds.record_action(C2paAction::for_app(C2paActionKind::Created, CraftAppTarget::ArtCraft, "2026-11-27T17:30:00Z", Some(DigitalSourceType::TrainedAlgorithmicMedia)));
    creds.record_action(C2paAction::for_app(C2paActionKind::Edited, CraftAppTarget::PhotoCraft, "2026-11-27T17:35:00Z", None));
    creds.record_action(C2paAction::for_app(C2paActionKind::Placed, CraftAppTarget::DesignCraft, "2026-11-27T17:40:00Z", None));
    let parent_sha = hex_encode(&sha256_digest(b"prompt-seed-reference"));
    let ing = C2paIngredient::try_new("ReferencePlate.png", "image/png", "xmp:iid:ref-01", IngredientRelationship::InputTo, parent_sha, Some(C2paManifestId::generate())).unwrap();
    creds.add_ingredient(ing);
    creds
  }
}
