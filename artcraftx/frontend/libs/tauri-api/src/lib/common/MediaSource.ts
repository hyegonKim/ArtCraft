/**
 * Three-way media source for generation requests, mirroring the Rust
 * `TauriMediaSource` enum. Local files and raw bytes never touch the
 * ArtCraft cloud unless the target provider itself requires it.
 *
 * Wire format (don't change without coordinating with the backend):
 * `{"kind": "media_file_token", "token": "m_..."}`,
 * `{"kind": "local_path", "path": "/Users/..."}`, or
 * `{"kind": "bytes", "bytes": [...], "file_name": "photo.png"}`.
 */
export type MediaSource =
  | { kind: "media_file_token"; token: string }
  | { kind: "local_path"; path: string }
  | { kind: "bytes"; bytes: number[]; file_name?: string };

/** The shape every prompt-box reference carries (see promptStore RefImage). */
export interface MediaSourceRefLike {
  mediaToken: string;
  localPath?: string;
  file: File;
}

/**
 * Build the request-side source for one reference: a non-empty media token
 * wins (library pick), then a local path, then the file's raw bytes
 * (paste/browser files that never touched disk).
 */
export const mediaSourceFromRef = async (
  ref: MediaSourceRefLike,
): Promise<MediaSource> => {
  if (ref.mediaToken.length > 0) {
    return { kind: "media_file_token", token: ref.mediaToken };
  }
  if (ref.localPath) {
    return { kind: "local_path", path: ref.localPath };
  }
  const bytes = Array.from(new Uint8Array(await ref.file.arrayBuffer()));
  return {
    kind: "bytes",
    bytes,
    file_name: ref.file.name.length > 0 ? ref.file.name : undefined,
  };
};

/** List twin of {@link mediaSourceFromRef}; order is preserved. */
export const mediaSourcesFromRefs = (
  refs: MediaSourceRefLike[],
): Promise<MediaSource[]> => Promise.all(refs.map(mediaSourceFromRef));
