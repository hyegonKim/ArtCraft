import { useMemo } from "react";
import type { PopoverItem } from "@storyteller/ui-popover";
import { Box, Film, Image } from "lucide-react";
import {
  getCreatorIcon,
  Model,
  ImageModel,
  Object3DModel,
  SplatModel,
  VideoModel,
} from "@storyteller/model-list";
import { ModelTag } from "@storyteller/model-list";
import {
  useImageModels,
  useObject3DModels,
  useSplatModels,
  useVideoModels,
} from "@storyteller/tauri-api";

export type ModelList = Omit<PopoverItem, "selected">[];

const withIcon = (creatorIcon: any, fallback: any) => creatorIcon || fallback;

const buildItems = (models: Model[], fallbackIcon: any) =>
  models.map((model: Model) => ({
    label: model.selectorName,
    icon: withIcon(getCreatorIcon(model.creator), fallbackIcon),
    description: model.selectorDescription,
    modelConfig: model.toLegacyModelConfig(), // Access to full object.
    model: model,
  }));

const sortedBySelectorName = <T extends Model>(models: T[]): T[] => {
  // De-dupe while preserving instances, then sort by selector name.
  const list = Array.from(new Set(models));
  list.sort((a, b) => a.selectorName?.localeCompare(b.selectorName));
  return list;
};

const imageIcon = <Image className="h-4 w-4" />;
const filmIcon = <Film className="h-4 w-4" />;
const cubeIcon = <Box className="h-4 w-4" />;

/**
 * Per-page model subsetting. These pages show a slice of the full model set
 * based on model CAPABILITY FLAGS (canTextToImage, canEditImages, canEditAngles,
 * the InstructiveEdit tag, etc.), all of which the Rust `models` crate serves.
 *
 * Each builder is a pure function over a model list. React components consume
 * them through the `use*PageModelList` hooks, which run them against the
 * backend-loaded lists from `useModelsStore`.
 */

export const buildTextToImagePageList = (imageModels: ImageModel[]): ModelList =>
  buildItems(
    sortedBySelectorName(imageModels.filter((m) => m.canTextToImage)),
    imageIcon,
  );

export const buildCanvas2dPageList = (imageModels: ImageModel[]): ModelList =>
  buildItems(
    sortedBySelectorName(
      imageModels.filter(
        (m) => m.canEditImages || m.tags?.includes(ModelTag.InstructiveEdit),
      ),
    ),
    imageIcon,
  );

export const buildStage3dPageList = (imageModels: ImageModel[]): ModelList =>
  buildItems(
    sortedBySelectorName(
      imageModels.filter((m) => m.tags?.includes(ModelTag.InstructiveEdit)),
    ),
    imageIcon,
  );

export const buildImageEditorPageList = (imageModels: ImageModel[]): ModelList =>
  buildItems(
    sortedBySelectorName(imageModels.filter((m) => m.canEditImages)),
    imageIcon,
  );

export const buildAnglesPageList = (imageModels: ImageModel[]): ModelList =>
  buildItems(
    sortedBySelectorName(imageModels.filter((m) => m.canEditAngles)),
    imageIcon,
  );

export const buildImageToVideoPageList = (videoModels: VideoModel[]): ModelList =>
  buildItems(
    // SwitchX is a VFX/background-change model with its own page; keep it
    // out of the general video selector.
    sortedBySelectorName(videoModels.filter((m) => m.id !== "switch_x")),
    filmIcon,
  );

export const buildImageTo3dWorldPageList = (splatModels: SplatModel[]): ModelList =>
  buildItems(splatModels, cubeIcon);

export const buildImageTo3dObjectPageList = (object3DModels: Object3DModel[]): ModelList =>
  buildItems(object3DModels, cubeIcon);

// ---------------------------------------------------------------------------
// Live hooks. Use these in React components; the lists are empty until
// `loadModelsFromBackend()` completes at app startup.
// ---------------------------------------------------------------------------

export const useTextToImagePageModelList = (): ModelList => {
  const imageModels = useImageModels();
  return useMemo(() => buildTextToImagePageList(imageModels), [imageModels]);
};

export const useCanvas2dPageModelList = (): ModelList => {
  const imageModels = useImageModels();
  return useMemo(() => buildCanvas2dPageList(imageModels), [imageModels]);
};

export const useStage3dPageModelList = (): ModelList => {
  const imageModels = useImageModels();
  return useMemo(() => buildStage3dPageList(imageModels), [imageModels]);
};

export const useImageEditorPageModelList = (): ModelList => {
  const imageModels = useImageModels();
  return useMemo(() => buildImageEditorPageList(imageModels), [imageModels]);
};

export const useAnglesPageModelList = (): ModelList => {
  const imageModels = useImageModels();
  return useMemo(() => buildAnglesPageList(imageModels), [imageModels]);
};

export const useImageToVideoPageModelList = (): ModelList => {
  const videoModels = useVideoModels();
  return useMemo(() => buildImageToVideoPageList(videoModels), [videoModels]);
};

export const useImageTo3dWorldPageModelList = (): ModelList => {
  const splatModels = useSplatModels();
  return useMemo(() => buildImageTo3dWorldPageList(splatModels), [splatModels]);
};

export const useImageTo3dObjectPageModelList = (): ModelList => {
  const object3DModels = useObject3DModels();
  return useMemo(() => buildImageTo3dObjectPageList(object3DModels), [object3DModels]);
};
