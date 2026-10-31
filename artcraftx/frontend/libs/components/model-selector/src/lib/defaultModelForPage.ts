import { Model } from "@storyteller/model-list";
import { ModelPage } from "./model-pages";

// Model ids (as served by the Rust `models` crate) to preselect per page.
const DEFAULT_MODEL_ID_FOR_PAGE: Partial<Record<ModelPage, string>> = {
  [ModelPage.TextToImage]: "nano_banana_pro",
  [ModelPage.ImageToVideo]: "seedance_2p0",
  [ModelPage.Canvas2D]: "gpt_image_1p5",
  [ModelPage.Stage3D]: "gpt_image_1p5",
  [ModelPage.ImageEditor]: "nano_banana_pro",
  [ModelPage.ImageTo3DWorld]: "marble_1p1",
  [ModelPage.Angles]: "flux_2_lora_angles",
};

// The page's default model from the given (backend-loaded) list, else the
// first model. Returns undefined while the list is still empty.
export const defaultModelForPage = (
  models: Model[],
  page: ModelPage,
): Model | undefined => {
  const defaultId = DEFAULT_MODEL_ID_FOR_PAGE[page];
  if (defaultId) {
    const fromList = models.find(
      (m) => m.id === defaultId || m.tauriId === defaultId,
    );
    if (fromList) return fromList;
  }
  return models[0];
};
