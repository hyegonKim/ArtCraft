import { GenerationProvider } from "@storyteller/api-enums";
import { ImageModel, ModelCreator } from "@storyteller/model-list";
import {
  chooseAccountForModel,
  chooseAccountForProvider,
  chooseModelForProvider,
  chooseProviderForModel,
  providerForService,
} from "./selection-rules";

const model = (id: string, providers: GenerationProvider[]) =>
  new ImageModel({
    id, tauriId: id, fullName: id, category: "image", creator: ModelCreator.ArtCraft,
    selectorName: id, selectorDescription: "", selectorBadges: [],
    maxGenerationCount: 1, defaultGenerationCount: 1, providers,
  });

const midjourney7 = model("midjourney_7", [GenerationProvider.Artcraft, GenerationProvider.Midjourney]);
const grokImagine = model("grok_imagine_image", [GenerationProvider.Grok]);
const flux = model("flux_1_dev", [GenerationProvider.Artcraft, GenerationProvider.Fal]);
const page = [midjourney7, flux, grokImagine];

const accounts = [
  { id: "c_artcraft", service: "artcraft" },
  { id: "c_grok", service: "grok_cookies" },
  { id: "c_mj", service: "midjourney_cookies" },
  { id: "c_runway", service: "runway_cookies" },
];

describe("providerForService", () => {
  it("maps credential services to providers", () => {
    expect(providerForService("grok_cookies")).toBe(GenerationProvider.Grok);
    expect(providerForService("midjourney_cookies")).toBe(GenerationProvider.Midjourney);
    expect(providerForService("artcraft_api")).toBe(GenerationProvider.Artcraft);
    expect(providerForService("higgsfield_cookies")).toBe(GenerationProvider.Higgsfield);
    expect(providerForService("higgsfield")).toBe(GenerationProvider.Higgsfield);
    expect(providerForService("runway_cookies")).toBeUndefined();
    expect(providerForService("worldlabs_cookies")).toBeUndefined();
  });
});

describe("chooseProviderForModel", () => {
  it("keeps the current provider when the model is offered there", () => {
    expect(chooseProviderForModel(midjourney7, GenerationProvider.Midjourney)).toBe(GenerationProvider.Midjourney);
  });
  it("falls back to the model's default provider otherwise", () => {
    expect(chooseProviderForModel(midjourney7, GenerationProvider.Grok)).toBe(GenerationProvider.Artcraft);
    expect(chooseProviderForModel(grokImagine, undefined)).toBe(GenerationProvider.Grok);
  });
});

describe("chooseModelForProvider", () => {
  it("picks the first page model the provider offers", () => {
    expect(chooseModelForProvider(GenerationProvider.Grok, page)).toBe(grokImagine);
    expect(chooseModelForProvider(GenerationProvider.Fal, page)).toBe(flux);
  });
  it("is undefined when the provider offers nothing on the page", () => {
    expect(chooseModelForProvider(GenerationProvider.Higgsfield, page)).toBeUndefined();
  });
});

describe("chooseAccountForModel", () => {
  const artcraft2 = { id: "c_artcraft_2", service: "artcraft" };
  const all = [...accounts, artcraft2];

  it("prefers the account last used with the model, even when the current one could run it", () => {
    // Midjourney v8 was run on the Midjourney account; v7 was last run on ArtCraft #2.
    const pick = chooseAccountForModel(midjourney7, all, "c_mj", { midjourney_7: "c_artcraft_2" });
    expect(pick.account?.id).toBe("c_artcraft_2");
    expect(pick.provider).toBe(GenerationProvider.Artcraft);
  });
  it("ignores a remembered account that no longer exists or can't run the model", () => {
    expect(chooseAccountForModel(midjourney7, all, "c_mj", { midjourney_7: "c_gone" }).account?.id).toBe("c_mj");
    expect(chooseAccountForModel(midjourney7, all, "c_mj", { midjourney_7: "c_grok" }).account?.id).toBe("c_mj");
  });
  it("keeps the current account when compatible, else falls back to the default provider", () => {
    expect(chooseAccountForModel(flux, all, "c_artcraft", {}).account?.id).toBe("c_artcraft");
    const pick = chooseAccountForModel(grokImagine, all, "c_artcraft", {});
    expect(pick.account).toBeUndefined();
    expect(pick.provider).toBe(GenerationProvider.Grok);
  });
});

describe("chooseAccountForProvider", () => {
  it("keeps the current account when it matches the provider", () => {
    expect(chooseAccountForProvider(GenerationProvider.Grok, accounts, "c_grok")?.id).toBe("c_grok");
  });
  it("switches to the first account for the provider otherwise", () => {
    expect(chooseAccountForProvider(GenerationProvider.Midjourney, accounts, "c_grok")?.id).toBe("c_mj");
    expect(chooseAccountForProvider(GenerationProvider.Fal, accounts, "c_grok")).toBeUndefined();
  });
});
