import { ListingModelBase } from "./ListingCommon.js";

// `models::configs::SplatModelConfig`
export interface SplatModelListing extends ListingModelBase {
  text_prompt_supported: boolean;
  image_references_supported: boolean;
  image_references_max?: number;
  video_reference_supported: boolean;
  panorama_supported: boolean;
  disable_recaption_supported: boolean;
}
