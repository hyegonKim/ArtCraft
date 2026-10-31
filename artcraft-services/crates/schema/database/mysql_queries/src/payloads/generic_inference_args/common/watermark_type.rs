
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum WatermarkType {
  /// Show a FakeYou watermark
  #[serde(rename = "f")] // NB: DO NOT CHANGE. It could break live jobs. Renamed to be fewer bytes.
  FakeYou,

  /// Show a Storyteller watermark
  #[serde(rename = "s")] // NB: DO NOT CHANGE. It could break live jobs. Renamed to be fewer bytes.
  Storyteller,

  /// Show no watermark
  /// Same as if Option<WatermarkType> is None.
  #[serde(rename = "n")] // NB: DO NOT CHANGE. It could break live jobs. Renamed to be fewer bytes.
  NoWatermark,
}
