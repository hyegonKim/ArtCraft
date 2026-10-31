import { useMemo, useRef, useState } from "react";
import { Box, Earth, Plus, X } from "lucide-react";
import { toast } from "react-hot-toast";
import { twMerge } from "tailwind-merge";
import { GenerateIconButton } from "@storyteller/ui-button";
import { useCostBreakdownModalStore } from "@storyteller/ui-pricing-modal";
import {
  GenerateMesh,
  GenerateSplat,
  useSplatModels,
  type MediaSource,
} from "@storyteller/tauri-api";
import {
  ClassyModelSelector,
  ModelPage,
  useImageTo3dObjectPageModelList,
  useImageTo3dWorldPageModelList,
  useSelectedModel,
} from "@storyteller/ui-model-selector";
import { PromptShell, useComposerTasks } from "~/components/PromptShell";
import { AccountSelector } from "~/components/account-selector/AccountSelector";
import { useSelectedAccountId } from "@storyteller/ui-model-selector";

// Minimal image→3D composer in the marketing-site style: reference tray +
// optional prompt + generate. Replaces the old ImageTo3DExperience page UI
// (viewer/history/gallery) — results are written to disk like every other
// modality. The old experience component is kept in the tree, just unused.

type Variant = "object" | "world";

interface TrayImage {
  id: string;
  preview: string;
  /** Set only for library picks; locally attached files keep their bytes in
   *  `file` and travel with the generate request instead. */
  mediaToken: string | null;
  file: File | null;
  name: string;
  isUploading: boolean;
}

const MAX_WORLD_IMAGES = 10;
const DEFAULT_OBJECT_MODEL_ID = "hunyuan_3d_3";

const generateId = () =>
  typeof crypto !== "undefined" && crypto.randomUUID
    ? crypto.randomUUID()
    : Math.random().toString(36).slice(2, 10);

interface ImageTo3DComposerProps {
  variant: Variant;
}

