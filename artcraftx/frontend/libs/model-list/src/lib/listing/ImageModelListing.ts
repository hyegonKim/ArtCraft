import {
  ListingAspectRatio,
  ListingModelBase,
  ListingQuality,
  ListingResolution,
} from "./ListingCommon.js";

// `models::configs::ImageModelConfig`
export interface ImageModelListing extends ListingModelBase {
  can_text_to_image: boolean;
  can_edit_images: boolean;
  uses_inpainting_mask: boolean;
  editing_is_inpainting: boolean;
  can_edit_angles: boolean;
  text_prompt_supported: boolean;
  negative_text_prompt_supported: boolean;
  image_refs_supported: boolean;
  image_refs_max?: number;
  has_fixed_editing_aspect_ratio: boolean;
  aspect_ratio_options: ListingAspectRatio[];
  aspect_ratio_default?: ListingAspectRatio;
  aspect_ratio_default_when_editing?: ListingAspectRatio;
  resolution_options: ListingResolution[];
  resolution_default?: ListingResolution;
  quality_options: ListingQuality[];
  quality_default?: ListingQuality;
  batch_size_min: number;
  batch_size_max: number;
  batch_size_options?: number[];
  batch_size_default: number;
}
