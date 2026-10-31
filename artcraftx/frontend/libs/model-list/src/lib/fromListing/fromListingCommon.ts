// Shared pieces for turning backend listings into the frontend model classes.
import { GenerationProvider } from "@storyteller/api-enums";
import { ModelCreator } from "../classes/metadata/ModelCreator.js";
import { ModelTag } from "../classes/metadata/ModelTag.js";
import { SizeIconOption, SizeOption } from "../classes/metadata/SizeOption.js";
import { CommonAspectRatio } from "../classes/properties/CommonAspectRatio.js";
import { CommonQuality } from "../classes/properties/CommonQuality.js";
import { CommonResolution } from "../classes/properties/CommonResolution.js";
import { ListingLegacyVideoSize } from "../listing/ListingCommon.js";

// Backend `model_creator` (snake_case) -> frontend enum. Unknown creators
// (a newer backend) fall back to ArtCraft so the icon still renders.
export const creatorFromListing = (raw: string): ModelCreator => {
  switch (raw) {
    case "alibaba": return ModelCreator.Alibaba;
    case "artcraft": return ModelCreator.ArtCraft;
    case "beeble": return ModelCreator.Beeble;
    case "black_forest_labs": return ModelCreator.BlackForestLabs;
    case "bytedance": return ModelCreator.Bytedance;
    case "deemos": return ModelCreator.Deemos;
    case "fal": return ModelCreator.Fal;
    case "google": return ModelCreator.Google;
    case "grok": return ModelCreator.Grok;
    case "hailuo": return ModelCreator.Hailuo;
    case "higgsfield": return ModelCreator.Higgsfield;
    case "kling": return ModelCreator.Kling;
    case "krea": return ModelCreator.Krea;
    case "meshy": return ModelCreator.Meshy;
    case "midjourney": return ModelCreator.Midjourney;
    case "open_ai": return ModelCreator.OpenAi;
    case "open_art": return ModelCreator.OpenArt;
    case "recraft": return ModelCreator.Recraft;
    case "replicate": return ModelCreator.Replicate;
    case "runway": return ModelCreator.Runway;
    case "stability": return ModelCreator.Stability;
    case "suno": return ModelCreator.Suno;
    case "tencent": return ModelCreator.Tencent;
    case "tensor_art": return ModelCreator.TensorArt;
    case "tripo": return ModelCreator.Tripo;
    case "vidu": return ModelCreator.Vidu;
    case "world_labs": return ModelCreator.WorldLabs;
    default:
      console.warn(`[model-list] unknown model creator "${raw}"; using ArtCraft`);
      return ModelCreator.ArtCraft;
  }
};

const KNOWN_PROVIDERS: Set<string> = new Set(Object.values(GenerationProvider));
const KNOWN_TAGS: Set<string> = new Set(Object.values(ModelTag));
const KNOWN_ASPECT_RATIOS: Set<string> = new Set(Object.values(CommonAspectRatio));
const KNOWN_RESOLUTIONS: Set<string> = new Set(Object.values(CommonResolution));
const KNOWN_QUALITIES: Set<string> = new Set(Object.values(CommonQuality));

// The wire values equal the enum values; drop anything this build doesn't know.
export const providersFromListing = (raw: string[]): GenerationProvider[] =>
  raw.filter((p) => KNOWN_PROVIDERS.has(p)) as GenerationProvider[];
export const tagsFromListing = (raw: string[]): ModelTag[] =>
  raw.filter((t) => KNOWN_TAGS.has(t)) as ModelTag[];
export const aspectRatiosFromListing = (raw: string[]): CommonAspectRatio[] =>
  raw.filter((v) => KNOWN_ASPECT_RATIOS.has(v)) as CommonAspectRatio[];
export const aspectRatioFromListing = (raw: string | undefined): CommonAspectRatio | undefined =>
  raw !== undefined && KNOWN_ASPECT_RATIOS.has(raw) ? (raw as CommonAspectRatio) : undefined;