export const ImageTo3DComposer = ({ variant }: ImageTo3DComposerProps) => {
  const isWorld = variant === "world";
  const pageId = isWorld ? ModelPage.ImageTo3DWorld : ModelPage.ImageTo3DObject;
  const maxImages = isWorld ? MAX_WORLD_IMAGES : 1;

  const selectedModel = useSelectedModel(pageId);
  const selectedObjectModelId = selectedModel?.id ?? DEFAULT_OBJECT_MODEL_ID;
  const worldModelList = useImageTo3dWorldPageModelList();
  const objectModelList = useImageTo3dObjectPageModelList();
  // Fallback when nothing is selected yet (lists load from the backend).
  const defaultSplatModel = useSplatModels()[0];

  const { busy, completed } = useComposerTasks(isWorld ? "splat" : "mesh");

  const estimatedCredits = useCostBreakdownModalStore(
    (state) => state.estimatedCreditsByPage[pageId],
  );

  const [images, setImages] = useState<TrayImage[]>([]);
  const [prompt, setPrompt] = useState("");
  const selectedAccountId = useSelectedAccountId();
  const [isGenerating, setIsGenerating] = useState(false);
  const fileInputRef = useRef<HTMLInputElement>(null);

  const readyImages = useMemo(
    () =>
      images.filter((img) => (img.mediaToken || img.file) && !img.isUploading),
    [images],
  );
  const canGenerate =
    !isGenerating && readyImages.length > 0 && !images.some((i) => i.isUploading);

  const handleFiles = async (files?: FileList | File[] | null) => {
    if (!files) return;
    const remaining = maxImages - images.length;
    const imageFiles = Array.from(files)
      .filter((f) => f.type.startsWith("image/"))
      .slice(0, Math.max(0, remaining));
    if (imageFiles.length === 0) return;

    // No upload at attach time: keep the bytes local; they travel with the
    // generate request (the backend uploads to ArtCraft only then, since
    // mesh/splat generation runs on the ArtCraft provider).
    await Promise.all(
      imageFiles.map(async (file) => {
        const imageId = generateId();
        const preview = await new Promise<string>((resolve) => {
          const reader = new FileReader();
          reader.onload = (e) => resolve(e.target?.result as string);
          reader.readAsDataURL(file);
        });

        setImages((prev) => [
          ...prev,
          {
            id: imageId,
            preview,
            mediaToken: null,
            file,
            name: file.name,
            isUploading: false,
          },
        ]);
      }),
    );
  };

  const handleGenerate = async () => {
    if (!canGenerate) return;
    setIsGenerating(true);
    const subscriberId = generateId();

    try {
      window.__storeTaskEnqueueMeta?.({
        prompt: prompt.trim() || undefined,
        refImageUrls: images.map((img) => img.preview).filter(Boolean),
        modelType: isWorld
          ? (selectedModel as any)?.tauriId ||
            defaultSplatModel?.tauriId ||
            String(selectedModel ?? defaultSplatModel)
          : (selectedModel as any)?.tauriId || selectedObjectModelId,
        timestamp: Date.now(),
      });

      // Three-way sources: library tokens pass through; locally attached
      // files travel as bytes.
      const referenceImageSources: MediaSource[] = await Promise.all(
        readyImages.map(async (img): Promise<MediaSource> => {
          if (img.mediaToken) {
            return { kind: "media_file_token", token: img.mediaToken };
          }
          const bytes = Array.from(
            new Uint8Array(await img.file!.arrayBuffer()),
          );
          return { kind: "bytes", bytes, file_name: img.name || undefined };
        }),
      );

      const result = isWorld
        ? await GenerateSplat({
            credential_id: selectedAccountId ?? undefined,
            model:
              (selectedModel as any)?.tauriId ??
              defaultSplatModel?.tauriId,
            prompt: prompt.trim() || undefined,
            reference_image_sources: referenceImageSources,
            frontend_caller: "splat_page",
            frontend_subscriber_id: subscriberId,
          })
        : await GenerateMesh({
            credential_id: selectedAccountId ?? undefined,
            model: selectedObjectModelId,
            prompt: prompt.trim() || undefined,
            reference_image_sources: referenceImageSources,
            frontend_caller: "mesh_page",
            frontend_subscriber_id: subscriberId,
          });

      if ("error_type" in result) {
        throw new Error(
          (result as any).error_message || (result as any).error_type,
        );
      }
      window.dispatchEvent(new Event("task-queue-update"));
    } catch (error) {
      const errorMessage =
        error instanceof Error
          ? error.message
          : ((error as { error_message?: string } | null)?.error_message ??
            "An unexpected error occurred");
      toast.error(`Failed to generate 3D model: ${errorMessage}`);
    } finally {
      setIsGenerating(false);
    }
  };

  return (
    <PromptShell
      icon={
        isWorld ? (
          <Earth className="h-[17px] w-[17px]" />
        ) : (
          <Box className="h-[17px] w-[17px]" />
        )
      }
      busy={busy}
      completed={completed}
    >
      <div
        className="flex h-full min-h-0 flex-col"
        onDragOver={(e) => e.preventDefault()}
        onDrop={(e) => {
          e.preventDefault();
          handleFiles(e.dataTransfer?.files);
        }}
      >
        {isWorld ? (
          <textarea
            placeholder="Optionally describe the world to build from your images..."
            className="promptbox-scrollbar min-h-0 w-full flex-1 resize-none overflow-y-auto bg-transparent text-[15px] leading-[1.85] text-bone/95 placeholder-mud focus:outline-none sm:text-[16.5px]"
            value={prompt}
            onChange={(e) => setPrompt(e.target.value)}
          />
        ) : (
          <div className="flex min-h-0 flex-1 items-start pt-1">
            <p className="text-[15px] leading-[1.85] text-mud sm:text-[16.5px]">
              Add a reference image below — the mesh is generated from the
              image alone.
            </p>
          </div>
        )}

        {/* references tray — same 72px deck cards as the image/video pages */}
        <div className="shrink-0">
          <div className="flex items-end gap-2">
            {images.map((img) => (
              <span
                key={img.id}
                className="group relative aspect-square w-[72px] shrink-0 overflow-hidden rounded-lg border-2 border-white/30"
              >
                <img
                  src={img.preview}
                  alt={img.name}
                  className="h-full w-full object-cover"
                />
                <button
                  type="button"
                  aria-label="Remove reference"
                  className="absolute right-[2px] top-[2px] flex h-5 w-5 cursor-pointer items-center justify-center rounded-full bg-black/50 text-white opacity-0 backdrop-blur-md transition-colors hover:bg-red/70 group-hover:opacity-100"
                  onClick={() =>
                    setImages((prev) => prev.filter((i) => i.id !== img.id))
                  }
                >
                  <X className="h-2.5 w-2.5" />
                </button>
              </span>
            ))}
            {images.length < maxImages && (
              <button
                type="button"
                aria-label="Select reference image file"
                className="flex aspect-square w-[72px] shrink-0 flex-col items-center justify-center gap-0.5 rounded-lg border-2 border-dashed border-line-2 bg-bone/[0.03] text-putty transition-colors duration-300 ease-out hover:border-bone/30 hover:bg-bone/[0.08] hover:text-bone"
                onClick={() => fileInputRef.current?.click()}
              >
                <Plus className="h-[18px] w-[18px] opacity-80" />
                <span className="px-0.5 text-center text-[10px] font-medium leading-tight opacity-70">
                  Select file
                </span>
              </button>
            )}
            {isWorld && images.length > 0 && (
              <button
                type="button"
                className="pb-1 text-[12px] text-mud transition-colors hover:text-putty"
                onClick={() => setImages([])}
              >
                Clear all
              </button>
            )}
          </div>
          <input
            ref={fileInputRef}
            type="file"
            accept="image/*"
            multiple={isWorld}
            className="hidden"
            onChange={(e) => {
              handleFiles(e.target.files);
              e.target.value = "";
            }}
          />
        </div>

        {/* control bar */}
        <div className="mt-3 flex shrink-0 flex-wrap items-center justify-between gap-2">
          <div className="flex flex-wrap items-center gap-2">
            <AccountSelector />
            <ClassyModelSelector
              variant="embedded"
              items={isWorld ? worldModelList : objectModelList}
              page={pageId}
            />
          </div>
          <GenerateIconButton
            onClick={handleGenerate}
            disabled={!canGenerate}
            loading={isGenerating}
            credits={estimatedCredits}
          />
        </div>
      </div>
    </PromptShell>
  );
};
