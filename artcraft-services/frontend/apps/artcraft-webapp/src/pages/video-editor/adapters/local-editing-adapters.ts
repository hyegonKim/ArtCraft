import { createDefaultAdapters, deserializeProjectDocument, serializeProjectDocument, kindFromMime, type MediaSourceAdapter, type ResolvedMedia } from '@storyteller/ui-video-editor';
import type { WebappProjectStorage } from './project-storage-adapter';

// Explicitly opted-in local editing: media never leaves this browser. Keep
// the Files as well as the project document so reload can rehydrate clips.
function fileRequest<T>(mode: IDBTransactionMode, operation: (store: IDBObjectStore) => IDBRequest<T>): Promise<T> {
  return new Promise((resolve, reject) => {
    const open = indexedDB.open('artcraft-local-video-media', 1);
    open.onupgradeneeded = () => open.result.createObjectStore('files');
    open.onerror = () => reject(open.error);
    open.onsuccess = () => {
      const db = open.result;
      const transaction = db.transaction('files', mode);
      const request = operation(transaction.objectStore('files'));
      transaction.oncomplete = () => { db.close(); resolve(request.result); };
      transaction.onerror = () => { db.close(); reject(transaction.error); };
      transaction.onabort = () => { db.close(); reject(transaction.error); };
    };
  });
}

export function createLocalEditingAdapters() {
  const defaults = createDefaultAdapters();
  const resolved = new Map<string, ResolvedMedia>();
  const projectStorage: WebappProjectStorage = {
    ...defaults.projectStorage,
    async renameProject(id, name) {
      const row = await defaults.projectStorage.loadProject(id);
      if (!row) throw new Error('Project not found');
      const document = deserializeProjectDocument(row.data);
      if (document) document.project.metadata.name = name;
      await defaults.projectStorage.saveProject({
        ...row, name, updatedAt: Date.now(),
        data: document ? serializeProjectDocument(document) : row.data,
      });
    },
  };
  const mediaSource: MediaSourceAdapter = {
    async uploadLocalFile(file) {
      const id = `local-media-${crypto.randomUUID()}`;
      await fileRequest('readwrite', (store) => store.put(file, id));
      return { id, kind: kindFromMime(file.type) };
    },
    async resolveMedia(handle) {
      const cached = resolved.get(handle.id);
      if (cached) return cached;
      const file = await fileRequest<File | undefined>('readonly', (store) => store.get(handle.id));
      if (!file) throw new Error('This local media file is unavailable in this browser.');
      const media = { url: URL.createObjectURL(file), mime: file.type || 'application/octet-stream' };
      resolved.set(handle.id, media);
      return media;
    },
    releaseResolved(media) {
      for (const [id, entry] of resolved) {
        if (entry === media) { resolved.delete(id); URL.revokeObjectURL(entry.url); }
      }
    },
    async deleteHandle(handle) {
      const media = resolved.get(handle.id);
      if (media) mediaSource.releaseResolved?.(media);
      await fileRequest('readwrite', (store) => store.delete(handle.id));
    },
  };
  return { projectStorage, mediaSource, exportSink: defaults.exportSink };
}
