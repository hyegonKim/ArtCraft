import { useEffect, useState } from "react";
import { ModelPage } from "@storyteller/ui-model-selector";
import { Model } from "@storyteller/model-list";
import {
  EstimateSplatCost,
  isEstimateSplatCostSuccess,
} from "@storyteller/tauri-api";
import { useCostBreakdownModalStore } from "./cost-breakdown-modal-store";

const SPLAT_PAGES = new Set<ModelPage>([ModelPage.ImageTo3DWorld]);

export function useSplatCostEstimate(
  activePage: ModelPage,
  selectedModel: Model | null | undefined,
  selectedProvider: string | null | undefined,
): { isLoading: boolean } {
  const [isLoading, setIsLoading] = useState(false);
  const setEstimatedCreditsForPage = useCostBreakdownModalStore(
    (s) => s.setEstimatedCreditsForPage,
  );

  useEffect(() => {
    if (!SPLAT_PAGES.has(activePage) || !selectedModel) {
      return;
    }

    const commonModel = selectedModel.tauriId;
    if (!commonModel) {
      setEstimatedCreditsForPage(activePage, null);
      return;
    }

    setIsLoading(true);

    EstimateSplatCost({
      model: commonModel,
      // Splat generation is image-driven; assume one reference image so the
      // estimate matches what the page will actually send.
      reference_image_media_tokens: ["m_placeholder"],
    })
      .then((result) => {
        if (isEstimateSplatCostSuccess(result)) {
          setEstimatedCreditsForPage(
            activePage,
            result.payload.cost_in_credits ?? null,
          );
        } else {
          setEstimatedCreditsForPage(activePage, null);
        }
      })
      .catch(() => {
        setEstimatedCreditsForPage(activePage, null);
      })
      .finally(() => {
        setIsLoading(false);
      });
  }, [activePage, selectedModel?.id, selectedProvider]);

  return { isLoading };
}
