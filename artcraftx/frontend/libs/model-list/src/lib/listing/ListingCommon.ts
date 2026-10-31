// Wire types shared by every modality's listing. These mirror the serde form
// of the Rust `models` crate (`crates/models/src/enums`); the strings are the
// ids the backend accepts on generate requests.

// `models::enums::GenerationProvider`
export type ListingProvider = "artcraft" | "fal" | "grok" | "higgsfield" | "midjourney";

// `models::enums::ModelCreator` (snake_case)
export type ListingCreator = string;

// `models::enums::ModelTag` (camelCase, same values as `ModelTag`)
export type ListingTag = string;

// `models::enums::CommonAspectRatio` / `CommonResolution` / `CommonQuality` /
// `CommonBitrate` (snake_case; same values as the frontend enums)
export type ListingAspectRatio = string;
export type ListingResolution = string;
export type ListingQuality = string;
export type ListingBitrate = string;

// `models::enums::LegacyVideoSize`
export type ListingLegacyVideoSize = "landscape" | "portrait" | "square";

// `models::providers::OfferedModel` — one model as offered by one provider.
export interface ListingOfferedModel<Config = unknown> {
  model: string;
  // This provider's replacement config, when it differs from the base one.
  overrides?: Config;
}

// `models::providers::ProviderOffering` — the models one provider offers.
export interface ListingProviderOffering<Config = unknown> {
  provider: ListingProvider;
  models: ListingOfferedModel<Config>[];
}

// model id -> providers offering it, in offering order (first = default).
export const providersByModelFromOfferings = (
  offerings: ListingProviderOffering[],
): Map<string, ListingProvider[]> => {
  const byModel = new Map<string, ListingProvider[]>();
  for (const offering of offerings) {
    for (const offered of offering.models) {
      const list = byModel.get(offered.model) ?? [];
      if (!list.includes(offering.provider)) list.push(offering.provider);
      byModel.set(offered.model, list);
    }
  }
  return byModel;
};

// Fields every modality's config carries.
export interface ListingModelBase {
  model: string;
  model_creator: ListingCreator;
  full_name: string;
  selector_name: string;
  selector_description: string;
  extra_info?: string;
  selector_badges: string[];
  tags: ListingTag[];
  // (Which providers offer the model lives in the listing's `providers`.)
  progress_bar_ms: number;
  // Absent = no limit.
  text_prompt_max_length?: number;
  is_disabled: boolean;
}
