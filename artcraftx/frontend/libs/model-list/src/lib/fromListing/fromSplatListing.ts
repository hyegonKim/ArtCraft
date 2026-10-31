import { SplatModel } from "../classes/SplatModel.js";
import { SplatModelListing } from "../listing/SplatModelListing.js";
import {
  creatorFromListing,
  promptMaxLengthFromListing,
  providersFromListing,
  tagsFromListing,
} from "./fromListingCommon.js";

export const splatModelFromListing = (m: SplatModelListing, providers: string[]): SplatModel =>
  new SplatModel({
    id: m.model,
    tauriId: m.model,
    fullName: m.full_name,
    category: "gaussian",
    creator: creatorFromListing(m.model_creator),
    selectorName: m.selector_name,
    selectorDescription: m.selector_description,
    selectorBadges: m.selector_badges,
    tags: tagsFromListing(m.tags),
    providers: providersFromListing(providers),
    progressBarTime: m.progress_bar_ms,
    maxPromptLength: promptMaxLengthFromListing(m.text_prompt_max_length),
  });
