import { useMemo } from "react";
import { PromptBoxAudio, usePromptAudioStore } from "@storyteller/ui-promptbox";
import {
  useOmniGenAudioModels,
  useAudioCostEstimate,
} from "@storyteller/omni-gen";
import { AudioLines } from "lucide-react";
import { PromptShell, useComposerTasks } from "~/components/PromptShell";
import { AccountSelector } from "~/components/account-selector/AccountSelector";
import { useSelectedAccountId } from "@storyteller/ui-model-selector";

// The whole page is the composer: no feed, no gallery — results are written
// straight to disk and the PromptShell shows the progress bar + receipt.
const CreateAudio = () => {
  const { models } = useOmniGenAudioModels();

  // Cost estimate inputs come from the shared audio prompt store (the
  // promptbox owns the settings; the page only observes them).
  const selectedModelId = usePromptAudioStore((s) => s.selectedModelId);
  const referenceAudios = usePromptAudioStore((s) => s.referenceAudios);
  const referenceImages = usePromptAudioStore((s) => s.referenceImages);
  const selectedModel = useMemo(
    () =>
      models.find((m) => m.model === selectedModelId) ??
      models.find((m) => m.model === "suno_music") ??
      models[0],
    [models, selectedModelId],
  );
  const selectedAccountId = useSelectedAccountId();
  const audioCredits = useAudioCostEstimate({
    model: selectedModel?.model ?? "",
    audioReferenceCount: referenceAudios.length,
    hasImageReference: referenceImages.length > 0,
    sampleRateHz: selectedModel?.sample_rate_hz_options?.length
      ? (selectedModel.sample_rate_hz_default ?? undefined)
      : undefined,
  });

  const { busy, completed } = useComposerTasks("audio");

  return (
    <PromptShell
      icon={<AudioLines className="h-[17px] w-[17px]" />}
      busy={busy}
      completed={completed}
    >
      <PromptBoxAudio
        fullBleed
        models={models}
        credits={audioCredits}
        accountSelector={<AccountSelector />}
        credentialId={selectedAccountId}
        onEnqueuePressed={async () => {
          // Nudge the task-queue hooks so the in-flight state appears
          // immediately.
          window.dispatchEvent(new Event("task-queue-update"));
          window.dispatchEvent(new Event("credits-change"));
        }}
      />
    </PromptShell>
  );
};

export default CreateAudio;
