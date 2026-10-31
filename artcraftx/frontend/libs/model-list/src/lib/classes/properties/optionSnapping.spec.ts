import { CommonAspectRatio } from "./CommonAspectRatio.js";
import { CommonResolution } from "./CommonResolution.js";
import {
  nearestAspectRatio,
  nearestNumber,
  nearestResolution,
  nearestResolutionLabel,
} from "./optionSnapping.js";

describe("nearestResolution", () => {
  const oneTwoFour = [CommonResolution.OneK, CommonResolution.TwoK, CommonResolution.FourK];

  it("keeps a supported resolution", () => {
    expect(nearestResolution(CommonResolution.TwoK, oneTwoFour)).toBe(CommonResolution.TwoK);
  });

  it("snaps to the nearest tier, preferring the cheaper one on a tie", () => {
    // 3K sits exactly between 2K and 4K.
    expect(nearestResolution(CommonResolution.ThreeK, oneTwoFour)).toBe(CommonResolution.TwoK);
    expect(nearestResolution(CommonResolution.HalfK, oneTwoFour)).toBe(CommonResolution.OneK);
    expect(nearestResolution(CommonResolution.FourK, [CommonResolution.OneK, CommonResolution.TwoK])).toBe(CommonResolution.TwoK);
  });

  it("crosses the image / video vocabularies", () => {
    expect(nearestResolution(CommonResolution.OneK, [CommonResolution.FourEightyP, CommonResolution.SevenTwentyP, CommonResolution.TenEightyP])).toBe(CommonResolution.SevenTwentyP);
    expect(nearestResolution(CommonResolution.TenEightyP, oneTwoFour)).toBe(CommonResolution.OneK);
  });

  it("is undefined with nothing supported", () => {
    expect(nearestResolution(CommonResolution.OneK, [])).toBeUndefined();
  });
});

describe("nearestResolutionLabel", () => {
  it("snaps video labels", () => {
    expect(nearestResolutionLabel("4K", ["480p", "720p", "1080p"])).toBe("1080p");
    expect(nearestResolutionLabel("480p", ["720p", "1080p"])).toBe("720p");
    expect(nearestResolutionLabel("720p", ["720p"])).toBe("720p");
    expect(nearestResolutionLabel("1080p", ["720p", "4K"])).toBe("720p");
  });

  it("gives up on unknown labels", () => {
    expect(nearestResolutionLabel("huge", ["720p"])).toBeUndefined();
  });
});

describe("nearestNumber", () => {
  it("clamps and snaps durations", () => {
    const seedance = Array.from({ length: 12 }, (_, i) => i + 4); // 4..15
    expect(nearestNumber(30, seedance)).toBe(15);
    expect(nearestNumber(1, seedance)).toBe(4);
    expect(nearestNumber(8, seedance)).toBe(8);
    expect(nearestNumber(7, [5, 10])).toBe(5);
    expect(nearestNumber(8, [5, 10])).toBe(10);
    expect(nearestNumber(8, [])).toBeUndefined();
  });
});

describe("nearestAspectRatio", () => {
  const seedream = [
    CommonAspectRatio.Square,
    CommonAspectRatio.WideFourByThree,
    CommonAspectRatio.TallThreeByFour,
    CommonAspectRatio.WideSixteenByNine,
    CommonAspectRatio.WideTwentyOneByNine,
    CommonAspectRatio.TallNineBySixteen,
    CommonAspectRatio.TallTwoByThree,
    CommonAspectRatio.WideThreeByTwo,
  ];

  it("keeps a supported ratio", () => {
    expect(nearestAspectRatio(CommonAspectRatio.WideSixteenByNine, seedream)).toBe(CommonAspectRatio.WideSixteenByNine);
  });

  it("snaps by numeric ratio", () => {
    expect(nearestAspectRatio(CommonAspectRatio.SquareHd, seedream)).toBe(CommonAspectRatio.Square);
    expect(nearestAspectRatio(CommonAspectRatio.WideFiveByFour, seedream)).toBe(CommonAspectRatio.WideFourByThree);
    expect(nearestAspectRatio(CommonAspectRatio.TallNineByTwentyOne, seedream)).toBe(CommonAspectRatio.TallNineBySixteen);
    expect(nearestAspectRatio(CommonAspectRatio.Wide, seedream)).toBe(CommonAspectRatio.WideSixteenByNine);
  });

  it("maps auto requests onto an auto option when there is one, else the fallback", () => {
    expect(nearestAspectRatio(CommonAspectRatio.Auto2k, [CommonAspectRatio.Auto, CommonAspectRatio.Square])).toBe(CommonAspectRatio.Auto);
    expect(nearestAspectRatio(CommonAspectRatio.Auto, seedream, CommonAspectRatio.WideSixteenByNine)).toBe(CommonAspectRatio.WideSixteenByNine);
    expect(nearestAspectRatio(CommonAspectRatio.Auto, seedream)).toBe(CommonAspectRatio.Square);
    expect(nearestAspectRatio(CommonAspectRatio.Auto, [])).toBeUndefined();
  });
});
