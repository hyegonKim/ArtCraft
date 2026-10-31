// The rules that keep a page's (model, provider, account) choice valid.
//
// Pure functions over the loaded models and accounts, so the store can apply
// them and tests can pin them. Every "which one do we swap to" decision goes
// through one of the `choose*` functions below — that's where most-recently-
// used preferences will plug in later (last provider used per model, last
// model used per provider, last account per model/provider).
import { GenerationProvider } from "@storyteller/api-enums";
import { Model } from "@storyteller/model-list";

// The identity of a stored credential the user can generate with.
export interface AccountSummary {
  id: string;
  // Backend `GenerationSource` value, e.g. "grok_cookies", "fal_api".
  service: string;
}

// The provider an account's service generates through. `undefined` for
// services the app can't generate with yet (they impose no constraint).
export const providerForService = (service: string): GenerationProvider | undefined => {
  switch (service) {
    case "artcraft":
    case "artcraft_local":
    case "artcraft_cookies":
    case "artcraft_api":
      return GenerationProvider.Artcraft;
    case "fal":
    case "fal_api":
      return GenerationProvider.Fal;
    case "grok":
    case "grok_cookies":
    case "xai_api":
      return GenerationProvider.Grok;
    case "higgsfield":
    case "higgsfield_cookies":
      return GenerationProvider.Higgsfield;
    case "midjourney":
    case "midjourney_cookies":
      return GenerationProvider.Midjourney;
    default:
      return undefined;
  }
};

export const modelIsOfferedBy = (model: Model, provider: GenerationProvider): boolean =>
  model.getProviders().includes(provider);

// The provider to use for a model: the current one if the model is offered
// there, else the model's default. (Future: the last provider used for it.)
export const chooseProviderForModel = (
  model: Model,
  current: GenerationProvider | undefined,
): GenerationProvider | undefined => {
  if (current !== undefined && modelIsOfferedBy(model, current)) return current;
  return model.getProviders()[0];
};

// The model to switch to when a provider doesn't offer the current one: the
// first model on the page the provider does offer. (Future: the last model
// used with that provider.)
export const chooseModelForProvider = (
  provider: GenerationProvider,
  pageModels: Model[],
): Model | undefined => pageModels.find((m) => modelIsOfferedBy(m, provider));

// The (account, provider) to use when a model is selected:
//   1. the account last used with this model, if it still exists and its
//      provider still offers the model;
//   2. else the current account, if its provider offers the model;
//   3. else the model's default provider (the account follows separately).
export const chooseAccountForModel = (
  model: Model,
  accounts: AccountSummary[],
  currentAccountId: string | null,
  lastAccountByModel: Record<string, string>,
): { account: AccountSummary | undefined; provider: GenerationProvider | undefined } => {
  const remembered = accounts.find((a) => a.id === lastAccountByModel[model.id]);
  const rememberedProvider = remembered ? providerForService(remembered.service) : undefined;
  if (remembered && rememberedProvider && modelIsOfferedBy(model, rememberedProvider)) {
    return { account: remembered, provider: rememberedProvider };
  }
  const current = accounts.find((a) => a.id === currentAccountId);
  const currentProvider = current ? providerForService(current.service) : undefined;
  if (current && currentProvider && modelIsOfferedBy(model, currentProvider)) {
    return { account: current, provider: currentProvider };
  }
  const provider = model.getProviders()[0];
  return { account: undefined, provider };
};

// The account to use for a provider: the current one if it generates through
// that provider, else the first stored account that does.
export const chooseAccountForProvider = (
  provider: GenerationProvider,
  accounts: AccountSummary[],
  currentAccountId: string | null,
): AccountSummary | undefined => {
  const current = accounts.find((a) => a.id === currentAccountId);
  if (current && providerForService(current.service) === provider) return current;
  return accounts.find((a) => providerForService(a.service) === provider);
};
