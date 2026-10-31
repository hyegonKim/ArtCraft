import {
  PromptBoxVideo,
  PromptBoxErrorBoundary,
} from "@storyteller/ui-promptbox";
import {
  ClassyModelSelector,
  useImageToVideoPageModelList,
  ModelPage,
  useSelectedVideoModel,
  useSelectedProviderForModel,
} from "@storyteller/ui-model-selector";
import { VideoModel } from "@storyteller/model-list";
import { useImageToVideoStore } from "./ImageToVideoStore";
import {
  useVideoGenerationCompleteEvent,
  VideoGenerationCompleteEvent,
} from "@storyteller/tauri-events";
import { useCostBreakdownModalStore } from "@storyteller/ui-pricing-modal";
import { GenerationProvider } from "@storyteller/api-enums";
import { Clapperboard } from "lucide-react";
import { PromptShell, useComposerTasks } from "~/components/PromptShell";
import { AccountSelector } from "~/components/account-selector/AccountSelector";
import { useSelectedAccountId } from "@storyteller/ui-model-selector";

const PAGE_ID: ModelPage = ModelPage.ImageToVideo;

interface ImageToVideoProps {
  imageMediaId?: string;
  imageUrl?: string;
}

// The whole page is the composer: no feed, no gallery — results are written
// straight to disk and the PromptShell shows the progress bar + receipt.
const ImageToVideo = ({ imageMediaId, imageUrl }: ImageToVideoProps) => {
  const imageToVideoModelList = useImageToVideoPageModelList();
  const startBatch = useImageToVideoStore((s) => s.startBatch);
  const selectedAccountId = useSelectedAccountId();
  const completeBatch = useImageToVideoStore((s) => s.completeBatch);

  const selectedVideoModel: VideoModel | undefined =
    useSelectedVideoModel(PAGE_ID);

  const selectedProvider: GenerationProvider | undefined =
    useSelectedProviderForModel(PAGE_ID, selectedVideoModel?.id);

  const videoCredits = useCostBreakdownModalStore(
    (s) => s.estimatedCreditsByPage[PAGE_ID],
  );

  const { busy, completed } = useComposerTasks("video");

  // Keep the local batch store in sync (other listeners may rely on it).
  useVideoGenerationCompleteEvent(
    async (event: VideoGenerationCompleteEvent) => {
      for (const video of event.generated_videos ?? []) {
        completeBatch(
          {
            cdn_url: video.cdn_url,
            media_token: video.media_token,
          },
          event.maybe_frontend_subscriber_id,
        );
      }
    },
  );

  return (
    <PromptShell
      icon={<Clapperboard className="h-[17px] w-[17px]" />}
      busy={busy}
      completed={completed}
    >
      <PromptBoxErrorBoundary>
        <PromptBoxVideo
          fullBleed
          selectedModel={selectedVideoModel}
          selectedProvider={selectedProvider}
          imageMediaId={imageMediaId}
          url={imageUrl ?? undefined}
          credits={videoCredits}
          accountSelector={<AccountSelector />}
          credentialId={selectedAccountId}
          modelSelector={
            <ClassyModelSelector
              variant="embedded"
              items={imageToVideoModelList}
              page={PAGE_ID}
            />
          }
          onEnqueuePressed={async (prompt, subscriberIds) => {
            const modelLabel = selectedVideoModel?.fullName ?? "";
            for (const subscriberId of subscriberIds) {
              startBatch(prompt, modelLabel, subscriberId);
            }
            // Nudge the TopBar task queue (and this page's activity poll) so
            // the in-flight state appears immediately.
            window.dispatchEvent(new Event("task-queue-update"));
          }}
        />
      </PromptBoxErrorBoundary>
    </PromptShell>
  );
};

export default ImageToVideo;
