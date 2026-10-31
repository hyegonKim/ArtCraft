import { ImageModel } from "../classes/ImageModel.js";
import { ImageModelListing } from "../listing/ImageModelListing.js";
import {
  aspectRatioFromListing,
  aspectRatiosFromListing,
  creatorFromListing,
  promptMaxLengthFromListing,
  providersFromListing,
  qualitiesFromListing,
  qualityFromListing,
  resolutionFromListing,
  resolutionsFromListing,
  tagsFromListing,
} from "./fromListingCommon.js";

// Build an ImageModel from the backend's config and the providers offering it
// (first = default). The model id doubles as the frontend id and the Tauri id.
export const imageModelFromListing = (m: ImageModelListing, providers: string[]): ImageModel => {
  const aspectRatios = aspectRatiosFromListing(m.aspect_ratio_options);
  const resolutions = resolutionsFromListing(m.resolution_options);
  return new ImageModel({
    id: m.model,
    tauriId: m.model,
    fullName: m.full_name,
    category: "image",
    creator: creatorFromListing(m.model_creator),
    selectorName: m.selector_name,
    selectorDescription: m.selector_description,
    selectorBadges: m.selector_badges,
    tags: tagsFromListing(m.tags),
    providers: providersFromListing(providers),
    progressBarTime: m.progress_bar_ms,
    maxPromptLength: promptMaxLengthFromListing(m.text_prompt_max_length),
    maxGenerationCount: m.batch_size_max,
    defaultGenerationCount: m.batch_size_default,
    predefinedGenerationCounts: m.batch_size_options,
    canEditImages: m.can_edit_images,
    usesInpaintingMask: m.uses_inpainting_mask,
    editingIsInpainting: m.editing_is_inpainting,
    canUseImagePrompt: m.image_refs_supported,
    maxImagePromptCount: m.image_refs_max ?? 1,
    canTextToImage: m.can_text_to_image,
    canEditAngles: m.can_edit_angles,
    canChangeAspectRatio: aspectRatios.length > 0,
    aspectRatios,
    defaultAspectRatio: aspectRatioFromListing(m.aspect_ratio_default),
    canChangeResolution: resolutions.length > 0,
    resolutions,
    defaultResolution: resolutionFromListing(m.resolution_default),
    qualityOptions: qualitiesFromListing(m.quality_options),
    defaultQuality: qualityFromListing(m.quality_default),
  });
};
