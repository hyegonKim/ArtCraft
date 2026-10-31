use std::collections::HashMap;

/// How TTS commands should be prefixed: !voiceName, /voiceName, etc.
#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone, Copy)]
#[serde(rename_all = "snake_case")]
pub enum CommandPrefixType {
  /// Prefix voice commands with "!"
  Exclamation,
  /// Prefix voice commands with "!" or "/"
  ExclamationOrSlash,
  /// Prefix voice commands with "/"
  Slash,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone)]
#[serde(rename_all = "snake_case")]
pub enum EventResponse {
  /// Default value
  /// NB: This has an empty struct so it serializes as JSON and not a String!
  NotSet {},

  /// Respond with a single TTS voice.
  TtsSingleVoice {
    tts_model_token: String,
  },

  /// Respond with a random TTS voice.
  TtsRandomVoice {
    tts_model_tokens: Vec<String>,
  },

  /// Use TTS commands that FakeYou recommends.
  TtsCommandPresets {
    command_prefix_type: CommandPrefixType,
  },

  /// Use your own TTS commands.
  /// The map is from "!command" (without the "/" or "!" prefix) to model token.
  /// Command is lower case, alphanum only, and compared case insensitive.
  TtsCommandCustom {
    command_prefix_type: CommandPrefixType,
    command_map: HashMap<String, String>,
  },
}

#[cfg(test)]
mod tests {
  use crate::complex_models::event_responses::EventResponse;

  #[test]
  fn not_set() {
    let rust_value = EventResponse::NotSet {};
    let json = "{\"not_set\":{}}";

    let converted_to_json= serde_json::to_string(&rust_value).unwrap();
    assert_eq!(&converted_to_json, json);

    let converted_from_json : EventResponse = serde_json::from_str(json).unwrap();
    assert_eq!(&converted_from_json, &rust_value);
  }

  #[test]
  fn tts_single_voice() {
    let rust_value = EventResponse::TtsSingleVoice {
      tts_model_token: "foo".to_string()
    };
    let json = "{\"tts_single_voice\":{\"tts_model_token\":\"foo\"}}";

    let converted_to_json= serde_json::to_string(&rust_value).unwrap();
    assert_eq!(&converted_to_json, json);

    let converted_from_json : EventResponse = serde_json::from_str(json).unwrap();
    assert_eq!(&converted_from_json, &rust_value);
  }
}
