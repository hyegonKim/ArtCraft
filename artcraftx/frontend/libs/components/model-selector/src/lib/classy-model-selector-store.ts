// The selection state shared by every generation page: which model and
// provider each page uses, and which account (credential) the app generates
// with. Every write goes through the rules in `selection-rules.ts`, so the
// combination stays valid: a model is only ever paired with a provider that
// offers it, and with an account that generates through that provider.
//
// The selected model is always the provider's view of it
// (`Model.forProvider()`): when a provider runs a model with narrower menus,
// the page holds that variant, so the prompt boxes' option pickers (which
// key off the model instance) re-validate their choices on provider change.
import { create } from "zustand";
import { ModelPage } from "./model-pages";
import { ImageModel, Model, VideoModel } from "@storyteller/model-list";
import { GenerationProvider } from "@storyteller/api-enums";
import {
  AccountSummary,
  chooseAccountForModel,
  chooseAccountForProvider,
  chooseModelForProvider,
  modelIsOfferedBy,
  providerForService,
} from "./selection-rules";

interface ClassyModelSelectorState {
  selectedModels: { [page in ModelPage]?: Model };
  selectedProviders: { [page in ModelPage]?: { [modelId: string]: GenerationProvider } };
  // The models each page's picker offers; registered by the selectors so the
  // rules can swap to a compatible model on that page.
  pageModels: { [page in ModelPage]?: Model[] };
  // Stored credentials, and the one the toolbar account picker selected
  // (shared across pages). `null` until accounts load or when none exist.
  accounts: AccountSummary[];
  // True once the account picker has delivered the credential list (even an
  // empty one), so persisted selections can be hydrated safely.
  accountsLoaded: boolean;
  selectedAccountId: string | null;
  // The account last used with each model (model id -> credential id). Wins
  // over the current account when a model is re-selected. Persisted.
  lastAccountByModel: Record<string, string>;

  registerPageModels: (page: ModelPage, models: Model[]) => void;
  // Select a model. The account becomes the one last used with it, else the
  // current one if it can run the model, else one for the model's default
  // provider; the provider follows the account.
  setSelectedModel: (page: ModelPage, model: Model) => void;
  // Remember a provider for a model on a page. When that model is the page's
  // selection, the account follows the provider.
  setSelectedProvider: (page: ModelPage, modelId: string, provider: GenerationProvider) => void;
  // Select a provider for the page. If it doesn't offer the current model, the
  // model switches to one it does offer (nothing changes if there is none).
  selectProvider: (page: ModelPage, provider: GenerationProvider) => void;
  setAccounts: (accounts: AccountSummary[]) => void;
  // Select an account. Every page's model switches to one its provider
  // offers (keeping the model when it already does).
  setSelectedAccountId: (id: string | null) => void;
  // Restore the persisted model -> account memory (merged over the current).
  setLastAccountByModel: (entries: Record<string, string>) => void;
}

type Draft = Pick<ClassyModelSelectorState, "selectedModels" | "selectedProviders" | "selectedAccountId" | "lastAccountByModel">;

export const useClassyModelSelectorStore = create<ClassyModelSelectorState>(
  (set, get) => ({
    selectedModels: {},
    selectedProviders: {},
    pageModels: {},
    accounts: [],
    accountsLoaded: false,
    selectedAccountId: null,
    lastAccountByModel: {},

    registerPageModels: (page, models) =>
      set((state) => ({ pageModels: { ...state.pageModels, [page]: models } })),

    setSelectedModel: (page, model) =>
      set((state) => {
        const { account, provider } = chooseAccountForModel(
          model,
          state.accounts,
          state.selectedAccountId,
          state.lastAccountByModel,
        );
        let draft = withModel(state, page, model, provider);
        if (account) {
          draft = { ...draft, selectedAccountId: account.id };
        } else if (provider) {
          draft = withAccountFor(state, draft, provider);
        }
        return remember(draft, page);
      }),

    setSelectedProvider: (page, modelId, provider) =>
      set((state) => {
        const draft = withProvider(state, page, modelId, provider);
        const isPageSelection = state.selectedModels[page]?.id === modelId;
        return isPageSelection ? remember(withAccountFor(state, draft, provider), page) : draft;
      }),

    selectProvider: (page, provider) =>
      set((state) => {
        const current = state.selectedModels[page];
        const model =
          current && modelIsOfferedBy(current, provider)
            ? current
            : chooseModelForProvider(provider, state.pageModels[page] ?? []);
        if (!model) return {}; // The provider offers nothing on this page.
        return remember(withAccountFor(state, withModel(state, page, model, provider), provider), page);
      }),

    setAccounts: (accounts) => {
      set({ accounts, accountsLoaded: true });
      const { selectedAccountId } = get();
      const stillExists = accounts.some((a) => a.id === selectedAccountId);
      if (accounts.length === 0) {
        if (selectedAccountId !== null) get().setSelectedAccountId(null);
      } else if (!stillExists) {
        get().setSelectedAccountId(accounts[0].id);
      }
    },

    setSelectedAccountId: (id) =>
      set((state) => {
        const account = state.accounts.find((a) => a.id === id);
        const provider = account ? providerForService(account.service) : undefined;
        let draft: Draft = { ...pick(state), selectedAccountId: id };
        if (provider === undefined) return draft; // No constraint from this account.
        for (const page of Object.keys(state.selectedModels) as ModelPage[]) {
          const current = state.selectedModels[page];
          if (!current) continue;
          const model = modelIsOfferedBy(current, provider)
            ? current
            : chooseModelForProvider(provider, state.pageModels[page] ?? []);
          if (!model) continue; // Nothing on this page works with the provider; leave it.
          draft = remember(withModel(draft, page, model, provider), page);
        }
        return draft;
      }),

    setLastAccountByModel: (entries) =>
      set((state) => ({ lastAccountByModel: { ...state.lastAccountByModel, ...entries } })),
  })
);

