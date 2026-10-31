import { PromptBoxImage } from "@storyteller/ui-promptbox";
import {
  useTextToImagePageModelList,
  ModelPage,
  ClassyModelSelector,
  useSelectedImageModel,
  useSelectedProviderForModel,
} from "@storyteller/ui-model-selector";
import { ImageModel } from "@storyteller/model-list";
import { useTextToImageStore } from "./TextToImageStore";
import { useCostBreakdownModalStore } from "@storyteller/ui-pricing-modal";
import { GenerationProvider } from "@storyteller/api-enums";
import { Image as ImageIcon } from "lucide-react";
import { PromptShell, useComposerTasks } from "~/components/PromptShell";
import { AccountSelector } from "~/components/account-selector/AccountSelector";
import { useSelectedAccountId } from "@storyteller/ui-model-selector";

const PAGE_ID: ModelPage = ModelPage.TextToImage;

interface TextToImageProps {
  imageMediaId?: string;
  imageUrl?: string;
}

// The whole page is the composer: no feed, no gallery — results are written
// straight to disk and the PromptShell shows the progress bar + receipt.
const TextToImage = ({ imageMediaId, imageUrl }: TextToImageProps) => {
  const textToImageModelList = useTextToImagePageModelList();
  const startBatch = useTextToImageStore((s) => s.startBatch);
  const selectedAccountId = useSelectedAccountId();

  const selectedImageModel: ImageModel | undefined =
    useSelectedImageModel(PAGE_ID);

  const selectedProvider: GenerationProvider | undefined =
    useSelectedProviderForModel(PAGE_ID, selectedImageModel?.id);

  const imageCredits = useCostBreakdownModalStore(
    (s) => s.estimatedCreditsByPage[PAGE_ID],
  );

  const { busy, completed } = useComposerTasks("image");

  return (
    <PromptShell
      icon={<ImageIcon className="h-[17px] w-[17px]" />}
      busy={busy}
      completed={completed}
    >
      <PromptBoxImage
        fullBleed
        selectedModel={selectedImageModel}
        selectedProvider={selectedProvider}
        imageMediaId={imageMediaId}
        url={imageUrl ?? undefined}
        credits={imageCredits}
        accountSelector={<AccountSelector />}
        credentialId={selectedAccountId}
        modelSelector={
          <ClassyModelSelector
            variant="embedded"
            items={textToImageModelList}
            page={PAGE_ID}
          />
        }
        onEnqueuePressed={async (prompt, count, subscriberId) => {
          const modelLabel = selectedImageModel?.fullName ?? "";
          startBatch(
            prompt,
            count,
            modelLabel,
            subscriberId,
            selectedImageModel?.creator,
          );
          // Nudge the TopBar task queue (and this page's activity poll) so
          // the in-flight state appears immediately.
          window.dispatchEvent(new Event("task-queue-update"));
        }}
      />
    </PromptShell>
  );
};

export default TextToImage;
