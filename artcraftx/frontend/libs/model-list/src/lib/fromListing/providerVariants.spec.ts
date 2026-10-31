import { GenerationProvider } from "@storyteller/api-enums";
import { ImageModel } from "../classes/ImageModel.js";
import { CommonResolution } from "../classes/properties/CommonResolution.js";
import { ImageModelListing } from "../listing/ImageModelListing.js";
import { ListingProviderOffering, providersByModelFromOfferings } from "../listing/ListingCommon.js";
import { imageModelFromListing } from "./fromImageListing.js";
import { attachProviderVariants } from "./providerVariants.js";

const listing = (model: string, resolutions: string[]): ImageModelListing => ({
  model,
  model_creator: "google",
  full_name: model,
  selector_name: model,
  selector_description: "",
  selector_badges: [],
  tags: [],
  progress_bar_ms: 1000,
  is_disabled: false,
  can_text_to_image: true,
  can_edit_images: true,
  uses_inpainting_mask: false,
  editing_is_inpainting: false,
  can_edit_angles: false,
  text_prompt_supported: true,
  negative_text_prompt_supported: false,
  image_refs_supported: true,
  image_refs_max: 6,
  has_fixed_editing_aspect_ratio: false,
  aspect_ratio_options: ["square", "wide_sixteen_by_nine"],
  aspect_ratio_default: "wide_sixteen_by_nine",
  resolution_options: resolutions,
  resolution_default: resolutions[0],
  quality_options: [],
  batch_size_min: 1,
  batch_size_max: 4,
  batch_size_default: 1,
} as unknown as ImageModelListing);

const offerings: ListingProviderOffering<ImageModelListing>[] = [
  { provider: "artcraft", models: [{ model: "nano_banana_2" }, { model: "flux_1_dev" }] },
  { provider: "higgsfield", models: [{ model: "nano_banana_2", overrides: listing("nano_banana_2", ["one_k", "two_k", "four_k"]) }] },
];

const build = () => {
  const providers = providersByModelFromOfferings(offerings);
  const base = [
    imageModelFromListing(listing("nano_banana_2", ["half_k", "one_k", "two_k", "four_k"]), providers.get("nano_banana_2") ?? []),
    imageModelFromListing(listing("flux_1_dev", ["one_k"]), providers.get("flux_1_dev") ?? []),
  ];
  attachProviderVariants(base, offerings, providers, imageModelFromListing);
  return base;
};

describe("attachProviderVariants", () => {
  it("builds a variant for providers with overrides and keeps the base for the rest", () => {
    const [nanoBanana2, flux] = build();
    expect(nanoBanana2.hasProviderVariants()).toBe(true);
    expect(flux.hasProviderVariants()).toBe(false);

    const higgsfield = nanoBanana2.forProvider(GenerationProvider.Higgsfield) as ImageModel;
    expect(higgsfield).not.toBe(nanoBanana2);
    expect(higgsfield.id).toBe("nano_banana_2");
    expect(higgsfield.resolutions).toEqual([CommonResolution.OneK, CommonResolution.TwoK, CommonResolution.FourK]);
    // The base still offers 0.5K.
    expect(nanoBanana2.resolutions).toContain(CommonResolution.HalfK);
    // Variants keep the full provider list so selection rules still hold.
    expect(higgsfield.getProviders()).toEqual(nanoBanana2.getProviders());
  });

  it("resolves back to the base for providers without overrides, from either side", () => {
    const [nanoBanana2] = build();
    const higgsfield = nanoBanana2.forProvider(GenerationProvider.Higgsfield);
    expect(nanoBanana2.forProvider(GenerationProvider.Artcraft)).toBe(nanoBanana2);
    expect(higgsfield.forProvider(GenerationProvider.Artcraft)).toBe(nanoBanana2);
    expect(higgsfield.forProvider(undefined)).toBe(nanoBanana2);
    expect(higgsfield.forProvider(GenerationProvider.Higgsfield)).toBe(higgsfield);
  });
});
