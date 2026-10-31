import { useEnterToGenerate } from "@storyteller/tauri-api";
import { useMemo, useRef, useState, ReactNode } from "react";
import { toast } from "@storyteller/ui-toaster";
import {
  GalleryModal,
  type GalleryItem,
} from "@storyteller/ui-gallery-modal";
import { PopoverMenu, PopoverItem } from "@storyteller/ui-popover";
import { Tooltip } from "@storyteller/ui-tooltip";
import { GenerateIconButton, ToggleButton } from "@storyteller/ui-button";
import { ChevronDown, ChevronUp, Mic, MicOff, Repeat } from "lucide-react";
import type { OmniGenAudioModelDetails, UploadMediaFn } from "@storyteller/api";
import {
  buildAudioRequest,
  AUDIO_MODELS_REQUIRING_AUDIO_REF,
  OMNI_GENERATE_OUTAGE_MESSAGE,
  type AudioGenerationSettings,
} from "@storyteller/omni-gen";
import { GenerateAudio, mediaSourcesFromRefs } from "@storyteller/tauri-api";
import {
  getCreatorIconPathForModelId,
  getModelDescription,
  getModelInfo,
} from "@storyteller/model-list";
import {
  usePromptAudioStore,
  type RefAudio,
} from "./promptStore";
import { useAutoGrowEditorHeight } from "./useAutoGrowEditorHeight";
import {
  PromptFullscreenModal,
  useFullscreenPrompt,
} from "./PromptFullscreenModal";
import { PromptFullscreenButton } from "./PromptFullscreenButton";
import { PromptClearAllButton } from "./PromptClearAllButton";
import { gtagEvent } from "@storyteller/google-analytics";
import { twMerge } from "tailwind-merge";
import {
  AudioReferenceRow,
  type AudioReferenceRowHandle,
} from "./common/AudioReferenceRow";
import {
  PromptBoxDropOverlay,
  usePromptBoxDrop,
  type DroppedFiles,
} from "./deck/usePromptBoxDrop";
import { AudioTuningPopover } from "./common/AudioTuningPopover";
import { SoundsSettingsPopover } from "./common/SoundsSettingsPopover";
import { StylePromptRow } from "./common/StylePromptRow";

const DEFAULT_AUDIO_MODEL_ID = "suno_music";

const AUDIO_REF_MAX_DURATION_SECONDS = 600;

// Capability flags are serde-skipped when absent — only `true` counts.
const supports = (flag: boolean | null | undefined): boolean => flag === true;

// Resolves 0 when metadata can't be loaded.
const getAudioDurationFromSrc = (src: string): Promise<number> =>
  new Promise((resolve) => {
    const audio = document.createElement("audio");
    audio.preload = "metadata";
    audio.onloadedmetadata = () => resolve(Math.round(audio.duration));
    audio.onerror = () => resolve(0);
    audio.src = src;
  });

interface PromptBoxAudioProps {
  // Audio models from GET /v1/omni_gen/models/audio (useOmniGenAudioModels).
  models: OmniGenAudioModelDetails[];
  uploadAudio?: UploadMediaFn;
  uploadImage?: UploadMediaFn;
  // Fired after a successful enqueue with every created job token (one
  // request can create several Suno clips).
  onEnqueuePressed?: (jobTokens: string[]) => void | Promise<void>;
  credits?: number | null;
  /** Optional account-picker slot rendered at the very start of the toolbar
   *  (left of the model selector). */
  accountSelector?: ReactNode;
  /** Stable id (`credential_{entropy}`) of the selected account credential,
   *  sent with generation requests so the backend knows which stored
   *  credential to generate with. */
  credentialId?: string | null;
  /** Render the focus-mode layout inline, filling the parent's height,
   *  instead of the floating card + fullscreen modal. */
  fullBleed?: boolean;
}

