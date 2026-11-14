import type { LayerProject } from '@storyteller/ui-pagedraw';
import type { LayerDraftStore, LayerDraftSummary } from './layer-draft-store';

export const LAYER_AUTOSAVE_DELAY_MS = 1000;
export const LAYER_SAVE_RETRY_MS = 5000;
/** Device copies are written sooner than server saves so a closed tab loses as little as possible. */
export const LAYER_LOCAL_DELAY_MS = 300;
/** A server check after an outage stays valid this long while retries continue. */
export const LAYER_VERIFY_TTL_MS = 30_000;
const OWN_HASH_LIMIT = 8;
const NAME_LIMIT = 120;
const COPY_SUFFIX = ' (offline copy)';
export const LOCAL_RECOVERY_UNAVAILABLE = 'This browser is not keeping a recovery copy on this device (storage is blocked or private browsing). Stay online until Saved appears.';

export interface LayerSaveState {
  dirty: boolean;
  saving: boolean;
  error: string;
  retrying: boolean;
  /** The browser reports no network; server saves wait for the online event. */
  offline: boolean;
  /** The latest edit is kept on this device and survives closing the browser. */
  local: boolean;
  localError: string;
  /** An outcome the user must know about, e.g. a conflict saved as a separate copy. */
  notice: string;
  /** This device can keep copies that survive closing the browser (signed in, storage allowed). */
  deviceCopies: boolean;
}

/** The server no longer has the project (deleted or no longer owned by this account). */
export class LayerProjectUnavailableError extends Error {}

interface Options {
  capture(): Promise<LayerProject>;
  getName(): string;
  /** `json` is `document` already serialized: upload exactly these bytes. */
  save(document: LayerProject, name: string, token: string | undefined, requestId: string, json?: string): Promise<string>;
  /** `name` is set when the save went to a differently named copy. */
  onSaved(token: string, name?: string): void;
  onState(state: LayerSaveState): void;
  drafts?: LayerDraftStore;
  /** Identity of serialized document JSON (the bytes a save uploads). */
  hash?(json: string): Promise<string>;
  /** Hash of the document now on the server; throws LayerProjectUnavailableError when it is gone. */
  serverHash?(token: string): Promise<string>;
  online?(): boolean;
  now?(): number;
}

type DraftMeta = Pick<LayerDraftSummary, 'token' | 'copyPending' | 'name' | 'baseHash' | 'ownHashes'>;

/** The name of a conflict copy: one suffix however often it conflicts, cut between characters. */
export function offlineCopyName(name: string): string {
  if (name.endsWith(COPY_SUFFIX)) return name;
  return Array.from(name).slice(0, NAME_LIMIT - COPY_SUFFIX.length).join('') + COPY_SUFFIX;
}

function localErrorMessage(error: unknown): string {
  return error instanceof DOMException && error.name === 'QuotaExceededError'
    ? 'This device is out of storage, so the latest edits are not kept locally. Stay online until Saved appears.'
    : 'Could not keep a recovery copy on this device. Stay online until Saved appears.';
}

/**
 * One serialized save stream per document. Each edit is first kept on this device (IndexedDB) and
 * then sent to the server; retries and reconnects capture the latest unsaved revision. The device
 * copy is deleted only after the server confirmed the latest revision.
 */
export class LayerAutosave {
  private revision = 0;
  private savedRevision = 0;
  private localRevision = 0;
  private token?: string;
  private enabled = false;
  private disposed = false;
  private online: boolean;
  private epoch = 0;
  private error = '';
  private localError = '';
  private notice = '';
  private timer?: ReturnType<typeof setTimeout>;
  private localTimer?: ReturnType<typeof setTimeout>;
  private flight?: Promise<boolean>;
  private copyPending = false;
  private owner?: string;
  private lastOwner?: string;
  private durable?: boolean;
  private draftId: string = crypto.randomUUID();
  private release?: () => void;
  /** A lock dispose() will release once its last device write lands, unless the same document is reactivated. */
  private parked?: { id: string; release: () => void };
  /** The revision whose device write is queued, so hiding and closing a tab do not write it twice. */
  private queuedRevision = 0;
  private baseHash?: string;
  private ownHashes: string[] = [];
  private needsVerify = false;
  private verifiedAt = -Infinity;
  /** The server copy changed or vanished while edits waited; retries go to a separate project, never the original. */
  private conflict = false;
  private chain: Promise<void> = Promise.resolve();

  constructor(private readonly options: Options) {
    this.online = options.online?.() ?? true;
  }