// ── Rule application helpers (pure) ──

const pick = (state: Draft): Draft => ({
  selectedModels: state.selectedModels,
  selectedProviders: state.selectedProviders,
  selectedAccountId: state.selectedAccountId,
  lastAccountByModel: state.lastAccountByModel,
});

// Record that the page's (final) model was used with the (final) account.
const remember = (draft: Draft, page: ModelPage): Draft => {
  const model = draft.selectedModels[page];
  if (!model || !draft.selectedAccountId) return draft;
  if (draft.lastAccountByModel[model.id] === draft.selectedAccountId) return draft;
  return {
    ...draft,
    lastAccountByModel: { ...draft.lastAccountByModel, [model.id]: draft.selectedAccountId },
  };
};

const withProvider = (state: Draft, page: ModelPage, modelId: string, provider: GenerationProvider): Draft => {
  const draft: Draft = {
    ...pick(state),
    selectedProviders: {
      ...state.selectedProviders,
      [page]: { ...(state.selectedProviders[page] ?? {}), [modelId]: provider },
    },
  };
  // The page's selection follows the provider's variant of the model.
  const current = state.selectedModels[page];
  if (current && current.id === modelId) {
    const effective = current.forProvider(provider);
    if (effective !== current) {
      draft.selectedModels = { ...state.selectedModels, [page]: effective };
    }
  }
  return draft;
};

const withModel = (state: Draft, page: ModelPage, model: Model, provider: GenerationProvider | undefined): Draft => {
  const draft: Draft = { ...pick(state), selectedModels: { ...state.selectedModels, [page]: model.forProvider(provider) } };
  return provider ? withProvider(draft, page, model.id, provider) : draft;
};

// Point the account at one that generates through `provider`, if any exists.
const withAccountFor = (state: ClassyModelSelectorState, draft: Draft, provider: GenerationProvider): Draft => {
  const account = chooseAccountForProvider(provider, state.accounts, draft.selectedAccountId);
  return account ? { ...draft, selectedAccountId: account.id } : draft;
};

// ── Non-reactive getters ──

export const getSelectedImageModel = (
  page: ModelPage
): ImageModel | undefined => {
  const { selectedModels } = useClassyModelSelectorStore.getState();
  const maybeModel = selectedModels[page];
  if (!maybeModel) {
    return undefined;
  }
  // NB: We can't use "instanceof" checks with Vite minification and class name mangling.
  // We have to do type tagging a different way.
  if (maybeModel.kind === "image_model") {
    return maybeModel as ImageModel;
  }
  return undefined;
};

export const getSelectedVideoModel = (
  page: ModelPage
): VideoModel | undefined => {
  const { selectedModels } = useClassyModelSelectorStore.getState();
  const maybeModel = selectedModels[page];
  if (!maybeModel) {
    return undefined;
  }
  if (maybeModel.kind !== "video_model") {
    return undefined;
  }
  return maybeModel as VideoModel;
};

export const getSelectedProviderForModel = (
  page: ModelPage,
  modelId: string
): GenerationProvider | undefined => {
  const { selectedProviders } = useClassyModelSelectorStore.getState();
  const byPage = selectedProviders[page];
  if (!byPage) return undefined;
  return byPage[modelId];
};

// ── Reactive hooks for UI subscriptions ──

export const useSelectedModel = (page: ModelPage): Model | undefined =>
  useClassyModelSelectorStore((s) => s.selectedModels[page]);

export const useSelectedImageModel = (
  page: ModelPage
): ImageModel | undefined => {
  const maybeModel = useSelectedModel(page);
  if (!maybeModel) return undefined;
  return maybeModel.kind === "image_model"
    ? (maybeModel as ImageModel)
    : undefined;
};

export const useSelectedVideoModel = (
  page: ModelPage
): VideoModel | undefined => {
  const maybeModel = useSelectedModel(page);
  if (!maybeModel) return undefined;
  return maybeModel.kind === "video_model"
    ? (maybeModel as VideoModel)
    : undefined;
};

// TODO: This shouldn't be on a per-page basis.
export const useSelectedProviderForModel = (
  page: ModelPage,
  modelId: string | undefined
): GenerationProvider | undefined =>
  useClassyModelSelectorStore((s) =>
    modelId ? s.selectedProviders[page]?.[modelId] : undefined
  );

// The account the app generates with (shared across pages).
export const useSelectedAccountId = (): string | null =>
  useClassyModelSelectorStore((s) => s.selectedAccountId);
