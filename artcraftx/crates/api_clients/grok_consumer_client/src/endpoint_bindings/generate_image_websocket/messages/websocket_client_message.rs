use crate::datatypes::api::request_id::RequestId;
use crate::prompt_flags::PromptFlags;
use chrono::Utc;
use serde::Serialize;
use uuid::Uuid;

/// Number of images requested per prompt. The web app pairs this with
/// `enable_side_by_side` to render a 2-up result (observed 2026-08-23).
const DEFAULT_NUM_GENERATIONS: usize = 2;

/// The web app requests two side-by-side images by default.
const DEFAULT_ENABLE_SIDE_BY_SIDE: bool = true;

/// The web app does not request a watermark.
const DEFAULT_ENABLE_WATERMARK: bool = false;

/// Aspect ratios available in **fast** ("speed") image mode (`enable_pro:
/// false`). Grok's fast mode supports a smaller set than quality mode.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FastAspectRatio {
  /// 1:1
  Square,
  /// 2:3
  TallTwoByThree,
  /// 3:2
  WideThreeByTwo,
  /// 16:9
  WideSixteenByNine,
  /// 9:16
  TallNineBySixteen,
}

impl FastAspectRatio {
  /// The Grok wire string, e.g. `"16:9"`.
  pub fn as_grok_str(self) -> &'static str {
    match self {
      Self::Square => "1:1",
      Self::TallTwoByThree => "2:3",
      Self::WideThreeByTwo => "3:2",
      Self::WideSixteenByNine => "16:9",
      Self::TallNineBySixteen => "9:16",
    }
  }
}

/// Aspect ratios available in **quality** ("pro") image mode (`enable_pro:
/// true`). A superset of the fast ratios plus 4:3, 21:9, and 5:2.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum QualityAspectRatio {
  /// 1:1
  Square,
  /// 9:16
  TallNineBySixteen,
  /// 16:9
  WideSixteenByNine,
  /// 2:3
  TallTwoByThree,
  /// 3:2
  WideThreeByTwo,
  /// 4:3
  WideFourByThree,
  /// 21:9
  WideTwentyOneByNine,
  /// 5:2
  WideFiveByTwo,
}

impl QualityAspectRatio {
  /// The Grok wire string, e.g. `"21:9"`.
  pub fn as_grok_str(self) -> &'static str {
    match self {
      Self::Square => "1:1",
      Self::TallNineBySixteen => "9:16",
      Self::WideSixteenByNine => "16:9",
      Self::TallTwoByThree => "2:3",
      Self::WideThreeByTwo => "3:2",
      Self::WideFourByThree => "4:3",
      Self::WideTwentyOneByNine => "21:9",
      Self::WideFiveByTwo => "5:2",
    }
  }
}

#[derive(Serialize)]
pub struct WebsocketClientMessage {
  /// Type of message
  /// eg. "conversation.item.create"
  pub r#type: String,

  /// Unix timestamp
  pub timestamp: u64,

  pub item: ClientMessageItem,
}

#[derive(Serialize)]
pub struct ClientMessageItem {
  /// Type of item
  /// eg. "message"
  pub r#type: String,

  pub content: Vec<ClientMessageItemContent>,
}

#[derive(Serialize)]
pub struct ClientMessageItemContent {
  /// UUID
  #[serde(rename="requestId")]
  pub request_id: String,

  /// Type of item content
  /// eg. "input_text"
  pub r#type: String,

  /// The prompt for the request
  pub text: String,

  pub properties: ClientMessageItemContentProperties,
}

/// Field order mirrors the web app's wire payload (observed 2026-08-23).
#[derive(Serialize)]
pub struct ClientMessageItemContentProperties {
  pub section_count: usize,
  pub is_kids_mode: bool,
  pub enable_nsfw: bool,
  pub skip_upsampler: bool,
  pub enable_side_by_side: bool,
  pub is_initial: bool,
  /// The Grok aspect-ratio string, e.g. "2:3" or "21:9".
  pub aspect_ratio: String,
  /// Quality flag: `true` selects "pro"/quality (slow); `false` is fast.
  pub enable_pro: bool,
  pub num_generations: usize,
  pub enable_watermark: bool,
}

