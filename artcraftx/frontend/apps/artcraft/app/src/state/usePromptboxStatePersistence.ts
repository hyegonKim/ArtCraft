// Keeps each prompt box's selection (account, model, options) in the backend's
// `promptbox_state.json` so it survives restarts.
//
//  - On boot, once the model lists AND the account list have loaded, hydrate:
//    the remembered model->account map, the selected account, each page's
//    model (only if that page still offers it), and each modality's options
//    (only fields we know, with the right primitive type). Anything invalid
//    is skipped — hydration never throws.
//  - Afterwards, subscribe to the selection and prompt stores and write back
//    (debounced) whatever changed. Hydration itself doesn't write: the first
//    snapshot after it becomes the baseline.
import { useEffect } from "react";
import {
  GetPromptboxState,
  ModalityPromptboxState,
  PromptboxModality,
  UpdatePromptboxState,
  UpdatePromptboxStateRequest,
  useModelsStore,
} from "@storyteller/tauri-api";
import {
  ModelList,
  ModelPage,
  buildImageTo3dObjectPageList,
  buildImageTo3dWorldPageList,
  buildImageToVideoPageList,
  buildTextToImagePageList,
  useClassyModelSelectorStore,
} from "@storyteller/ui-model-selector";
import {
  usePromptAudioStore,
  usePromptImageStore,
  usePromptVideoStore,
} from "@storyteller/ui-promptbox";

const WRITE_DEBOUNCE_MS = 400;

// Modalities backed by the shared model selector, and the page each one's
// prompt box lives on.
const SELECTOR_PAGES: { modality: PromptboxModality; page: ModelPage }[] = [
  { modality: "image", page: ModelPage.TextToImage },
  { modality: "video", page: ModelPage.ImageToVideo },
  { modality: "splat", page: ModelPage.ImageTo3DWorld },
  { modality: "mesh", page: ModelPage.ImageTo3DObject },
];

