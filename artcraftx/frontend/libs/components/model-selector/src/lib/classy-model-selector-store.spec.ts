import { GenerationProvider } from "@storyteller/api-enums";
import { ImageModel, ModelCreator } from "@storyteller/model-list";
import { ModelPage } from "./model-pages";
import { useClassyModelSelectorStore } from "./classy-model-selector-store";

const model = (id: string, providers: GenerationProvider[]) =>
  new ImageModel({
    id, tauriId: id, fullName: id, category: "image", creator: ModelCreator.ArtCraft,
    selectorName: id, selectorDescription: "", selectorBadges: [],
    maxGenerationCount: 1, defaultGenerationCount: 1, providers,
  });

// Nano Banana 2 runs on ArtCraft and Higgsfield; Higgsfield's variant has a
// narrower menu, so it's a distinct instance with the same id.
const nanoBanana2 = model("nano_banana_2", [GenerationProvider.Artcraft, GenerationProvider.Higgsfield]);
const nanoBanana2OnHiggsfield = model("nano_banana_2", [GenerationProvider.Artcraft, GenerationProvider.Higgsfield]);
nanoBanana2.attachProviderVariant(GenerationProvider.Higgsfield, nanoBanana2OnHiggsfield);
const flux = model("flux_1_dev", [GenerationProvider.Artcraft]);

const page = ModelPage.TextToImage;
const accounts = [
  { id: "c_artcraft", service: "artcraft" },
  { id: "c_higgsfield", service: "higgsfield_cookies" },
];

const reset = () => {
  useClassyModelSelectorStore.setState({
    selectedModels: {}, selectedProviders: {}, pageModels: {}, accounts: [], accountsLoaded: false,
    selectedAccountId: null, lastAccountByModel: {},
  });
  const store = useClassyModelSelectorStore.getState();
  store.registerPageModels(page, [nanoBanana2, flux]);
  store.setAccounts(accounts);
};

describe("provider variants in the selection store", () => {
  beforeEach(reset);

  it("holds the provider's variant of the selected model", () => {
    const store = useClassyModelSelectorStore.getState();
    store.setSelectedAccountId("c_artcraft");
    store.setSelectedModel(page, nanoBanana2);
    expect(useClassyModelSelectorStore.getState().selectedModels[page]).toBe(nanoBanana2);

    store.setSelectedAccountId("c_higgsfield");
    const selected = useClassyModelSelectorStore.getState().selectedModels[page];
    expect(selected).toBe(nanoBanana2OnHiggsfield);
    expect(selected?.forProvider(undefined)).toBe(nanoBanana2);
    expect(useClassyModelSelectorStore.getState().selectedProviders[page]?.["nano_banana_2"]).toBe(GenerationProvider.Higgsfield);
  });

  it("re-selecting the base model on a Higgsfield account settles on the variant (no thrash)", () => {
    const store = useClassyModelSelectorStore.getState();
    store.setSelectedAccountId("c_higgsfield");
    store.setSelectedModel(page, nanoBanana2);
    const first = useClassyModelSelectorStore.getState().selectedModels[page];
    expect(first).toBe(nanoBanana2OnHiggsfield);
    // What the picker's "swap stale instance" effect would do if it compared
    // raw identities: select the base again. The result is the same variant,
    // and its base is the list's instance, so the effect has nothing to do.
    store.setSelectedModel(page, nanoBanana2);
    const second = useClassyModelSelectorStore.getState().selectedModels[page];
    expect(second).toBe(first);
    expect(second?.forProvider(undefined)).toBe(nanoBanana2);
  });

  it("switching back to an ArtCraft account restores the base model", () => {
    const store = useClassyModelSelectorStore.getState();
    store.setSelectedAccountId("c_higgsfield");
    store.setSelectedModel(page, nanoBanana2);
    store.setSelectedAccountId("c_artcraft");
    expect(useClassyModelSelectorStore.getState().selectedModels[page]).toBe(nanoBanana2);
  });

  it("models without variants are untouched", () => {
    const store = useClassyModelSelectorStore.getState();
    store.setSelectedAccountId("c_artcraft");
    store.setSelectedModel(page, flux);
    expect(useClassyModelSelectorStore.getState().selectedModels[page]).toBe(flux);
  });
});
