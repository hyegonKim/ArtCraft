import { useEffect } from "react";
import { STALE_AFTER_MS, useAppPreferencesStore } from "@storyteller/tauri-api";
import { useAppPreferencesChangedEvent } from "../events/functional/AppPreferencesChangedEvent";

// Keeps the frontend's app-preferences cache (`useAppPreferencesStore`) in
// sync with the backend. Mount ONCE near the app root.
//
//  - loads on mount
//  - reloads immediately when the backend emits `app_preferences_changed_event`
//    (every preference write does), so a change made in Settings — or from
//    another window — is visible everywhere at once
//  - reloads on window focus and on a stale timer as a safety net
export const useAppPreferencesSync = () => {
  const refresh = useAppPreferencesStore((s) => s.refresh);
  const refreshIfStale = useAppPreferencesStore((s) => s.refreshIfStale);

  useEffect(() => {
    refresh();

    const onFocus = () => { refreshIfStale(); };
    window.addEventListener("focus", onFocus);
    const timer = window.setInterval(refreshIfStale, STALE_AFTER_MS);

    return () => {
      window.removeEventListener("focus", onFocus);
      window.clearInterval(timer);
    };
  }, [refresh, refreshIfStale]);

  useAppPreferencesChangedEvent(async () => {
    await refresh();
  });
};