  get state(): LayerSaveState {
    const dirty = this.revision !== this.savedRevision;
    const unavailable = !!this.owner && !!this.options.drafts && this.durable === false;
    return { dirty, saving: !!this.flight, error: this.error,
      retrying: !!this.error && this.enabled && dirty && this.online, offline: !this.online,
      local: dirty && !!this.owner && this.durable === true && this.localRevision === this.revision,
      localError: this.localError || (unavailable ? LOCAL_RECOVERY_UNAVAILABLE : ''), notice: this.notice,
      deviceCopies: !!this.owner && !!this.options.drafts && this.durable !== false };
  }

  /** Local draft id of the open document. */
  get currentDraftId(): string { return this.draftId; }

  activate(): void {
    this.disposed = false;
    this.flight = undefined;
    // Remounting the same document (StrictMode, a quick route change) keeps the lock dispose() parked.
    if (this.parked?.id === this.draftId) { this.release = this.parked.release; this.parked = undefined; }
    else if (!this.release) this.claimCurrent();
  }

  /**
   * Device copies are written only for a signed-in owner and are never shown to another account.
   * Returns true when a different account signed in: the open document then belongs to the previous
   * one, and this session has been reset so it is neither kept nor saved for the new account. The
   * caller clears the editor.
   */
  setOwner(owner: string | undefined): boolean {
    if (this.owner === owner) return false;
    // Keep what was typed before signing out or an expired session; signing back in continues the same record.
    if (this.owner && this.state.dirty && this.localRevision !== this.revision) void this.persistLocal(true);
    const switched = !!owner && !!this.lastOwner && owner !== this.lastOwner;
    // Its pending edit was just kept for the previous account above.
    if (switched) this.reset();
    if (owner) this.lastOwner = owner;
    this.owner = owner;
    this.durable = undefined;
    this.localError = '';
    const drafts = this.options.drafts;
    if (owner && drafts) {
      void drafts.durable().then(durable => {
        if (this.owner !== owner) return;
        this.durable = durable;
        this.emit();
      });
      if (this.state.dirty) this.scheduleLocal(0);
    }
    this.emit();
    return switched;
  }

  setOnline(online: boolean): void {
    if (this.online === online || this.disposed) return;
    this.online = online;
    if (!online) {
      this.clearTimer();
      // Another device may save while this one is offline; check before overwriting.
      this.needsVerify = true;
      this.verifiedAt = -Infinity;
    }
    this.emit();
    if (online) this.schedule(0);
  }

  markDirty(): void {
    if (this.disposed) return;
    this.revision += 1;
    this.emit();
    this.scheduleLocal(LAYER_LOCAL_DELAY_MS);
    if (!this.error) this.schedule(LAYER_AUTOSAVE_DELAY_MS);
    else if (!this.timer) this.schedule(LAYER_SAVE_RETRY_MS);
  }

  setEnabled(enabled: boolean): void {
    if (this.enabled === enabled || this.disposed) return;
    this.enabled = enabled;
    this.clearTimer();
    this.emit();
    if (enabled) this.schedule(this.error ? LAYER_SAVE_RETRY_MS : LAYER_AUTOSAVE_DELAY_MS);
  }

  /** Starts a new document session. `baseHash` identifies an opened server document. */
  reset(token?: string, next: { baseHash?: string; draftId?: string; release?: () => void } = {}): void {
    this.epoch += 1;
    this.clearTimer();
    this.clearLocalTimer();
    this.token = token;
    this.revision = this.savedRevision = this.localRevision = this.queuedRevision = 0;
    this.error = '';
    this.localError = '';
    this.notice = '';
    this.flight = undefined;
    this.copyPending = false;
    this.baseHash = next.baseHash;
    this.ownHashes = [];
    this.needsVerify = false;
    this.verifiedAt = -Infinity;
    this.conflict = false;
    this.bindDraft(next.draftId ?? crypto.randomUUID(), next.release);
    this.emit();
  }

  /** Continues a document recovered from this device. It stays unsynced until the server confirms it. */
  adopt(draft: LayerDraftSummary, release?: () => void): void {
    this.reset(draft.token, { baseHash: draft.baseHash, draftId: draft.id, release });
    this.copyPending = draft.copyPending;
    this.ownHashes = [...draft.ownHashes];
    this.revision = this.localRevision = 1;
    this.needsVerify = true;
    this.emit();
    this.schedule(0);
  }

  /** The user chose to throw away the open document's unsynced edits. */
  discardLocal(): Promise<void> {
    this.clearLocalTimer();
    const drafts = this.options.drafts;
    const id = this.draftId;
    const epoch = this.epoch;
    // Until the document is replaced, its edits exist only in the editor and must be written again.
    this.localRevision = this.queuedRevision = 0;
    this.emit();
    if (!drafts) return Promise.resolve();
    return this.enqueue(async () => {
      await drafts.remove(id);
      // A device write queued before the discard may have marked the edits as kept meanwhile.
      if (epoch === this.epoch) { this.localRevision = 0; this.emit(); }
    });
  }

