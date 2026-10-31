// Every model the backend served, across modalities, for non-React lookups
// (e.g. the task queue matching a task's model_type to a progress bar time).
//
// Populated by the models store in @storyteller/tauri-api after each load.
// Bound to `window` rather than a module singleton: our nx production build
// duplicates library modules across bundles (see SoundRegistry), so a plain
// module-level variable would not be shared.
import { Model } from "../classes/Model.js";

const GLOBAL_KEY = "artcraft_loaded_models";

const normalizeKey = (key: string): string =>
  key.toLowerCase().replace(/\./g, "_").trim();

const store = (): Map<string, Model> => {
  const w = window as any;
  if (w[GLOBAL_KEY] === undefined) {
    w[GLOBAL_KEY] = new Map<string, Model>();
  }
  return w[GLOBAL_KEY];
};

// Replace the registry with the given models (all modalities, disabled ones
// included so historical tasks still resolve).
export const registerLoadedModels = (models: Model[]) => {
  const map = store();
  map.clear();
  for (const model of models) {
    map.set(normalizeKey(model.id), model);
    if (model.tauriId) map.set(normalizeKey(model.tauriId), model);
  }
};

// Look up a model by id or Tauri id (dots normalize to underscores so backend
// variants like `kling_1.6_pro` resolve).
export const findLoadedModel = (key: string | null | undefined): Model | undefined => {
  if (!key) return undefined;
  return store().get(normalizeKey(key));
};
