import { useCallback, useEffect, useRef, useState } from "react";
import { listen, UnlistenFn } from "@tauri-apps/api/event";
import { GetTaskQueue } from "@storyteller/tauri-api";
import { MediaFilesApi } from "@storyteller/api";

// Lean activity tracker for the full-bleed composers: "is anything in flight
// for this modality" plus newly-completed file URLs. The old generation feed
// expanded results into gallery items; here completions only drive the disk
// receipt, so this stays thumbnail- and gallery-free.

const POLL_INTERVAL_MS = 5000;

export type ComposerTaskModality = "image" | "video" | "splat" | "mesh" | "audio";

export interface CompletedFile {
  /** Unique per file (task id, or task id + batch index). */
  id: string;
  url: string;
  /** Generation provider ("artcraft", "fal", "higgsfield", ...). Every
   *  provider's files are auto-saved by the Rust polling threads; the
   *  frontend only receipts them. */
  provider?: string;
}

const TASK_TYPES: Record<ComposerTaskModality, ReadonlySet<string>> = {
  image: new Set(["image_generation"]),
  video: new Set(["video_generation"]),
  splat: new Set(["splat_generation"]),
  mesh: new Set(["mesh_generation"]),
  audio: new Set(["audio_generation"]),
};

export function useComposerTasks(modality: ComposerTaskModality) {
  const [busy, setBusy] = useState(false);
  const [completed, setCompleted] = useState<CompletedFile[]>([]);
  const mediaApiRef = useRef(new MediaFilesApi());
  // null until the first load — pre-existing completions must not re-emit
  // (and re-save) every time the page remounts on a tab switch.
  const prevCompletedIdsRef = useRef<Set<string> | null>(null);

  const load = useCallback(async () => {
    try {
      const { tasks } = await GetTaskQueue();
      const mine = tasks.filter((t) =>
        TASK_TYPES[modality].has(String(t.task_type)),
      );

      setBusy(
        mine.some(
          (t) => t.task_status === "pending" || t.task_status === "started",
        ),
      );

      const done = mine.filter(
        (t) =>
          String(t.task_status) === "complete_success" &&
          t.completed_item?.primary_media_file?.cdn_url,
      );
      const doneIds = new Set(done.map((t) => t.id));

      if (prevCompletedIdsRef.current) {
        const prev = prevCompletedIdsRef.current;
        const fresh = done.filter((t) => !prev.has(t.id));
        if (fresh.length > 0) {
          const expanded = await Promise.all(
            fresh.map((t) => expandTaskFiles(t, mediaApiRef.current)),
          );
          const files = expanded.flat();
          if (files.length > 0) {
            setCompleted((prevFiles) => {
              const existing = new Set(prevFiles.map((f) => f.id));
              return [
                ...prevFiles,
                ...files.filter((f) => !existing.has(f.id)),
              ];
            });
          }
        }
      }
      prevCompletedIdsRef.current = doneIds;
    } catch {
      // ignore — next poll retries
    }
  }, [modality]);

  useEffect(() => {
    load();
    const intervalId = setInterval(load, POLL_INTERVAL_MS);

    const handleTaskUpdate = () => load();
    window.addEventListener("task-queue-update", handleTaskUpdate);

    let cancelled = false;
    const unlistens: Promise<UnlistenFn>[] = [
      "generation-complete-event",
      "generation-failed-event",
    ].map((name) =>
      listen(name, () => {
        if (!cancelled) load();
      }),
    );

    return () => {
      cancelled = true;
      clearInterval(intervalId);
      window.removeEventListener("task-queue-update", handleTaskUpdate);
      unlistens.forEach((p) => p.then((f) => f()));
    };
  }, [load]);

  return { busy, completed };
}

/** One image task can produce a whole batch — expand to every file so all of
 *  them land on disk, falling back to the primary file alone. */
async function expandTaskFiles(
  t: Awaited<ReturnType<typeof GetTaskQueue>>["tasks"][number],
  mediaFilesApi: MediaFilesApi,
): Promise<CompletedFile[]> {
  const provider = t.provider ? String(t.provider) : undefined;
  const primary: CompletedFile = {
    id: t.id,
    // Callers filter on cdn_url presence before expanding (line ~57).
    url: t.completed_item!.primary_media_file!.cdn_url,
    provider,
  };

  const batchToken = t.completed_item?.maybe_batch_token;
  if (!batchToken) return [primary];

  try {
    const batchResponse = await mediaFilesApi.GetMediaFilesByBatchToken({
      batchToken,
    });
    if (!batchResponse.success || !batchResponse.data?.length) {
      return [primary];
    }
    const files = batchResponse.data
      .map((file: any, index: number): CompletedFile | null => {
        const cdnUrl = file.media_links?.cdn_url;
        if (!cdnUrl) return null;
        return { id: `${t.id}:${file.token ?? index}`, url: cdnUrl, provider };
      })
      .filter((f: CompletedFile | null): f is CompletedFile => f !== null);
    return files.length > 0 ? files : [primary];
  } catch {
    return [primary];
  }
}