  dismissNotice(): void { this.notice = ''; this.emit(); }

  /** Writes the device copy now, e.g. when the page is hidden or about to close. */
  persistNow(): Promise<void> { return this.persistLocal(false); }

  flush(asCopy = false): Promise<boolean> {
    if (this.disposed || !this.enabled) return Promise.resolve(false);
    if (this.flight) return this.flight;
    if (asCopy) { this.copyPending = true; this.revision += 1; this.scheduleLocal(0); }
    this.clearTimer();
    if (!this.state.dirty) return Promise.resolve(true);
    if (!this.online) { this.emit(); return Promise.resolve(false); }
    const epoch = this.epoch;
    const revision = this.revision;
    const name = this.options.getName().trim() || 'Untitled image project';
    const original = this.copyPending || this.conflict ? undefined : this.token;
    const current = () => epoch === this.epoch && !this.disposed;
    let attempted: string | undefined;
    const task = Promise.resolve().then(async () => {
      const document = structuredClone(await this.options.capture());
      if (!current()) return false;
      // Serialized once: the hash identifies exactly the bytes the save uploads.
      const json = this.options.hash ? JSON.stringify(document) : undefined;
      const hash = json === undefined ? undefined : await this.options.hash?.(json);
      if (!current()) return false;
      let token = original;
      let copyName = this.conflict ? offlineCopyName(name) : undefined;
      if (token && await this.serverChanged(token, hash)) {
        if (!current()) return false;
        token = undefined;
        copyName = offlineCopyName(name);
        // Remember the verdict: a failed copy upload must never fall back to overwriting the original.
        this.conflict = true;
      }
      if (!current()) return false;
      if (token) attempted = hash;
      // The legacy API rejects reused UUIDs, including some failed uploads.
      const saved = await this.options.save(document, copyName ?? name, token, crypto.randomUUID(), json);
      if (!current()) return false;
      this.token = saved;
      this.savedRevision = revision;
      this.copyPending = false;
      this.conflict = false;
      this.error = '';
      this.baseHash = hash;
      this.ownHashes = [];
      this.needsVerify = false;
      if (copyName) {
        this.notice = `The server copy changed or was removed while these edits were waiting, so they were saved as a new private project “${copyName}”. The original was not overwritten.`;
      }
      this.options.onSaved(saved, copyName);
      this.acknowledge();
      return true;
    }).catch((error: unknown) => {
      if (!current()) return false;
      this.error = error instanceof Error ? error.message : 'Could not save project.';
      this.needsVerify = true;
      // The write may have landed even though its response was lost; that version is ours, not a conflict.
      if (attempted && !this.ownHashes.includes(attempted)) {
        this.ownHashes = [...this.ownHashes, attempted].slice(-OWN_HASH_LIMIT);
        this.updateDraft(false);
      }
      return false;
    }).finally(() => {
      if (!current()) return;
      this.flight = undefined;
      this.emit();
      this.schedule(this.error ? LAYER_SAVE_RETRY_MS : LAYER_AUTOSAVE_DELAY_MS);
    });
    this.flight = task;
    this.emit();
    return task;
  }

  dispose(): void {
    // Unmounting (route change, StrictMode replay) must not drop an edit from the last few hundred ms.
    if (this.state.dirty && this.localRevision !== this.revision) void this.persistLocal(true);
    this.disposed = true;
    this.enabled = false;
    this.epoch += 1;
    this.clearTimer();
    this.clearLocalTimer();
    // Another tab may restore this copy as soon as the lock is gone: release it after the write above lands.
    const release = this.release;
    this.release = undefined;
    if (release) {
      const parked = { id: this.draftId, release };
      this.parked = parked;
      void this.enqueue(async () => {
        if (this.parked !== parked) return;
        this.parked = undefined;
        release();
      });
    }
  }

  /** True when overwriting `token` could destroy a version this device never saw. */
  private async serverChanged(token: string, hash: string | undefined): Promise<boolean> {
    const now = this.options.now?.() ?? Date.now();
    if (!this.needsVerify || !this.options.serverHash || !this.baseHash || now - this.verifiedAt < LAYER_VERIFY_TTL_MS) return false;
    let server: string;
    try {
      server = await this.options.serverHash(token);
    } catch (error) {
      if (error instanceof LayerProjectUnavailableError) return true;
      throw error;
    }
    const changed = server !== this.baseHash && server !== hash && !this.ownHashes.includes(server);
    // Only a passing check may be reused while retries continue.
    if (!changed) this.verifiedAt = now;
    return changed;
  }

