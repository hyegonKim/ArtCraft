import type { LayerProject } from '@storyteller/ui-pagedraw';

export const LAYER_AUTOSAVE_DELAY_MS = 1000;
export const LAYER_SAVE_RETRY_MS = 5000;
export interface LayerSaveState {
  dirty: boolean;
  saving: boolean;
  error: string;
  retrying: boolean;
}
interface Options {
  capture(): Promise<LayerProject>;
  getName(): string;
  save(document: LayerProject, name: string, token: string | undefined, requestId: string): Promise<string>;
  onSaved(token: string): void;
  onState(state: LayerSaveState): void;
}

/** One serialized save stream per document; retries capture the latest unsaved revision. */
export class LayerAutosave {
  private revision = 0;
  private savedRevision = 0;
  private token?: string;
  private enabled = false;
  private disposed = false;
  private epoch = 0;
  private error = '';
  private timer?: ReturnType<typeof setTimeout>;
  private flight?: Promise<boolean>;
  private copyPending = false;

  constructor(private readonly options: Options) {}

  get state(): LayerSaveState {
    return { dirty: this.revision !== this.savedRevision, saving: !!this.flight,
      error: this.error, retrying: !!this.error && this.enabled && this.revision !== this.savedRevision };
  }

  activate(): void {
    this.disposed = false;
    this.flight = undefined;
  }

  markDirty(): void {
    if (this.disposed) return;
    this.revision += 1;
    this.emit();
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

  reset(token?: string): void {
    this.epoch += 1;
    this.clearTimer();
    this.token = token;
    this.revision = this.savedRevision = 0;
    this.error = '';
    this.flight = undefined;
    this.copyPending = false;
    this.emit();
  }

  flush(asCopy = false): Promise<boolean> {
    if (this.disposed || !this.enabled) return Promise.resolve(false);
    if (this.flight) return this.flight;
    if (asCopy) { this.copyPending = true; this.revision += 1; }
    this.clearTimer();
    if (!this.state.dirty) return Promise.resolve(true);
    const epoch = this.epoch;
    const revision = this.revision;
    const name = this.options.getName().trim() || 'Untitled image project';
    const token = this.copyPending ? undefined : this.token;
    const current = () => epoch === this.epoch && !this.disposed;
    const task = Promise.resolve().then(async () => {
      const document = structuredClone(await this.options.capture());
      if (!current()) return false;
      // The legacy API rejects reused UUIDs, including some failed uploads.
      const saved = await this.options.save(document, name, token, crypto.randomUUID());
      if (!current()) return false;
      this.token = saved;
      this.savedRevision = revision;
      this.copyPending = false;
      this.error = '';
      this.options.onSaved(saved);
      return true;
    }).catch((error: unknown) => {
      if (current()) this.error = error instanceof Error ? error.message : 'Could not save project.';
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
    this.disposed = true;
    this.enabled = false;
    this.epoch += 1;
    this.clearTimer();
  }

  private schedule(delay: number): void {
    if (!this.enabled || this.disposed || this.flight || !this.state.dirty) return;
    this.clearTimer();
    this.timer = setTimeout(() => { this.timer = undefined; void this.flush(); }, delay);
  }
  private clearTimer(): void { clearTimeout(this.timer); this.timer = undefined; }
  private emit(): void { this.options.onState(this.state); }
}
