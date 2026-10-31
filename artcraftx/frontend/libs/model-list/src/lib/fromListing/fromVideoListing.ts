import { VideoModel } from "../classes/VideoModel.js";
import { VideoModelListing } from "../listing/VideoModelListing.js";
import {
  aspectRatiosFromListing,
  creatorFromListing,
  promptMaxLengthFromListing,
  providersFromListing,
  sizeOptionForAspectRatio,
  sizeOptionForLegacySize,
  tagsFromListing,
  videoResolutionLabel,
} from "./fromListingCommon.js";

// Build a VideoModel from the backend's config and the providers offering it
// (first = default). The model id doubles as the frontend id and the Tauri id.
export const videoModelFromListing = (m: VideoModelListing, providers: string[]): VideoModel => {
  const aspectRatios = aspectRatiosFromListing(m.aspect_ratio_options);
  // Modern models pick a CommonAspectRatio; the first-party Grok / Sora paths
  // keep their coarse landscape/portrait/square picker.
  const sizeOptions =
    aspectRatios.length > 0
      ? aspectRatios.map(sizeOptionForAspectRatio)
      : m.legacy_size_options.map(sizeOptionForLegacySize);
  return new VideoModel({
    id: m.model,
    tauriId: m.model,
    fullName: m.full_name,
    category: "video",
    creator: creatorFromListing(m.model_creator),
    selectorName: m.selector_name,
    selectorDescription: m.selector_description,
    selectorBadges: m.selector_badges,
    tags: tagsFromListing(m.tags),
    providers: providersFromListing(providers),
    progressBarTime: m.progress_bar_ms,
    maxPromptLength: promptMaxLengthFromListing(m.text_prompt_max_length),
    supportsSystemPrompt: m.supports_system_prompt,
    startFrame: m.starting_keyframe_supported,
    endFrame: m.ending_keyframe_supported,
    requiresImage: m.starting_keyframe_required,
    textToVideoSupported: m.text_to_video_supported,
    generateWithSound: m.show_generate_with_sound_toggle,
    durationOptions: m.duration_seconds_options,
    defaultDuration: m.duration_seconds_default,
    supportsReferenceMode: m.image_references_supported,
    maxReferenceImages: m.image_references_max,
    maxReferenceVideos: m.video_references_max,
    maxVideoRefDuration: m.video_references_max_total_duration_seconds,
    maxReferenceAudios: m.audio_references_max,
    maxAudioRefDuration: m.audio_references_max_total_duration_seconds,
    resolutionOptions:
      m.resolution_options.length > 0 ? m.resolution_options.map(videoResolutionLabel) : undefined,
    defaultResolution:
      m.resolution_default !== undefined ? videoResolutionLabel(m.resolution_default) : undefined,
    sizeOptions,
    supportsCommonAspectRatio: aspectRatios.length > 0,
  });
};
