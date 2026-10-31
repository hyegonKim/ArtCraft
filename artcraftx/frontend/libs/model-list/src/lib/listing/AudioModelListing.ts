import { ListingModelBase } from "./ListingCommon.js";

// `models::configs::AudioModelConfig`
export interface AudioModelListing extends ListingModelBase {
  text_prompt_supported: boolean;
  style_prompt_supported: boolean;
  audio_references_supported: boolean;
  audio_references_max?: number;
  image_references_supported: boolean;
  image_references_max?: number;
  keep_lyrics_supported: boolean;
  instrumental_toggle_supported: boolean;
  loopable_toggle_supported: boolean;
  bpm_supported: boolean;
  musical_key_supported: boolean;
  sample_rate_hz_options: number[];
  sample_rate_hz_default?: number;
  speed_supported: boolean;
  volume_supported: boolean;
  pitch_supported: boolean;
}
