import type { EditorCore } from "../index";

type SaveManagerOptions = {
  debounceMs?: number;
};

export class SaveManager {
  private debounceMs: number;
  private isPaused = false;
  private isSaving = false;
  private hasPendingSave = false;
  private saveTimer: ReturnType<typeof setTimeout> | null = null;
  private unsubscribeHandlers: Array<() => void> = [];

  constructor({
    editor,
    debounceMs = 800,
  }: {
    editor: EditorCore;
  } & SaveManagerOptions) {
    this.editor = editor;
    this.debounceMs = debounceMs;
  }

  private editor: EditorCore;

  start(): void {
    if (this.unsubscribeHandlers.length > 0) return;

    this.unsubscribeHandlers = [
      this.editor.scenes.subscribe(() => {
        this.markDirty();
      }),
      this.editor.timeline.subscribe(() => {
        this.markDirty();
      }),
    ];
  }

  stop(): void {
    for (const unsubscribe of this.unsubscribeHandlers) {
      unsubscribe();
    }
    this.unsubscribeHandlers = [];
    this.clearTimer();
  }

  pause(): void {
    this.isPaused = true;
  }

  resume(): void {
    this.isPaused = false;
    if (this.hasPendingSave) {
      this.queueSave();
    }
  }

  markDirty({ force = false }: { force?: boolean } = {}): void {
    if (this.isPaused && !force) return;
    this.hasPendingSave = true;
    this.queueSave();
  }

  async flush(): Promise<void> {
    this.hasPendingSave = true;
    await this.saveNow();
  }

  getIsDirty(): boolean {
    return this.hasPendingSave || this.isSaving;
  }

  private queueSave(delayMs = this.debounceMs): void {
    if (this.isSaving) return;
    if (this.saveTimer) {
      clearTimeout(this.saveTimer);
    }
    this.saveTimer = setTimeout(() => {
      // Storage adapters surface failures. Keep autosave rejections handled;
      // explicit flush() callers can still observe the failure.
      void this.saveNow().catch(() => {});
    }, delayMs);
  }

  private async saveNow(): Promise<void> {
    if (this.isSaving) return;
    if (!this.hasPendingSave) return;

    const activeProject = this.editor.project.getActive();
    if (!activeProject) return;
    if (this.editor.project.getIsLoading()) return;
    if (this.editor.project.getMigrationState().isMigrating) return;

    this.isSaving = true;
    this.hasPendingSave = false;
    this.clearTimer();

    let failed = false;
    try {
      await this.editor.project.saveCurrentProject();
    } catch (error) {
      failed = true;
      this.hasPendingSave = true;
      throw error;
    } finally {
      this.isSaving = false;
      if (this.hasPendingSave && !this.isPaused && this.unsubscribeHandlers.length > 0) {
        this.queueSave(failed ? Math.max(5000, this.debounceMs) : this.debounceMs);
      }
    }
  }

  private clearTimer(): void {
    if (!this.saveTimer) return;
    clearTimeout(this.saveTimer);
    this.saveTimer = null;
  }
}