  private persistLocal(detached: boolean): Promise<void> {
    this.clearLocalTimer();
    const drafts = this.options.drafts;
    const owner = this.owner;
    if (!drafts || !owner || !this.state.dirty || this.localRevision === this.revision || this.queuedRevision === this.revision) {
      return this.chain;
    }
    const epoch = this.epoch;
    const revision = this.revision;
    this.queuedRevision = revision;
    // A write that did not land lets the next request for this revision try again.
    const unqueue = () => { if (epoch === this.epoch && this.queuedRevision === revision) this.queuedRevision = 0; };
    const id = this.draftId;
    const scheduled = this.draftMeta();
    // capture() reads the scene synchronously before its first await.
    const snapshot = this.options.capture();
    snapshot.catch(() => undefined);
    return this.enqueue(async () => {
      const live = epoch === this.epoch && !this.disposed;
      if (!live && !detached) { unqueue(); return; }
      let document: LayerProject;
      // IndexedDB clones synchronously inside put(), so the fresh capture needs no extra copy.
      try { document = await snapshot; } catch (error) {
        // While saving is disabled there is no open document to recover yet; otherwise say nothing was kept.
        unqueue();
        if (epoch === this.epoch && this.enabled) {
          this.localError = `Could not keep a recovery copy on this device. ${error instanceof Error ? error.message : ''}`.trim();
          this.emit();
        }
        return;
      }
      if (revision <= this.savedRevision) { unqueue(); return; }
      try {
        await drafts.put({ id, owner, document, updatedAt: this.options.now?.() ?? Date.now(),
          ...(epoch === this.epoch ? this.draftMeta() : scheduled) });
        if (epoch !== this.epoch) return;
        this.localRevision = Math.max(this.localRevision, revision);
        this.localError = '';
      } catch (error) {
        unqueue();
        if (epoch === this.epoch) this.localError = localErrorMessage(error);
      }
      if (epoch === this.epoch) this.emit();
    });
  }

  /** The server confirmed a revision: drop the device copy, or keep newer edits under the new identity. */
  private acknowledge(): void { this.updateDraft(this.revision === this.savedRevision); }

  private updateDraft(synced: boolean): void {
    const drafts = this.options.drafts;
    if (!drafts) return;
    const id = this.draftId;
    const epoch = this.epoch;
    const meta = this.draftMeta();
    void this.enqueue(async () => {
      const live = epoch === this.epoch;
      if (live ? this.revision === this.savedRevision : synced) { await drafts.remove(id); return; }
      // Details only: the kept document is newer than the revision the server confirmed.
      await drafts.update(id, live ? this.draftMeta() : meta);
    });
  }

  private draftMeta(): DraftMeta {
    return { token: this.token, copyPending: this.copyPending, name: this.options.getName().trim() || 'Untitled image project',
      baseHash: this.baseHash, ownHashes: [...this.ownHashes] };
  }

  private enqueue(task: () => Promise<void>): Promise<void> {
    this.chain = this.chain.then(task).catch(() => undefined);
    return this.chain;
  }

  private bindDraft(id: string, release?: () => void): void {
    if (this.release && (id !== this.draftId || release)) {
      // After any device write already queued for the previous document.
      const previous = this.release;
      this.release = undefined;
      void this.enqueue(async () => previous());
    }
    this.draftId = id;
    if (release) this.release = release;
    else if (!this.release) this.claimCurrent();
  }

  private claimCurrent(): void {
    const drafts = this.options.drafts;
    const id = this.draftId;
    if (!drafts || this.disposed) return;
    void drafts.claim(id).then(release => {
      if (!release) return;
      if (this.draftId !== id || this.disposed || this.release) release();
      else this.release = release;
    });
  }

  private schedule(delay: number): void {
    if (!this.enabled || this.disposed || this.flight || !this.state.dirty || !this.online) return;
    this.clearTimer();
    this.timer = setTimeout(() => { this.timer = undefined; void this.flush(); }, delay);
  }
  private scheduleLocal(delay: number): void {
    if (this.disposed || !this.options.drafts || !this.owner) return;
    this.clearLocalTimer();
    this.localTimer = setTimeout(() => { this.localTimer = undefined; void this.persistLocal(false); }, delay);
  }
  private clearTimer(): void { clearTimeout(this.timer); this.timer = undefined; }
  private clearLocalTimer(): void { clearTimeout(this.localTimer); this.localTimer = undefined; }
  private emit(): void { this.options.onState(this.state); }
}
