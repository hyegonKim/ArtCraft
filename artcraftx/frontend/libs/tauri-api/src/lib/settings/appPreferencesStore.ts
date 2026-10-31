import { create } from "zustand";
import { AppBackupPreferences, AppPreferencesPayload, GetAppPreferences } from "./GetAppPreferences";

// ── App preferences cache ──
//
// The single frontend copy of the backend's app preferences. Hot paths (e.g.
// deciding what Enter does in a prompt box) read it synchronously; nothing
// calls the backend per keypress.
//
// Kept fresh by `useAppPreferencesSync` (in @storyteller/tauri-events), which
// loads it on mount, re-loads when the backend emits
// `app_preferences_changed_event` (sent after every preference write), and
// re-loads on window focus / when older than STALE_AFTER_MS as a safety net.
// Code that writes a preference can also call `refresh()` directly.

export const STALE_AFTER_MS = 60_000;

interface AppPreferencesStore {
  preferences: AppPreferencesPayload | undefined;
  // When `preferences` was last loaded (ms since epoch); undefined if never.
  loadedAt: number | undefined;
  // Re-read from the backend. Failures are logged and leave the cache as-is.
  refresh: () => Promise<void>;
  // Re-read only if never loaded or older than STALE_AFTER_MS.
  refreshIfStale: () => Promise<void>;
}

export const useAppPreferencesStore = create<AppPreferencesStore>()((set, get) => ({
  preferences: undefined,
  loadedAt: undefined,

  refresh: async () => {
    try {
      const result = await GetAppPreferences();
      set({ preferences: result.preferences, loadedAt: Date.now() });
    } catch (err) {
      console.warn("Could not load app preferences; keeping the cached copy:", err);
    }
  },

  refreshIfStale: async () => {
    const { loadedAt, refresh } = get();
    if (loadedAt === undefined || Date.now() - loadedAt > STALE_AFTER_MS) {
      await refresh();
    }
  },
}));

// ── Selectors ──
//
// Each falls back to the backend's default until the first load completes.

export const selectEnterToGenerate = (s: AppPreferencesStore): boolean =>
  s.preferences?.prompt.enter_to_generate ?? true;

// Whether Enter submits the prompt (Shift+Enter inserts a newline). Reactive
// and synchronous — safe to read in key handlers.
export const useEnterToGenerate = (): boolean =>
  useAppPreferencesStore(selectEnterToGenerate);

const DEFAULT_BACKUP_PREFERENCES: AppBackupPreferences = { enabled: false, maybe_artcraft_credential_id: null };

export const selectBackupPreferences = (s: AppPreferencesStore): AppBackupPreferences =>
  s.preferences?.backup ?? DEFAULT_BACKUP_PREFERENCES;

// The Account Backup settings (Settings → Account Backup). Falls back to
// "off, no account" until the first load completes.
export const useBackupPreferences = (): AppBackupPreferences =>
  useAppPreferencesStore(selectBackupPreferences);

// Non-hook access for code outside React (e.g. sound playback).
export const getCachedAppPreferences = (): AppPreferencesPayload | undefined =>
  useAppPreferencesStore.getState().preferences;