impl WebsocketClientMessage {
  /// A **fast** ("speed") image prompt (`enable_pro: false`).
  ///
  /// `flags` appends any `--` long args (e.g. `--mode=extremely-spicy-or-crazy`)
  /// to the prompt text; [`PromptFlags::default()`] leaves the prompt untouched,
  /// matching the plain-prompt captures. Mirrors the web app's
  /// `conversation.item.create` send frame (see
  /// `external/requests/sites/grok.com/2026-08-23-imagine/14_same_websocket_low_quality_fast_images.har.json`
  /// and `21_spicey_image_gen_test.har.json`).
  pub fn new_fast_image_prompt(prompt: &str, aspect_ratio: FastAspectRatio, flags: &PromptFlags) -> Self {
    Self::new_image_prompt(prompt, aspect_ratio.as_grok_str(), false, flags)
  }

  /// A **quality** ("pro") image prompt (`enable_pro: true`).
  ///
  /// Mirrors the web app's send frame (see
  /// `external/requests/sites/grok.com/2026-08-23-imagine/13_image_high_quality_websocket.har.json`).
  pub fn new_quality_image_prompt(prompt: &str, aspect_ratio: QualityAspectRatio, flags: &PromptFlags) -> Self {
    Self::new_image_prompt(prompt, aspect_ratio.as_grok_str(), true, flags)
  }

  /// The generated `requestId` of this prompt. Grok tags every progress /
  /// image / error frame for the prompt with it, so callers key results on it.
  pub fn request_id(&self) -> RequestId {
    RequestId(
      self.item.content.first()
          .map(|content| content.request_id.clone())
          .unwrap_or_default(),
    )
  }

