import type { LayerProject } from '@storyteller/ui-pagedraw';

/** A device-local copy of a 2D document whose latest edits may not be on the server yet. */
export interface LayerDraft {
  id: string;
  /** The signed-in user's token. Drafts are never shown to another account. */
  owner: string;
  token?: string;
  /** A Save copy that has not reached the server yet must still create a separate project. */
  copyPending: boolean;
  name: string;
  document: LayerProject;
  /** SHA-256 of the last document the server confirmed for `token`. */
  baseHash?: string;
  /** Writes this device sent for `token` whose responses may have been lost. */
  ownHashes: string[];
  updatedAt: number;
}

export interface LayerDraftStore {
  /** False when the browser refuses persistent storage; drafts then last only for this page. */
  durable(): Promise<boolean>;
  put(draft: LayerDraft): Promise<void>;
  get(id: string): Promise<LayerDraft | undefined>;
  /** Newest first. */
  list(owner: string): Promise<LayerDraft[]>;
  remove(id: string): Promise<void>;
  /** Holds a cross-tab lock until released. Resolves undefined while another tab holds it. */
  claim(id: string): Promise<(() => void) | undefined>;
  /** Draft ids currently held by any tab, including this one. */
  held(): Promise<Set<string>>;
}

const DB_NAME = 'artcraft-layer-drafts';
const STORE = 'drafts';
const LOCK_PREFIX = 'artcraft-layer-draft:';
const newestFirst = (a: LayerDraft, b: LayerDraft) => b.updatedAt - a.updatedAt;

export function createMemoryDraftStore(durable = false): LayerDraftStore {
  const rows = new Map<string, LayerDraft>();
  const locks = new Set<string>();
  return {
    durable: async () => durable,
    put: async draft => { rows.set(draft.id, structuredClone(draft)); },
    get: async id => { const row = rows.get(id); return row && structuredClone(row); },
    list: async owner => [...rows.values()].filter(row => row.owner === owner).map(row => structuredClone(row)).sort(newestFirst),
    remove: async id => { rows.delete(id); },
    claim: async id => {
      if (locks.has(id)) return undefined;
      locks.add(id);
      return () => { locks.delete(id); };
    },
    held: async () => new Set(locks),
  };
}

function settle<T>(request: IDBRequest<T>): Promise<T> {
  return new Promise((resolve, reject) => {
    request.onsuccess = () => resolve(request.result);
    request.onerror = () => reject(request.error ?? new Error('Local storage request failed.'));
  });
}

function openDatabase(factory: IDBFactory | undefined): Promise<IDBDatabase | undefined> {
  return new Promise(resolve => {
    if (!factory) { resolve(undefined); return; }
    try {
      const request = factory.open(DB_NAME, 1);
      request.onupgradeneeded = () => {
        request.result.createObjectStore(STORE, { keyPath: 'id' }).createIndex('owner', 'owner');
      };
      request.onsuccess = () => {
        const database = request.result;
        database.onversionchange = () => database.close();
        resolve(database);
      };
      request.onerror = () => resolve(undefined);
    } catch {
      // Some private modes throw synchronously instead of failing the request.
      resolve(undefined);
    }
  });
}

/** IndexedDB-backed drafts. Falls back to page memory when the browser blocks storage. */
export function openLayerDraftStore(factory: IDBFactory | undefined = globalThis.indexedDB): LayerDraftStore {
  const fallback = createMemoryDraftStore();
  const database = openDatabase(factory);
  async function run<T>(mode: IDBTransactionMode, operation: (store: IDBObjectStore) => IDBRequest<T>): Promise<T> {
    const db = await database;
    if (!db) throw new Error('Local storage is unavailable.');
    const transaction = db.transaction(STORE, mode);
    const result = settle(operation(transaction.objectStore(STORE)));
    await new Promise<void>((resolve, reject) => {
      transaction.oncomplete = () => resolve();
      transaction.onerror = () => reject(transaction.error ?? new Error('Local storage request failed.'));
      transaction.onabort = () => reject(transaction.error ?? new Error('Local storage request was aborted.'));
    });
    return result;
  }
  const locks = (globalThis.navigator as Navigator | undefined)?.locks;
  return {
    durable: async () => !!(await database),
    async put(draft) { if (await database) await run('readwrite', store => store.put(draft)); else await fallback.put(draft); },
    async get(id) { return (await database) ? run('readonly', store => store.get(id) as IDBRequest<LayerDraft | undefined>) : fallback.get(id); },
    async list(owner) {
      if (!(await database)) return fallback.list(owner);
      const rows = await run('readonly', store => store.index('owner').getAll(owner) as IDBRequest<LayerDraft[]>);
      return rows.sort(newestFirst);
    },
    async remove(id) { if (await database) await run('readwrite', store => store.delete(id)); else await fallback.remove(id); },
    claim(id) {
      if (!locks?.request) return fallback.claim(id);
      return new Promise(resolve => {
        locks.request(LOCK_PREFIX + id, { ifAvailable: true }, lock => {
          if (!lock) { resolve(undefined); return undefined; }
          return new Promise<void>(release => resolve(() => release()));
        }).catch(() => resolve(() => {})); // A broken lock API must not block recovery in a single tab.
      });
    },
    async held() {
      if (!locks?.query) return fallback.held();
      const snapshot = await locks.query();
      return new Set((snapshot.held ?? []).map(lock => lock.name ?? '')
        .filter(name => name.startsWith(LOCK_PREFIX)).map(name => name.slice(LOCK_PREFIX.length)));
    },
  };
}

/** Content identity of a document, stable across a JSON round trip through the server. */
export async function hashLayerDocument(document: LayerProject): Promise<string> {
  const digest = await crypto.subtle.digest('SHA-256', new TextEncoder().encode(JSON.stringify(document)));
  return Array.from(new Uint8Array(digest), byte => byte.toString(16).padStart(2, '0')).join('');
}
