import {
  ListingAspectRatio,
  ListingBitrate,
  ListingLegacyVideoSize,
  ListingModelBase,
  ListingQuality,
  ListingResolution,
} from "./ListingCommon.js";

// `models::configs::VideoModelConfig`
export interface VideoModelListing extends ListingModelBase {
  supports_system_prompt: boolean;
  text_to_video_supported: boolean;
  text_prompt_supported: boolean;
  negative_text_prompt_supported: boolean;
  starting_keyframe_supported: boolean;
  starting_keyframe_required: boolean;
  ending_keyframe_supported: boolean;
  image_references_supported: boolean;
  image_references_max?: number;
  video_references_supported: boolean;
  video_references_max?: number;
  video_references_max_total_duration_seconds?: number;
  audio_references_supported: boolean;
  audio_references_max?: number;
  audio_references_max_total_duration_seconds?: number;
  character_references_supported: boolean;
  character_references_max?: number;
  show_generate_with_sound_toggle: boolean;
  aspect_ratio_options: ListingAspectRatio[];
  aspect_ratio_default?: ListingAspectRatio;
  legacy_size_options: ListingLegacyVideoSize[];
  resolution_options: ListingResolution[];
  resolution_default?: ListingResolution;
  bitrate_options: ListingBitrate[];
  bitrate_default?: ListingBitrate;
  quality_options: ListingQuality[];
  quality_default?: ListingQuality;
  duration_seconds_min?: number;
  duration_seconds_max?: number;
  duration_seconds_max_with_image_references?: number;
  duration_seconds_options?: number[];
  duration_seconds_default?: number;
  batch_size_min: number;
  batch_size_max: number;
  batch_size_options?: number[];
  batch_size_default: number;
}
