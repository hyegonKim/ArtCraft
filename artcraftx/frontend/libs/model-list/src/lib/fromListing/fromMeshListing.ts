import { Object3DModel } from "../classes/Object3DModel.js";
import { MeshModelListing } from "../listing/MeshModelListing.js";
import {
  creatorFromListing,
  promptMaxLengthFromListing,
  providersFromListing,
  tagsFromListing,
} from "./fromListingCommon.js";

export const object3DModelFromListing = (m: MeshModelListing, providers: string[]): Object3DModel =>
  new Object3DModel({
    id: m.model,
    tauriId: m.model,
    fullName: m.full_name,
    category: "3d_object",
    creator: creatorFromListing(m.model_creator),
    selectorName: m.selector_name,
    selectorDescription: m.selector_description,
    selectorBadges: m.selector_badges,
    tags: tagsFromListing(m.tags),
    providers: providersFromListing(providers),
    progressBarTime: m.progress_bar_ms,
    maxPromptLength: promptMaxLengthFromListing(m.text_prompt_max_length),
  });