export const PromptBoxAudio = ({
  models,
  uploadAudio,
  uploadImage,
  onEnqueuePressed,
  credits,
  accountSelector,
  credentialId,
  fullBleed = false,
}: PromptBoxAudioProps) => {
  const prompt = usePromptAudioStore((s) => s.prompt);
  const setPrompt = usePromptAudioStore((s) => s.setPrompt);
  const stylePrompt = usePromptAudioStore((s) => s.stylePrompt);
  const setStylePrompt = usePromptAudioStore((s) => s.setStylePrompt);
  const selectedModelId = usePromptAudioStore((s) => s.selectedModelId);
  const setSelectedModelId = usePromptAudioStore((s) => s.setSelectedModelId);
  const isInstrumental = usePromptAudioStore((s) => s.isInstrumental);
  const setIsInstrumental = usePromptAudioStore((s) => s.setIsInstrumental);
  const keepLyrics = usePromptAudioStore((s) => s.keepLyrics);
  const setKeepLyrics = usePromptAudioStore((s) => s.setKeepLyrics);
  const isLoopable = usePromptAudioStore((s) => s.isLoopable);
  const setIsLoopable = usePromptAudioStore((s) => s.setIsLoopable);
  const bpm = usePromptAudioStore((s) => s.bpm);
  const setBpm = usePromptAudioStore((s) => s.setBpm);
  const musicalKey = usePromptAudioStore((s) => s.musicalKey);
  const setMusicalKey = usePromptAudioStore((s) => s.setMusicalKey);
  const sampleRateHz = usePromptAudioStore((s) => s.sampleRateHz);
  const setSampleRateHz = usePromptAudioStore((s) => s.setSampleRateHz);
  const speed = usePromptAudioStore((s) => s.speed);
  const setSpeed = usePromptAudioStore((s) => s.setSpeed);
  const volume = usePromptAudioStore((s) => s.volume);
  const setVolume = usePromptAudioStore((s) => s.setVolume);
  const pitch = usePromptAudioStore((s) => s.pitch);
  const setPitch = usePromptAudioStore((s) => s.setPitch);
  const referenceAudios = usePromptAudioStore((s) => s.referenceAudios);
  const setReferenceAudios = usePromptAudioStore((s) => s.setReferenceAudios);
  const referenceImages = usePromptAudioStore((s) => s.referenceImages);
  const setReferenceImages = usePromptAudioStore((s) => s.setReferenceImages);

  const [isEnqueueing, setIsEnqueueing] = useState(false);
  const [isFocused, setIsFocused] = useState(false);
  const enterToGenerate = useEnterToGenerate();

  // Audio library picker (the "From library" button in the reference row).
  const [isAudioLibraryOpen, setIsAudioLibraryOpen] = useState(false);
  const [audioLibrarySelectedIds, setAudioLibrarySelectedIds] = useState<
    string[]
  >([]);
  const [isAudioLibraryProcessing, setIsAudioLibraryProcessing] =
    useState(false);

  const textareaRef = useRef<HTMLTextAreaElement>(null);
  const { isExpanded, toggleExpand } = useAutoGrowEditorHeight(
    textareaRef,
    prompt,
  );
  const { isFullscreen, openFullscreen, closeFullscreen } =
    useFullscreenPrompt();

  const selectedModel = useMemo((): OmniGenAudioModelDetails | undefined => {
    if (!models.length) return undefined;
    if (selectedModelId) {
      const chosen = models.find((m) => m.model === selectedModelId);
      if (chosen) return chosen;
    }
    return (
      models.find((m) => m.model === DEFAULT_AUDIO_MODEL_ID) ?? models[0]
    );
  }, [models, selectedModelId]);

  // Capability gating
  const styleSupported = supports(selectedModel?.style_prompt_supported);
  const hasInstrumental = supports(
    selectedModel?.instrumental_toggle_supported,
  );
  const hasKeepLyrics = supports(selectedModel?.keep_lyrics_supported);
  const hasLoopable = supports(selectedModel?.loopable_toggle_supported);
  const hasBpm = supports(selectedModel?.bpm_supported);
  const hasMusicalKey = supports(selectedModel?.musical_key_supported);
  const sampleRateOptions = selectedModel?.sample_rate_hz_options ?? null;
  const hasSpeed = supports(selectedModel?.speed_supported);
  const hasVolume = supports(selectedModel?.volume_supported);
  const hasPitch = supports(selectedModel?.pitch_supported);
  const hasTuning =
    !!sampleRateOptions?.length || hasSpeed || hasVolume || hasPitch;
  const audioRefsSupported = supports(
    selectedModel?.audio_references_supported,
  );
  const maxAudioRefs = selectedModel?.audio_references_max ?? 1;
  const imageRefsSupported = supports(
    selectedModel?.image_references_supported,
  );
  const requiresAudioRef = AUDIO_MODELS_REQUIRING_AUDIO_REF.has(
    selectedModel?.model ?? "",
  );
  const missingRequiredAudioRef =
    requiresAudioRef && referenceAudios.length !== 1;

  const effectiveSampleRate =
    sampleRateHz != null && sampleRateOptions?.includes(sampleRateHz)
      ? sampleRateHz
      : (selectedModel?.sample_rate_hz_default ??
        sampleRateOptions?.[0] ??
        null);

  const modelItems = useMemo(
    (): PopoverItem[] =>
      models.map((model) => ({
        label: model.full_name || model.model,
        selected: model.model === selectedModel?.model,
        description: getModelDescription(model.model, model.extra_info_short),
        info: getModelInfo(model.model, model.extra_info) || undefined,
        icon: (
          <img
            src={getCreatorIconPathForModelId(model.model)}
            alt={`${model.model} logo`}
            className="h-4 w-4 icon-auto-contrast"
          />
        ),
        action: model.model,
      })),
    [models, selectedModel?.model],
  );

  // Seed Audio can't combine audio and image references — adding one kind
  // clears the other so the request is always valid.
  const handleReferenceAudiosChange = (audios: typeof referenceAudios) => {
    if (audios.length > 0 && referenceImages.length > 0) {
      setReferenceImages([]);
      toast.error("Removed image reference — it can't be combined with audio");
    }
    setReferenceAudios(audios);
  };

  const maxAudioLibrarySelections = Math.max(
    1,
    maxAudioRefs - referenceAudios.length,
  );

  const handleAudioLibrarySelectToggle = (id: string) => {
    setAudioLibrarySelectedIds((prev) => {
      if (prev.includes(id)) return prev.filter((x) => x !== id);
      if (prev.length >= maxAudioLibrarySelections) {
        return maxAudioLibrarySelections === 1 ? [id] : prev;
      }
      return [...prev, id];
    });
  };

  const closeAudioLibrary = () => {
    setIsAudioLibraryOpen(false);
    setAudioLibrarySelectedIds([]);
  };

  const handleAudioLibraryUseSelected = async (items: GalleryItem[]) => {
    const availableSlots = Math.max(0, maxAudioRefs - referenceAudios.length);
    const picked = items
      .slice(0, availableSlots)
      .filter((item): item is GalleryItem & { fullImage: string } =>
        Boolean(item.fullImage),
      );

    setIsAudioLibraryProcessing(true);
    try {
      // Use the duration the list endpoint already knows; probe the file's
      // metadata only when it doesn't.
      const durations = await Promise.all(
        picked.map((item) =>
          item.durationMillis != null
            ? Promise.resolve(Math.round(item.durationMillis / 1000))
            : getAudioDurationFromSrc(item.fullImage),
        ),
      );

      const added: RefAudio[] = [];
      let total = referenceAudios.reduce((sum, a) => sum + a.duration, 0);
      for (let i = 0; i < picked.length; i++) {
        const item = picked[i]!;
        const duration = durations[i]!;
        if (total + duration > AUDIO_REF_MAX_DURATION_SECONDS) {
          toast.error(
            `Total audio duration cannot exceed ${AUDIO_REF_MAX_DURATION_SECONDS}s`,
          );
          break;
        }
        total += duration;
        added.push({
          id: Math.random().toString(36).substring(7),
          url: item.fullImage,
          file: new File([], "library-audio"),
          mediaToken: item.id,
          duration,
        });
      }
      if (added.length > 0) {
        handleReferenceAudiosChange([...referenceAudios, ...added]);
      }
    } finally {
      setIsAudioLibraryProcessing(false);
    }
    closeAudioLibrary();
  };

  const handleReferenceImagesChange = (images: typeof referenceImages) => {
    if (images.length > 0 && referenceAudios.length > 0) {
      setReferenceAudios([]);
      toast.error("Removed audio reference — it can't be combined with an image");
    }
    setReferenceImages(images);
  };

  // Drag & drop / paste onto the box bounds. Uploads run through the
  // reference row's own plumbing so limits, duration caps, and the
  // audio/image mutual exclusion all behave exactly as they do for the
  // row's own file pickers.
  const referenceRowRef = useRef<AudioReferenceRowHandle>(null);

  const handleDroppedFiles = ({ images, audios }: DroppedFiles) => {
    if (audios.length > 0) {
      void referenceRowRef.current?.addAudioFiles(audios);
    } else if (images.length > 0) {
      // One image ref max, and it can't coexist with audio — so a mixed drop
      // resolves to the audio above and the image is ignored.
      void referenceRowRef.current?.addImageFile(images[0]!);
    }
  };

  const drop = usePromptBoxDrop({
    acceptsImages: imageRefsSupported,
    acceptsVideos: false,
    acceptsAudio: audioRefsSupported,
    onDropFiles: handleDroppedFiles,
  });

  const handleChange = (e: React.ChangeEvent<HTMLTextAreaElement>) => {
    setPrompt(e.target.value);
  };

  const handlePaste = (e: React.ClipboardEvent<HTMLTextAreaElement>) => {
    e.preventDefault();
    const pastedText = e.clipboardData.getData("text");
    const target = e.currentTarget;
    const { selectionStart, selectionEnd, value } = target;
    const next =
      value.slice(0, selectionStart) + pastedText + value.slice(selectionEnd);
    setPrompt(next);
    requestAnimationFrame(() => {
      const pos = Math.min(selectionStart + pastedText.length, next.length);
      textareaRef.current?.setSelectionRange(pos, pos);
    });
  };

  const handleKeyDown = (e: React.KeyboardEvent<HTMLTextAreaElement>) => {
    if (e.key !== "Enter") return;
    if (enterToGenerate && !e.shiftKey) {
      e.preventDefault();
      handleEnqueue();
    }
  };

  const hasAttachedRefs =
    referenceAudios.length > 0 || referenceImages.length > 0;
  const hasClearableContent =
    prompt.length > 0 || stylePrompt.length > 0 || hasAttachedRefs;

  const handleClearAll = () => {
    setPrompt("");
    setStylePrompt("");
    setReferenceAudios([]);
    setReferenceImages([]);
  };

  const handleEnqueue = async () => {
    if (!prompt.trim() || !selectedModel || isEnqueueing) return;

    if (missingRequiredAudioRef) {
      toast.error(
        `${selectedModel.full_name ?? "This model"} needs an audio track to work from — add one first`,
      );
      return;
    }

    setIsEnqueueing(true);
    gtagEvent("enqueue_audio");

    try {
      const settings: AudioGenerationSettings = {
        prompt,
        stylePrompt,
        audioMediaTokens: referenceAudios
          .map((audio) => audio.mediaToken)
          .filter((token) => token.length > 0),
        imageMediaTokens: referenceImages
          .map((image) => image.mediaToken)
          .filter((token) => token.length > 0),
        isInstrumental,
        keepLyrics,
        isLoopable,
        bpm,
        musicalKey,
        sampleRateHz: effectiveSampleRate,
        speed,
        volume,
        pitch,
      };

      // Route through the credential-driven Tauri command (the account
      // selector's credential authenticates the request).
      const body = buildAudioRequest(selectedModel, settings);

      // Three-way sources: library tokens pass through; local files stay on
      // disk (path) or travel as bytes — never uploaded to the cloud here.
      const audioSources =
        selectedModel.audio_references_supported === true &&
        referenceAudios.length > 0
          ? await mediaSourcesFromRefs(referenceAudios)
          : undefined;
      const imageSources =
        selectedModel.image_references_supported === true &&
        referenceImages.length > 0
          ? await mediaSourcesFromRefs(referenceImages)
          : undefined;

      await GenerateAudio({
        credential_id: credentialId ?? undefined,
        frontend_caller: "audio_page",
        model: String(body.model),
        prompt: body.prompt ?? undefined,
        style_prompt: body.style_prompt ?? undefined,
        audio_sources: audioSources,
        image_sources: imageSources,
        keep_lyrics: body.keep_lyrics ?? undefined,
        is_instrumental: body.is_instrumental ?? undefined,
        is_loopable: body.is_loopable ?? undefined,
        bpm: body.bpm ?? undefined,
        musical_key: body.musical_key ? String(body.musical_key) : undefined,
        sample_rate_hz: body.sample_rate_hz ?? undefined,
        speed: body.speed ?? undefined,
        volume: body.volume ?? undefined,
        pitch: body.pitch ?? undefined,
      });

      await onEnqueuePressed?.([]);
    } catch (err) {
      console.error("PromptBoxAudio - enqueue failed", err);
      const backendMessage = (err as { error_message?: string } | null)
        ?.error_message;
      toast.error(
        backendMessage || "Failed to start audio generation. Please try again.",
      );
    } finally {
      setIsEnqueueing(false);
    }
  };

  const modelSelector: ReactNode = (
    <Tooltip content="Model" position="top" className="z-50" closeOnClick>
      <PopoverMenu
        items={modelItems}
        onSelect={(item) => {
          if (item.action) setSelectedModelId(item.action);
        }}
        mode="toggle"
        panelTitle="Select Model"
        panelClassName="w-[360px]"
        richList
        triggerIcon={
          <img
            src={getCreatorIconPathForModelId(selectedModel?.model ?? "")}
            alt=""
            className="h-4 w-4 icon-auto-contrast"
          />
        }
        triggerLabel={selectedModel?.full_name ?? "Model"}
      />
    </Tooltip>
  );

  const toggleButtons: ReactNode = (
    <>
      {hasInstrumental && (
        <Tooltip
          content={isInstrumental ? "Instrumental: ON" : "Instrumental: OFF"}
          position="top"
          className="z-50"
          delay={200}
        >
          <ToggleButton
            isActive={isInstrumental}
            icon={MicOff}
            activeIcon={MicOff}
            label="Instrumental"
            onClick={() => setIsInstrumental(!isInstrumental)}
          />
        </Tooltip>
      )}
      {hasKeepLyrics && (
        <Tooltip
          content={keepLyrics ? "Keep lyrics: ON" : "Keep lyrics: OFF"}
          position="top"
          className="z-50"
          delay={200}
        >
          <ToggleButton
            isActive={keepLyrics}
            icon={Mic}
            activeIcon={Mic}
            label="Keep lyrics"
            onClick={() => setKeepLyrics(!keepLyrics)}
          />
        </Tooltip>
      )}
      {hasLoopable && (
        <Tooltip
          content={isLoopable ? "Loop: ON" : "Loop: OFF"}
          position="top"
          className="z-50"
          delay={200}
        >
          <ToggleButton
            isActive={isLoopable}
            icon={Repeat}
            activeIcon={Repeat}
            label="Loop"
            onClick={() => setIsLoopable(!isLoopable)}
          />
        </Tooltip>
      )}
    </>
  );

  const settingsPopovers: ReactNode = (
    <>
      {(hasBpm || hasMusicalKey) && (
        <SoundsSettingsPopover
          showBpm={hasBpm}
          bpm={bpm}
          onBpmChange={setBpm}
          showMusicalKey={hasMusicalKey}
          musicalKey={musicalKey}
          onMusicalKeyChange={setMusicalKey}
        />
      )}
      {hasTuning && (
        <AudioTuningPopover
          sampleRateOptions={sampleRateOptions}
          sampleRateHz={effectiveSampleRate}
          onSampleRateChange={setSampleRateHz}
          showSpeed={hasSpeed}
          speed={speed}
          onSpeedChange={setSpeed}
          showVolume={hasVolume}
          volume={volume}
          onVolumeChange={setVolume}
          showPitch={hasPitch}
          pitch={pitch}
          onPitchChange={setPitch}
        />
      )}
    </>
  );

  const referenceRow: ReactNode =
    audioRefsSupported || imageRefsSupported ? (
      <AudioReferenceRow
        ref={referenceRowRef}
        referenceAudios={referenceAudios}
        onReferenceAudiosChange={handleReferenceAudiosChange}
        maxAudioCount={audioRefsSupported ? maxAudioRefs : 0}
        maxAudioRefDuration={AUDIO_REF_MAX_DURATION_SECONDS}
        uploadAudio={uploadAudio}
        onPickAudioFromLibrary={undefined}
        audioRequired={requiresAudioRef}
        imageSupported={imageRefsSupported}
        referenceImages={referenceImages}
        onReferenceImagesChange={handleReferenceImagesChange}
        uploadImage={uploadImage}
      />
    ) : null;

  // The audio library picker is an input picker (select mode), shared by both
  // layouts.
  const audioLibraryModal = (
    <GalleryModal
      mode="select"
      isOpen={isAudioLibraryOpen}
      onClose={closeAudioLibrary}
      selectedItemIds={audioLibrarySelectedIds}
      onSelectItem={handleAudioLibrarySelectToggle}
      maxSelections={maxAudioLibrarySelections}
      onUseSelected={handleAudioLibraryUseSelected}
      useSelectedLoading={isAudioLibraryProcessing}
      forceFilter="audio"
      hideFilter
    />
  );

  if (fullBleed) {
    return (
      <>
        <div
          className="relative flex h-full min-h-0 flex-col"
          {...drop.dropZoneProps}
        >
          <PromptBoxDropOverlay
            dragState={drop.dragState}
            acceptsImages={imageRefsSupported}
            acceptsVideos={false}
            acceptsAudio={audioRefsSupported}
          />
          <div className="flex min-h-0 flex-1 flex-col overflow-hidden">
            <textarea
              autoFocus
              placeholder="Describe the music or sound you want..."
              className="promptbox-scrollbar h-full min-h-0 w-full flex-1 resize-none overflow-y-auto bg-transparent text-[15px] leading-[1.85] text-bone/95 placeholder-mud focus:outline-none sm:text-[16.5px]"
              value={prompt}
              onChange={handleChange}
              onPaste={handlePaste}
              onKeyDown={handleKeyDown}
            />
          </div>
          {styleSupported && (
            <div className="shrink-0">
              <StylePromptRow value={stylePrompt} onChange={setStylePrompt} />
            </div>
          )}
          <div className="shrink-0 pt-2">{referenceRow}</div>
          <div className="mt-3 flex shrink-0 flex-wrap items-center justify-between gap-2">
            <div className="flex flex-wrap items-center gap-2">
              {accountSelector}
              {modelSelector}
              {toggleButtons}
              {settingsPopovers}
            </div>
            <div className="flex items-center gap-2">
              {missingRequiredAudioRef && (
                <span className="flex animate-pulse items-center gap-1.5 text-xs font-medium text-red-500">
                  Audio track required
                </span>
              )}
              <PromptClearAllButton
                onClick={handleClearAll}
                disabled={!hasClearableContent}
                confirmClear={hasAttachedRefs}
              />
              <GenerateIconButton
                onClick={handleEnqueue}
                disabled={!prompt.trim() || missingRequiredAudioRef}
                loading={isEnqueueing}
                credits={credits}
              />
            </div>
          </div>
        </div>
        {audioLibraryModal}
      </>
    );
  }

  return (
    <>
      <div className="relative z-20 flex flex-col">
        <div
          className={twMerge(
            "glass relative w-full rounded-2xl p-4",
            isFocused
              ? "ring-1 ring-primary border-primary"
              : "ring-1 ring-transparent",
          )}
          {...drop.dropZoneProps}
        >
          <PromptBoxDropOverlay
            dragState={drop.dragState}
            acceptsImages={imageRefsSupported}
            acceptsVideos={false}
            acceptsAudio={audioRefsSupported}
          />
          {referenceRow}

          <div className="flex justify-center gap-2">
            <div className="promptbox-resize-wrap relative flex-1">
              <textarea
                ref={textareaRef}
                rows={1}
                placeholder="Describe the music or sound you want..."
                className="promptbox-scrollbar text-md mb-2 min-h-[2.5em] w-full resize-y overflow-y-auto rounded bg-transparent pb-2 pr-8 pt-1 text-base-fg placeholder-base-fg/60 transition-[height] duration-200 ease-out focus:outline-none"
                value={prompt}
                onChange={handleChange}
                onPaste={handlePaste}
                onKeyDown={handleKeyDown}
                onFocus={() => setIsFocused(true)}
                onBlur={() => setIsFocused(false)}
              />
              <PromptFullscreenButton onClick={openFullscreen} />
            </div>
          </div>

          {styleSupported && (
            <StylePromptRow value={stylePrompt} onChange={setStylePrompt} />
          )}

          <div className="mt-2 flex items-center justify-between gap-2">
            <div className="flex flex-wrap items-center gap-2">
              {accountSelector}
              {modelSelector}
              {toggleButtons}
              {settingsPopovers}
            </div>
            <div className="flex items-center gap-2">
              {missingRequiredAudioRef && (
                <span className="flex animate-pulse items-center gap-1.5 text-xs font-medium text-red-500">
                  Audio track required
                </span>
              )}
              <PromptClearAllButton
                onClick={handleClearAll}
                disabled={!hasClearableContent}
                confirmClear={hasAttachedRefs}
              />
              <GenerateIconButton
                onClick={handleEnqueue}
                disabled={!prompt.trim() || missingRequiredAudioRef}
                loading={isEnqueueing}
                credits={credits}
              />
            </div>
          </div>

          <div className="absolute -bottom-1 left-1/2 -translate-x-1/2">
            <Tooltip
              content={isExpanded ? "Collapse" : "Expand"}
              position="top"
              className="-mb-2"
            >
              <button
                type="button"
                onClick={toggleExpand}
                className="text-base-fg/30 hover:text-base-fg/90 transition-colors px-3 py-0.5"
              >
                {isExpanded ? (
                  <ChevronUp size="1em" className="text-xs" />
                ) : (
                  <ChevronDown size="1em" className="text-xs" />
                )}
              </button>
            </Tooltip>
          </div>
        </div>
      </div>
      {audioLibraryModal}
      <PromptFullscreenModal
        isOpen={isFullscreen}
        onClose={closeFullscreen}
        promptLength={prompt.length}
        maxLength={Infinity}
        clearAllButton={
          <PromptClearAllButton
            onClick={handleClearAll}
            disabled={!hasClearableContent}
            confirmClear={hasAttachedRefs}
          />
        }
        footerControls={
          <>
            {modelSelector}
            {toggleButtons}
            {settingsPopovers}
          </>
        }
      >
        <textarea
          placeholder="Describe the music or sound you want..."
          className="promptbox-scrollbar text-md h-full min-h-0 w-full resize-none overflow-y-auto rounded bg-transparent text-base-fg placeholder-base-fg/60 focus:outline-none"
          value={prompt}
          onChange={handleChange}
          onPaste={handlePaste}
          onKeyDown={handleKeyDown}
        />
      </PromptFullscreenModal>
    </>
  );
};