  fn new_image_prompt(prompt: &str, aspect_ratio: &str, enable_pro: bool, flags: &PromptFlags) -> Self {
    Self {
      r#type: "conversation.item.create".to_string(),
      timestamp: Utc::now().timestamp_millis() as u64,
      item: ClientMessageItem {
        r#type: "message".to_string(),
        content: vec![
          ClientMessageItemContent {
            request_id: Uuid::new_v4().to_string(),
            r#type: "input_text".to_string(),
            text: flags.apply_to(prompt),
            properties: ClientMessageItemContentProperties {
              section_count: 0,
              is_kids_mode: false,
              enable_nsfw: true,
              skip_upsampler: false,
              enable_side_by_side: DEFAULT_ENABLE_SIDE_BY_SIDE,
              is_initial: false,
              aspect_ratio: aspect_ratio.to_string(),
              enable_pro,
              num_generations: DEFAULT_NUM_GENERATIONS,
              enable_watermark: DEFAULT_ENABLE_WATERMARK,
            },
          }
        ],
      },
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  // Cargo runs tests with the crate root as the working directory.
  fn load_send_frame(file_name: &str) -> serde_json::Value {
    serde_json::from_str(
      &std::fs::read_to_string(format!("test_data/websocket_messages/{file_name}")).unwrap(),
    ).unwrap()
  }

  /// Assert our serialized message matches a captured send frame property for
  /// property (ignoring `requestId`/`timestamp`, which we generate).
  fn assert_matches_capture(ours: &serde_json::Value, captured: &serde_json::Value) {
    assert_eq!(ours["type"], captured["type"]);
    assert_eq!(ours["item"]["type"], captured["item"]["type"]);

    let ours_content = &ours["item"]["content"][0];
    let captured_content = &captured["item"]["content"][0];
    assert_eq!(ours_content["type"], captured_content["type"]);
    assert_eq!(ours_content["text"], captured_content["text"]);
    assert!(ours_content["requestId"].as_str().is_some());

    let captured_props = captured_content["properties"].as_object().unwrap();
    let ours_props = &ours_content["properties"];
    for (key, value) in captured_props {
      assert_eq!(&ours_props[key], value, "property `{key}` differs from the real frame");
    }
  }

  // A `--mode` flag is appended to the image prompt text, matching the real
  // spicy capture (21_spicey_image_gen_test.har.json).
  #[test]
  fn mode_flag_appended_to_image_prompt() {
    use crate::prompt_flags::GenerationMode;
    let value = serde_json::to_value(
      WebsocketClientMessage::new_fast_image_prompt(
        "woman on beach",
        FastAspectRatio::WideSixteenByNine,
        &PromptFlags::with_mode(GenerationMode::Spicy),
      ),
    ).unwrap();
    assert_eq!(
      value["item"]["content"][0]["text"],
      "woman on beach --mode=extremely-spicy-or-crazy",
    );
  }

  mod fast_mode_tests {
    use super::*;

    #[test]
    fn wire_format() {
      let value = serde_json::to_value(
        WebsocketClientMessage::new_fast_image_prompt("a cat", FastAspectRatio::TallTwoByThree, &PromptFlags::default()),
      ).unwrap();
      let properties = &value["item"]["content"][0]["properties"];

      assert_eq!(value["type"], "conversation.item.create");
      assert_eq!(properties["aspect_ratio"], "2:3");
      assert_eq!(properties["enable_pro"], false);
      assert_eq!(properties["enable_side_by_side"], true);
      assert_eq!(properties["num_generations"], 2);
      assert_eq!(properties["enable_watermark"], false);
      assert_eq!(properties["skip_upsampler"], false);
    }

    // Real fast send frame: "Race car", 3:2 (from capture 14).
    #[test]
    fn matches_real_captured_frame() {
      let ours = serde_json::to_value(
        WebsocketClientMessage::new_fast_image_prompt("Race car", FastAspectRatio::WideThreeByTwo, &PromptFlags::default()),
      ).unwrap();
      assert_matches_capture(&ours, &load_send_frame("real_fast_image_prompt_request.json"));
    }

    #[test]
    fn aspect_ratios_serialize_to_grok_strings() {
      assert_eq!(FastAspectRatio::Square.as_grok_str(), "1:1");
      assert_eq!(FastAspectRatio::TallTwoByThree.as_grok_str(), "2:3");
      assert_eq!(FastAspectRatio::WideThreeByTwo.as_grok_str(), "3:2");
      assert_eq!(FastAspectRatio::WideSixteenByNine.as_grok_str(), "16:9");
      assert_eq!(FastAspectRatio::TallNineBySixteen.as_grok_str(), "9:16");
    }
  }

  mod quality_mode_tests {
    use super::*;

    #[test]
    fn wire_format() {
      let value = serde_json::to_value(
        WebsocketClientMessage::new_quality_image_prompt("a cat", QualityAspectRatio::WideTwentyOneByNine, &PromptFlags::default()),
      ).unwrap();
      let properties = &value["item"]["content"][0]["properties"];

      assert_eq!(properties["aspect_ratio"], "21:9");
      assert_eq!(properties["enable_pro"], true);
      assert_eq!(properties["enable_side_by_side"], true);
      assert_eq!(properties["num_generations"], 2);
    }

    // Real quality send frame: "luxury car", 21:9 (from capture 13).
    #[test]
    fn matches_real_captured_frame() {
      let ours = serde_json::to_value(
        WebsocketClientMessage::new_quality_image_prompt("luxury car", QualityAspectRatio::WideTwentyOneByNine, &PromptFlags::default()),
      ).unwrap();
      assert_matches_capture(&ours, &load_send_frame("real_quality_image_prompt_request.json"));
    }

    #[test]
    fn aspect_ratios_serialize_to_grok_strings() {
      assert_eq!(QualityAspectRatio::Square.as_grok_str(), "1:1");
      assert_eq!(QualityAspectRatio::TallNineBySixteen.as_grok_str(), "9:16");
      assert_eq!(QualityAspectRatio::WideSixteenByNine.as_grok_str(), "16:9");
      assert_eq!(QualityAspectRatio::TallTwoByThree.as_grok_str(), "2:3");
      assert_eq!(QualityAspectRatio::WideThreeByTwo.as_grok_str(), "3:2");
      assert_eq!(QualityAspectRatio::WideFourByThree.as_grok_str(), "4:3");
      assert_eq!(QualityAspectRatio::WideTwentyOneByNine.as_grok_str(), "21:9");
      assert_eq!(QualityAspectRatio::WideFiveByTwo.as_grok_str(), "5:2");
    }
  }
}
