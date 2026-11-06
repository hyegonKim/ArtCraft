import type { Model } from "@storyteller/model-list";
import type { PopoverItem } from "@storyteller/ui-popover";

export const MODEL_PREVIEW_NOTICE =
  "The live model catalog is unavailable. You can browse built-in models, but creation is disabled.";

export function buildPreviewModelItems(
  models: readonly Model[],
  selectedId: string,
): PopoverItem[] {
  return models.map((model) => ({
    action: model.id,
    label: model.fullName,
    description: model.selectorDescription,
    selected: model.id === selectedId,
  }));
}
