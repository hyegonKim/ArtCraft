import { invoke } from "@tauri-apps/api/core";
import { CommandResult } from "../common/CommandStatus";

/**
 * Bulk thumbnail lookup for local files. The backend serves what its
 * thumbnail worker has already produced (`cache/thumbnails/{hash}.jpg|.webp`)
 * and queues anything `pending` for the worker; `local_thumbnail_ready_event`
 * announces those as they land. Render the returned paths via
 * `convertFileSrc(...)`.
 */

export type LocalThumbnailStatus =
  | "ready"
  | "pending"
  | "unsupported"
  | "missing_file"
  | "failed";

export interface LocalThumbnail {
  file_path: string;
  status: LocalThumbnailStatus;
  maybe_file_hash?: string;
  /** Absolute path of the static JPEG thumbnail (512px longest edge). */
  maybe_thumbnail_path?: string;
  /** Absolute path of the ~1s animated WebP preview (videos only). */
  maybe_animated_preview_path?: string;
}

interface GetLocalThumbnailsResponse {
  thumbnails: LocalThumbnail[];
}

interface GetLocalThumbnailsSuccess extends CommandResult {
  payload: GetLocalThumbnailsResponse;
}

export const GetLocalThumbnails = async (
  filePaths: string[],
): Promise<LocalThumbnail[]> => {
  if (filePaths.length === 0) return [];
  const result = (await invoke("get_local_thumbnails_command", {
    request: { file_paths: filePaths },
  })) as GetLocalThumbnailsSuccess;
  return result?.payload?.thumbnails ?? [];
};
