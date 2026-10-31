import { useEffect, useState } from "react";
import { ModelPage } from "@storyteller/ui-model-selector";
import { Model } from "@storyteller/model-list";
import {
  EstimateMeshCost,
  isEstimateMeshCostSuccess,
} from "@storyteller/tauri-api";
import { useCostBreakdownModalStore } from "./cost-breakdown-modal-store";

const MESH_PAGES = new Set<ModelPage>([ModelPage.ImageTo3DObject]);

export function useMeshCostEstimate(
  activePage: ModelPage,
  selectedModel: Model | null | undefined,
  _selectedProvider: string | null | undefined,
): { isLoading: boolean } {
  const [isLoading, setIsLoading] = useState(false);
  const setEstimatedCreditsForPage = useCostBreakdownModalStore(
    (s) => s.setEstimatedCreditsForPage,
  );

  useEffect(() => {
    if (!MESH_PAGES.has(activePage) || !selectedModel) {
      return;
    }

    const commonModel = selectedModel.tauriId;
    if (!commonModel) {
      setEstimatedCreditsForPage(activePage, null);
      return;
    }

    setIsLoading(true);

    EstimateMeshCost({
      model: commonModel,
      // Mesh generation is image-driven; assume one reference image so the
      // estimate matches what the page will actually send.
      reference_image_media_tokens: ["m_placeholder"],
    })
      .then((result) => {
        if (isEstimateMeshCostSuccess(result)) {
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
  }, [activePage, selectedModel, setEstimatedCreditsForPage]);

  return { isLoading };
}