export const usePromptboxStatePersistence = () => {
  const modelsLoaded = useModelsStore((s) => s.loaded);
  const accountsLoaded = useClassyModelSelectorStore((s) => s.accountsLoaded);

  useEffect(() => {
    if (!modelsLoaded || !accountsLoaded) return;

    let cancelled = false;
    let unsubscribe: (() => void) | undefined;

    (async () => {
      try {
        const { state } = await GetPromptboxState();
        if (cancelled) return;
        hydrate(state);
      } catch (err) {
        console.warn("[promptbox] could not load persisted state; starting fresh:", err);
      }
      if (!cancelled) unsubscribe = startPersisting();
    })();

    return () => {
      cancelled = true;
      unsubscribe?.();
    };
    // Runs once both are loaded; later reloads don't re-hydrate.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [modelsLoaded, accountsLoaded]);
};

// ── Hydration ──

const hydrate = (state: { last_account_by_model?: Record<string, string> } & Partial<Record<PromptboxModality, ModalityPromptboxState>>) => {
  const selection = useClassyModelSelectorStore.getState();
  const models = useModelsStore.getState();

  if (isStringMap(state.last_account_by_model)) {
    selection.setLastAccountByModel(state.last_account_by_model);
  }

  // One account is shared by every page; take the first modality that saved one.
  const savedAccount = (["image", "video", "splat", "mesh", "audio"] as PromptboxModality[])
      .map((m) => state[m]?.selected_account_id)
      .find((id): id is string => typeof id === "string" && selection.accounts.some((a) => a.id === id));
  if (savedAccount) {
    selection.setSelectedAccountId(savedAccount);
  }

  const pageModels: Partial<Record<ModelPage, ModelList>> = {
    [ModelPage.TextToImage]: buildTextToImagePageList(models.imageModels),
    [ModelPage.ImageToVideo]: buildImageToVideoPageList(models.videoModels),
    [ModelPage.ImageTo3DWorld]: buildImageTo3dWorldPageList(models.splatModels),
    [ModelPage.ImageTo3DObject]: buildImageTo3dObjectPageList(models.object3DModels),
  };
  for (const { modality, page } of SELECTOR_PAGES) {
    const savedModel = state[modality]?.selected_model;
    if (typeof savedModel !== "string") continue;
    const model = (pageModels[page] ?? []).map((item) => item.model).find((m) => m?.id === savedModel);
    if (model) {
      selection.setSelectedModel(page, model);
    } else {
      console.warn(`[promptbox] ignoring saved ${modality} model "${savedModel}": not offered on that page`);
    }
  }

  const savedAudioModel = state.audio?.selected_model;
  if (typeof savedAudioModel === "string" && models.audioModels.some((m) => m.model === savedAudioModel)) {
    usePromptAudioStore.getState().setSelectedModelId(savedAudioModel);
  }

  applyOptions(asRecord(usePromptImageStore.getState()), state.image?.options, IMAGE_OPTION_FIELDS);
  applyOptions(asRecord(usePromptVideoStore.getState()), state.video?.options, VIDEO_OPTION_FIELDS);
  applyOptions(asRecord(usePromptAudioStore.getState()), state.audio?.options, AUDIO_OPTION_FIELDS);
};

// The option fields each prompt store persists, with the primitive type a
// saved value must have to be accepted.
type FieldType = "string" | "number" | "boolean";
type FieldSpec = Record<string, FieldType | FieldType[]>;

const IMAGE_OPTION_FIELDS: FieldSpec = {
  aspectRatio: "string",
  resolution: "string",
  useSystemPrompt: "boolean",
  generationCount: "number",
  commonAspectRatio: "string",
  commonResolution: "string",
  commonQuality: "string",
};

const VIDEO_OPTION_FIELDS: FieldSpec = {
  resolution: "string",
  aspectRatio: "string",
  useSystemPrompt: "boolean",
  generateWithSound: "boolean",
  duration: "number",
  inputMode: "string",
  generationCount: "number",
};

const AUDIO_OPTION_FIELDS: FieldSpec = {
  isInstrumental: "boolean",
  keepLyrics: "boolean",
  isLoopable: "boolean",
  bpm: "number",
  musicalKey: "string",
  sampleRateHz: "number",
  speed: "number",
  volume: "number",
  pitch: "number",
};

// Set each known, well-typed saved option through the store's setter
// (`setFoo` for field `foo`). Unknown or mistyped values are skipped.
const applyOptions = (store: Record<string, unknown>, saved: Record<string, unknown> | undefined, spec: FieldSpec) => {
  if (!saved || typeof saved !== "object") return;
  for (const [field, expected] of Object.entries(spec)) {
    if (!(field in saved)) continue;
    const value = saved[field];
    const allowed = Array.isArray(expected) ? expected : [expected];
    if (!allowed.includes(typeof value as FieldType)) continue;
    const setter = store[`set${field[0].toUpperCase()}${field.slice(1)}`];
    if (typeof setter === "function") setter(value);
  }
};

// The zustand stores are plain objects of fields + setters; read them generically.
const asRecord = (store: object): Record<string, unknown> => store as unknown as Record<string, unknown>;

const isStringMap = (value: unknown): value is Record<string, string> =>
  typeof value === "object" && value !== null && Object.values(value).every((v) => typeof v === "string");

// ── Persistence ──

const pickOptions = (store: Record<string, unknown>, spec: FieldSpec): Record<string, unknown> => {
  const out: Record<string, unknown> = {};
  for (const field of Object.keys(spec)) {
    const value = store[field];
    if (value !== undefined && value !== null) out[field] = value;
  }
  return out;
};

// The complete state to persist, as the backend stores it.
const snapshot = (): UpdatePromptboxStateRequest => {
  const selection = useClassyModelSelectorStore.getState();
  const accountId = selection.selectedAccountId ?? undefined;
  const optionsFor: Partial<Record<PromptboxModality, Record<string, unknown>>> = {
    image: pickOptions(asRecord(usePromptImageStore.getState()), IMAGE_OPTION_FIELDS),
    video: pickOptions(asRecord(usePromptVideoStore.getState()), VIDEO_OPTION_FIELDS),
    audio: pickOptions(asRecord(usePromptAudioStore.getState()), AUDIO_OPTION_FIELDS),
  };

  const modalityState = (modality: PromptboxModality, selectedModel: string | undefined): ModalityPromptboxState => ({
    selected_account_id: accountId,
    selected_model: selectedModel,
    options: optionsFor[modality] ?? {},
  });
  const selectorModel = (page: ModelPage) => selection.selectedModels[page]?.id;

  return {
    image: modalityState("image", selectorModel(ModelPage.TextToImage)),
    video: modalityState("video", selectorModel(ModelPage.ImageToVideo)),
    splat: modalityState("splat", selectorModel(ModelPage.ImageTo3DWorld)),
    mesh: modalityState("mesh", selectorModel(ModelPage.ImageTo3DObject)),
    audio: modalityState("audio", usePromptAudioStore.getState().selectedModelId ?? undefined),
    last_account_by_model: selection.lastAccountByModel,
  };
};

// Subscribe to every store that feeds the snapshot; write the whole snapshot
// (debounced) whenever it differs from the last one written.
const startPersisting = (): (() => void) => {
  let lastWritten = JSON.stringify(snapshot());

  let timer: number | undefined;
  const flush = () => {
    timer = undefined;
    const state = snapshot();
    const json = JSON.stringify(state);
    if (json === lastWritten) return;
    lastWritten = json;
    UpdatePromptboxState(state).catch((err) => {
      console.warn("[promptbox] could not persist state:", err);
      lastWritten = ""; // Try again on the next change.
    });
  };
  const schedule = () => {
    if (timer !== undefined) window.clearTimeout(timer);
    timer = window.setTimeout(flush, WRITE_DEBOUNCE_MS);
  };

  const unsubscribers = [
    useClassyModelSelectorStore.subscribe(schedule),
    usePromptImageStore.subscribe(schedule),
    usePromptVideoStore.subscribe(schedule),
    usePromptAudioStore.subscribe(schedule),
  ];
  return () => {
    if (timer !== undefined) window.clearTimeout(timer);
    for (const unsubscribe of unsubscribers) unsubscribe();
  };
};
