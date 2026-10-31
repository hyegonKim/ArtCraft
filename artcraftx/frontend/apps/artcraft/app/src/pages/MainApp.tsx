// Top-level shell for the artcraft app. Always-mounted chrome
// (TopBar, pricing modals, toaster, Tauri event listeners,
// background refresh hooks) lives here, and a single tab-driven
// switch picks the active page below it.

import { useEffect, useState } from "react";
import * as gpu from "detect-gpu";
import { useSignals } from "@preact/signals-react/runtime";

import { TopBar } from "~/components";
import {
  toast,
  Toaster,
  showProgressToast,
  dismissProgressToast,
} from "@storyteller/ui-toaster";
import {
  PricingModal,
  CreditsModal,
  useCreditsModalStore,
} from "@storyteller/ui-pricing-modal";
import {
  isActionReminderOpen,
  actionReminderProps,
  ActionReminderModal,
} from "@storyteller/ui-action-reminder-modal";
import {
  useAppPreferencesSync,
  useFlashFileDownloadErrorEvent,
  useFlashNoticeEvent,
  useProgressNoticeEvent,
  ProgressNoticeState,
  useFlashUserInputErrorEvent,
  useGenerationCompleteEvent,
  useGenerationEnqueueFailureEvent,
  useGenerationEnqueueSuccessEvent,
  useGenerationFailedEvent,
  useTextToImageGenerationCompleteEvent,
} from "@storyteller/tauri-events";
import { usePromptboxStatePersistence } from "~/state/usePromptboxStatePersistence";

import { useBackgroundLoadingMedia } from "~/hooks/useBackgroundLoadingMedia";
import { CredentialErrorModal } from "~/components/CredentialErrorModal/CredentialErrorModal";
import { useTabStore } from "./Stores/TabState";
import { useTextToImageStore } from "./PageImage/TextToImageStore";

import TextToImage from "./PageImage/TextToImage";
import ImageToVideo from "./PageVideo/ImageToVideo";
import CreateAudio from "./PageAudio/CreateAudio";
import { ImageTo3DObject } from "./PageImageTo3DObject";
import { ImageTo3DWorld } from "./PageImageTo3DWorld";
import { PageSettings } from "./PageSettings";
import {
  topNavMediaId,
  topNavMediaUrl,
} from "~/components/signaled/TopBar/TopBar";

export const MainApp = () => {
  useSignals();

  // Background plumbing — should keep running regardless of which tab
  // is active.
  useBackgroundLoadingMedia();

  // Tauri event listeners. Must always be mounted so generation/upload
  // completions are surfaced no matter which tab the user is on.
  // Keep the app-preferences cache (Enter-to-generate, sounds, ...) in sync.
  useAppPreferencesSync();
  // Remember each prompt box's account / model / options across restarts.
  usePromptboxStatePersistence();

  useGenerationEnqueueSuccessEvent();
  useGenerationEnqueueFailureEvent();
  useGenerationCompleteEvent();
  useGenerationFailedEvent();

  const completeBatch = useTextToImageStore((s) => s.completeBatch);
  useTextToImageGenerationCompleteEvent(async (event) => {
    completeBatch(
      event.generated_images || [],
      event.maybe_frontend_subscriber_id,
    );
  });

  useFlashNoticeEvent(async (event) => {
    console.log("Flash notice event received:", event);
    // Informational "notice" flash: blue, ~5 seconds.
    toast(event.message, {
      duration: 5000,
      icon: "\u2139\ufe0f",
      style: {
        background: "#1d4ed8",
        color: "#ffffff",
      },
    });
  });

  // A backend step the user is waiting on (e.g. Higgsfield's IP check):
  // the toast stays up until the matching `finished` event, the user
  // closes it, or a long safety timeout.
  useProgressNoticeEvent(async (event) => {
    console.log("Progress notice event received:", event);
    if (event.state === ProgressNoticeState.Started) {
      showProgressToast(event.notice_id, event.maybe_message ?? "Working\u2026");
    } else {
      dismissProgressToast(event.notice_id);
    }
  });

  useFlashUserInputErrorEvent(async (event) => {
    console.log("Flash user input error event received:", event);
    toast.error(event.message);
  });

  useFlashFileDownloadErrorEvent(async (event) => {
    console.log("Flash file download error event received:", event);
    toast.error(event.message || "File download failed");
  });

  // GPU detection — an app-wide concern, not 3D-only.
  const [, setValidGpu] = useState("unknown");
  useEffect(() => {
    const { getGPUTier } = gpu;
    getGPUTier().then((gpuTier) => {
      console.log("GPU tier", gpuTier);
      let isValid = false;
      const fps = gpuTier.fps || 0;
      if (gpuTier.tier > 1) isValid = true;
      if (fps > 15) isValid = true;
      if (gpuTier.gpu === "apple gpu (Apple GPU)") isValid = true;
      setValidGpu(isValid ? "valid" : "error");
    });
  }, []);

  const { isOpen: isCreditsOpen, closeModal: closeCreditsModal } =
    useCreditsModalStore();

  const currentReminderModalProps = actionReminderProps.value;

  return (
    <div className="w-screen">
      <TopBar />

      <TabBody />

      <CredentialErrorModal />
      <Toaster offsetTop={52} offsetRight={12} zIndex={9999} />
      {currentReminderModalProps && (
        <ActionReminderModal
          isOpen={isActionReminderOpen.value}
          onClose={currentReminderModalProps.onClose}
          reminderType={currentReminderModalProps.reminderType}
          onPrimaryAction={currentReminderModalProps.onPrimaryAction}
          title={currentReminderModalProps.title}
          message={currentReminderModalProps.message}
          primaryActionText={currentReminderModalProps.primaryActionText}
          secondaryActionText={currentReminderModalProps.secondaryActionText}
          onSecondaryAction={currentReminderModalProps.onSecondaryAction}
          isLoading={currentReminderModalProps.isLoading}
          openAiLogo={currentReminderModalProps.openAiLogo}
          primaryActionIcon={currentReminderModalProps.primaryActionIcon}
          primaryActionBtnClassName={
            currentReminderModalProps.primaryActionBtnClassName
          }
        />
      )}
      <PricingModal />
      <CreditsModal isOpen={isCreditsOpen} onClose={closeCreditsModal} />
    </div>
  );
};

const TabBody = () => {
  const tabStore = useTabStore();

  // Every page is a fragment whose top-level children may use
  // position: fixed; the wrapping <div> scopes them so they don't
  // stack as siblings of the TopBar at the MainApp root.
  switch (tabStore.activeTabId) {
    case "IMAGE":
      return (
        <div>
          <TextToImage
            imageMediaId={topNavMediaId.value}
            imageUrl={topNavMediaUrl.value}
          />
        </div>
      );
    case "VIDEO":
      return (
        <div>
          <ImageToVideo />
        </div>
      );
    case "AUDIO":
      return (
        <div>
          <CreateAudio />
        </div>
      );
    case "IMAGE_TO_3D_OBJECT":
      return (
        <div>
          <ImageTo3DObject />
        </div>
      );
    case "IMAGE_TO_3D_WORLD":
      return (
        <div>
          <ImageTo3DWorld />
        </div>
      );
    case "SETTINGS":
      return (
        <div>
          <PageSettings />
        </div>
      );
    default:
      return null;
  }
};