export const resolutionsFromListing = (raw: string[]): CommonResolution[] =>
  raw.filter((v) => KNOWN_RESOLUTIONS.has(v)) as CommonResolution[];
export const resolutionFromListing = (raw: string | undefined): CommonResolution | undefined =>
  raw !== undefined && KNOWN_RESOLUTIONS.has(raw) ? (raw as CommonResolution) : undefined;
export const qualitiesFromListing = (raw: string[]): CommonQuality[] =>
  raw.filter((v) => KNOWN_QUALITIES.has(v)) as CommonQuality[];
export const qualityFromListing = (raw: string | undefined): CommonQuality | undefined =>
  raw !== undefined && KNOWN_QUALITIES.has(raw) ? (raw as CommonQuality) : undefined;

// Absent on the wire means "no limit".
export const promptMaxLengthFromListing = (raw: number | undefined): number =>
  raw ?? Infinity;

// A video size-picker option for a CommonAspectRatio value. `tauriValue` is
// the serde string sent in the request.
export const sizeOptionForAspectRatio = (value: string): SizeOption => {
  switch (value) {
    case "square":
      return { tauriValue: value, textLabel: "1:1", icon: SizeIconOption.Square };
    case "square_hd":
      return { tauriValue: value, textLabel: "1:1 HD", icon: SizeIconOption.Square };
    case "wide_sixteen_by_nine":
      return { tauriValue: value, textLabel: "16:9", icon: SizeIconOption.Landscape16x9 };
    case "tall_nine_by_sixteen":
      return { tauriValue: value, textLabel: "9:16", icon: SizeIconOption.Portrait9x16 };
    case "wide_four_by_three":
      return { tauriValue: value, textLabel: "4:3", icon: SizeIconOption.Standard4x3 };
    case "tall_three_by_four":
      return { tauriValue: value, textLabel: "3:4", icon: SizeIconOption.Portrait3x4 };
    case "wide_three_by_two":
      return { tauriValue: value, textLabel: "3:2", icon: SizeIconOption.Landscape };
    case "tall_two_by_three":
      return { tauriValue: value, textLabel: "2:3", icon: SizeIconOption.Portrait };
    case "wide_five_by_four":
      return { tauriValue: value, textLabel: "5:4", icon: SizeIconOption.Landscape };
    case "tall_four_by_five":
      return { tauriValue: value, textLabel: "4:5", icon: SizeIconOption.Portrait };
    case "wide_twenty_one_by_nine":
      return { tauriValue: value, textLabel: "21:9", icon: SizeIconOption.Landscape16x9 };
    case "tall_nine_by_twenty_one":
      return { tauriValue: value, textLabel: "9:21", icon: SizeIconOption.Portrait9x16 };
    case "wide":
      return { tauriValue: value, textLabel: "Landscape", icon: SizeIconOption.Landscape };
    case "tall":
      return { tauriValue: value, textLabel: "Portrait", icon: SizeIconOption.Portrait };
    case "auto":
      return { tauriValue: value, textLabel: "Auto", icon: SizeIconOption.Square };
    default:
      return { tauriValue: value, textLabel: value, icon: SizeIconOption.Square };
  }
};

// The coarse size picker of the first-party Grok / Sora paths.
export const sizeOptionForLegacySize = (value: ListingLegacyVideoSize): SizeOption => {
  switch (value) {
    case "landscape":
      return { tauriValue: value, textLabel: "Landscape", icon: SizeIconOption.Landscape };
    case "portrait":
      return { tauriValue: value, textLabel: "Portrait", icon: SizeIconOption.Portrait };
    case "square":
      return { tauriValue: value, textLabel: "Square", icon: SizeIconOption.Square };
  }
};

// CommonResolution serde value -> the label form the video prompt box uses
// (its RESOLUTION_STRING_TO_COMMON accepts both forms; labels display better).
export const videoResolutionLabel = (value: string): string => {
  switch (value) {
    case "four_eighty_p": return "480p";
    case "seven_twenty_p": return "720p";
    case "ten_eighty_p": return "1080p";
    default: return value;
  }
};
