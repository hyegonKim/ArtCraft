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

/** A draft's details without its document, which can be many megabytes of embedded images. */
export type LayerDraftSummary = Omit<LayerDraft, 'document'>;

export interface LayerDraftStore {
  /** False when the browser refuses persistent storage; drafts then last only for this page. */
  durable(): Promise<boolean>;
  put(draft: LayerDraft): Promise<void>;
  get(id: string): Promise<LayerDraft | undefined>;
  /** The owner's drafts without their documents, newest first: cheap enough to read on every focus. */
  list(owner: string): Promise<LayerDraftSummary[]>;
  /** Changes a draft's details, never its document; does nothing when the draft is gone. */
  update(id: string, details: Partial<Omit<LayerDraftSummary, 'id' | 'owner'>>): Promise<void>;
  remove(id: string): Promise<void>;
  /** Holds a cross-tab lock until released. Resolves undefined while another tab holds it. */
  claim(id: string): Promise<(() => void) | undefined>;
  /** Draft ids currently held by any tab, including this one. */
  held(): Promise<Set<string>>;
}

const DB_NAME = 'artcraft-layer-drafts';
/** Version 2 keeps documents apart from the details that lists read. */
const DB_VERSION = 2;
const DRAFTS = 'drafts';
const DOCUMENTS = 'documents';
const LOCK_PREFIX = 'artcraft-layer-draft:';
const newestFirst = (a: LayerDraftSummary, b: LayerDraftSummary) => b.updatedAt - a.updatedAt;

function summary({ document: _document, ...details }: LayerDraft): LayerDraftSummary {
  return details;
}

export function createMemoryDraftStore(durable = false): LayerDraftStore {
  const rows = new Map<string, LayerDraft>();
  const locks = new Set<string>();
  return {
    durable: async () => durable,
    put: async draft => { rows.set(draft.id, structuredClone(draft)); },
    get: async id => { const row = rows.get(id); return row && structuredClone(row); },
    list: async owner => [...rows.values()].filter(row => row.owner === owner).map(row => structuredClone(summary(row))).sort(newestFirst),
    update: async (id, details) => {
      const row = rows.get(id);
      if (row) rows.set(id, { ...row, ...structuredClone(details), id, owner: row.owner });
    },
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
      const request = factory.open(DB_NAME, DB_VERSION);
      request.onupgradeneeded = event => upgrade(request.result, request.transaction, event.oldVersion);
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

function upgrade(database: IDBDatabase, transaction: IDBTransaction | null, oldVersion: number): void {
  const drafts = oldVersion < 1 || !transaction
    ? database.createObjectStore(DRAFTS, { keyPath: 'id' })
    : transaction.objectStore(DRAFTS);
  if (oldVersion < 1) drafts.createIndex('owner', 'owner');
  if (database.objectStoreNames.contains(DOCUMENTS)) return;
  const documents = database.createObjectStore(DOCUMENTS, { keyPath: 'id' });
  if (oldVersion < 1) return;
  // Version 1 kept each document inside its draft row: move it out so lists stop reading it.
  drafts.openCursor().onsuccess = event => {
    const cursor = (event.target as IDBRequest<IDBCursorWithValue | null>).result;
    if (!cursor) return;
    const row = cursor.value as LayerDraft;
    if (row.document !== undefined) {
      documents.put({ id: row.id, document: row.document });
      cursor.update(summary(row));
    }
    cursor.continue();
  };
}

function createIndexedDbStore(factory: IDBFactory | undefined): LayerDraftStore {
  const fallback = createMemoryDraftStore();
  const database = openDatabase(factory);
  /** Runs `body` in one transaction over both stores; `body` must issue its requests before it awaits. */
  async function transact<T>(mode: IDBTransactionMode, body: (transaction: IDBTransaction) => Promise<T>): Promise<T> {
    const db = await database;
    if (!db) throw new Error('Local storage is unavailable.');
    const transaction = db.transaction([DRAFTS, DOCUMENTS], mode);
    const done = new Promise<void>((resolve, reject) => {
      transaction.oncomplete = () => resolve();
      transaction.onerror = () => reject(transaction.error ?? new Error('Local storage request failed.'));
      transaction.onabort = () => reject(transaction.error ?? new Error('Local storage request was aborted.'));
    });
    const result = body(transaction);
    // A failed request fails the transaction too, and that error is the one reported: never leave this unhandled.
    result.catch(() => undefined);
    await done;
    return result;
  }
  const locks = (globalThis.navigator as Navigator | undefined)?.locks;
  return {
    durable: async () => !!(await database),
    async put(draft) {
      if (!(await database)) return fallback.put(draft);
      await transact('readwrite', async transaction => {
        transaction.objectStore(DRAFTS).put(summary(draft));
        transaction.objectStore(DOCUMENTS).put({ id: draft.id, document: draft.document });
      });
    },
    async get(id) {
      if (!(await database)) return fallback.get(id);
      return transact('readonly', async transaction => {
        const [details, stored] = await Promise.all([
          settle(transaction.objectStore(DRAFTS).get(id) as IDBRequest<LayerDraftSummary | undefined>),
          settle(transaction.objectStore(DOCUMENTS).get(id) as IDBRequest<{ document: LayerProject } | undefined>),
        ]);
        return details && stored ? { ...details, document: stored.document } : undefined;
      });
    },
    async list(owner) {
      if (!(await database)) return fallback.list(owner);
      const rows = await transact('readonly', transaction =>
        settle(transaction.objectStore(DRAFTS).index('owner').getAll(owner) as IDBRequest<LayerDraftSummary[]>));
      return rows.sort(newestFirst);
    },
    async update(id, details) {
      if (!(await database)) return fallback.update(id, details);
      await transact('readwrite', transaction => new Promise<void>((resolve, reject) => {
        const drafts = transaction.objectStore(DRAFTS);
        const read = drafts.get(id) as IDBRequest<LayerDraftSummary | undefined>;
        read.onerror = () => reject(read.error ?? new Error('Local storage request failed.'));
        // Written inside the success event, while the transaction is still active.
        read.onsuccess = () => {
          if (read.result) drafts.put({ ...read.result, ...details, id, owner: read.result.owner });
          resolve();
        };
      }));
    },
    async remove(id) {
      if (!(await database)) return fallback.remove(id);
      await transact('readwrite', async transaction => {
        transaction.objectStore(DRAFTS).delete(id);
        transaction.objectStore(DOCUMENTS).delete(id);
      });
    },
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

const stores = new WeakMap<IDBFactory, LayerDraftStore>();
let pageFallback: LayerDraftStore | undefined;

/**
 * IndexedDB-backed drafts, falling back to page memory when the browser blocks storage. One store
 * (and one database connection) per factory is shared by every editor mount of the page.
 */
export function openLayerDraftStore(factory: IDBFactory | undefined = globalThis.indexedDB): LayerDraftStore {
  if (!factory) return (pageFallback ??= createIndexedDbStore(undefined));
  let store = stores.get(factory);
  if (!store) { store = createIndexedDbStore(factory); stores.set(factory, store); }
  return store;
}

/** Content identity of serialized document JSON: the exact bytes a save uploads. */
export async function hashLayerJson(json: string): Promise<string> {
  const digest = await crypto.subtle.digest('SHA-256', new TextEncoder().encode(json));
  return Array.from(new Uint8Array(digest), byte => byte.toString(16).padStart(2, '0')).join('');
}

/** Content identity of a document, stable across a JSON round trip through the server. */
export function hashLayerDocument(document: LayerProject): Promise<string> {
  return hashLayerJson(JSON.stringify(document));
}
