// Per-provider model variants. A provider offering can carry `overrides`: a
// replacement config for a model whose menus differ on that provider (e.g.
// Higgsfield's Nano Banana 2 has no 0.5K tier). Each override is built into a
// full model instance and attached to the base model, so the pickers can
// swap to the variant when that provider is selected.
import { GenerationProvider } from "@storyteller/api-enums";
import { Model } from "../classes/Model.js";
import { ListingProviderOffering } from "../listing/ListingCommon.js";

const KNOWN_PROVIDERS: Set<string> = new Set(Object.values(GenerationProvider));

// Attach a variant to every base model that some provider overrides.
// `build` turns an override config into a model the same way the base was
// built; `providersByModel` is the model's full provider list (variants
// keep it, so provider rules see the same offerings).
export const attachProviderVariants = <Listing, M extends Model>(
  baseModels: M[],
  offerings: ListingProviderOffering<Listing>[],
  providersByModel: Map<string, string[]>,
  build: (listing: Listing, providers: string[]) => M,
): void => {
  const byId = new Map(baseModels.map((model) => [model.id, model]));
  for (const offering of offerings) {
    if (!KNOWN_PROVIDERS.has(offering.provider)) continue;
    const provider = offering.provider as GenerationProvider;
    for (const offered of offering.models) {
      if (!offered.overrides) continue;
      const base = byId.get(offered.model);
      if (!base) continue;
      const variant = build(offered.overrides, providersByModel.get(offered.model) ?? []);
      base.attachProviderVariant(provider, variant);
    }
  }
};
