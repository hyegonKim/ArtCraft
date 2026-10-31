// "Nearest supported option" helpers. When a model (or the provider it runs
// on) doesn't offer the option the user had selected, snap to the closest
// one it does offer instead of silently jumping to the default — a 4K
// selection becomes 2K on a 1K/2K provider, a 20s clip becomes 15s, etc.
import { CommonAspectRatio } from "./CommonAspectRatio.js";
import { CommonResolution } from "./CommonResolution.js";

// One ladder for image (`*K`) and video (`*p`) tiers so they compare.
const RESOLUTION_RANK: Record<CommonResolution, number> = {
  [CommonResolution.HalfK]: 0,
  [CommonResolution.FourEightyP]: 1,
  [CommonResolution.SevenTwentyP]: 2,
  [CommonResolution.OneK]: 3,
  [CommonResolution.TenEightyP]: 4,
  [CommonResolution.TwoK]: 5,
  [CommonResolution.ThreeK]: 6,
  [CommonResolution.FourK]: 7,
};

// The video prompt box keeps resolutions as display labels.
const RESOLUTION_LABEL_RANK: Record<string, number> = {
  "0.5K": 0,
  "480p": 1,
  "720p": 2,
  "1K": 3,
  "1080p": 4,
  "2K": 5,
  "3K": 6,
  "4K": 7,
};

// Nearest by numeric distance; ties go to the lower value (the cheaper tier).
const nearestByValue = <T>(
  requested: number,
  candidates: T[],
  valueOf: (candidate: T) => number | undefined,
): T | undefined => {
  let best: T | undefined;
  let bestDistance = Infinity;
  let bestValue = Infinity;
  for (const candidate of candidates) {
    const value = valueOf(candidate);
    if (value === undefined) continue;
    const distance = Math.abs(value - requested);
    if (distance < bestDistance || (distance === bestDistance && value < bestValue)) {
      best = candidate;
      bestDistance = distance;
      bestValue = value;
    }
  }
  return best;
};

// The supported resolution closest to `requested`; `requested` itself when
// it's supported; `undefined` when nothing is supported.
export const nearestResolution = (
  requested: CommonResolution,
  supported: CommonResolution[],
): CommonResolution | undefined => {
  if (supported.includes(requested)) return requested;
  return nearestByValue(RESOLUTION_RANK[requested], supported, (r) => RESOLUTION_RANK[r]);
};

// Same, over the video prompt box's resolution labels ("480p", "2K", ...).
// Labels this build doesn't know rank nowhere and are skipped.
export const nearestResolutionLabel = (
  requested: string,
  supported: string[],
): string | undefined => {
  if (supported.includes(requested)) return requested;
  const rank = RESOLUTION_LABEL_RANK[requested];
  if (rank === undefined) return undefined;
  return nearestByValue(rank, supported, (label) => RESOLUTION_LABEL_RANK[label]);
};

// The supported number closest to `requested` (durations, batch counts).
export const nearestNumber = (requested: number, supported: number[]): number | undefined => {
  if (supported.includes(requested)) return requested;
  return nearestByValue(requested, supported, (n) => n);
};

const ASPECT_RATIO_VALUE: Partial<Record<CommonAspectRatio, number>> = {
  [CommonAspectRatio.Square]: 1,
  [CommonAspectRatio.SquareHd]: 1,
  [CommonAspectRatio.WideThreeByTwo]: 3 / 2,
  [CommonAspectRatio.WideFourByThree]: 4 / 3,
  [CommonAspectRatio.WideFiveByFour]: 5 / 4,
  [CommonAspectRatio.WideSixteenByNine]: 16 / 9,
  [CommonAspectRatio.Wide]: 16 / 9,
  [CommonAspectRatio.WideTwentyOneByNine]: 21 / 9,
  [CommonAspectRatio.TallTwoByThree]: 2 / 3,
  [CommonAspectRatio.TallThreeByFour]: 3 / 4,
  [CommonAspectRatio.TallFourByFive]: 4 / 5,
  [CommonAspectRatio.TallNineBySixteen]: 9 / 16,
  [CommonAspectRatio.Tall]: 9 / 16,
  [CommonAspectRatio.TallNineByTwentyOne]: 9 / 21,
};

export const isAutoAspectRatio = (ratio: CommonAspectRatio): boolean =>
  ASPECT_RATIO_VALUE[ratio] === undefined;

// The supported ratio closest to `requested` by width/height value. An auto
// request keeps an auto option when one is supported (any of them), else it
// falls to `fallback` (typically the model's default) or the first option.
export const nearestAspectRatio = (
  requested: CommonAspectRatio,
  supported: CommonAspectRatio[],
  fallback?: CommonAspectRatio,
): CommonAspectRatio | undefined => {
  if (supported.includes(requested)) return requested;
  if (supported.length === 0) return undefined;
  const value = ASPECT_RATIO_VALUE[requested];
  if (value === undefined) {
    const anyAuto = supported.find(isAutoAspectRatio);
    if (anyAuto) return anyAuto;
    return fallback && supported.includes(fallback) ? fallback : supported[0];
  }
  return nearestByValue(value, supported, (r) => ASPECT_RATIO_VALUE[r])
    ?? (fallback && supported.includes(fallback) ? fallback : supported[0]);
};
