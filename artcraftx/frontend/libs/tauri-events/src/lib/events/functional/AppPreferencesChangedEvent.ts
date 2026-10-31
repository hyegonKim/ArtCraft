import { listen, UnlistenFn } from "@tauri-apps/api/event";
import { BasicEventWrapper } from "../../common/BasicEventWrapper";
import { useEffect } from "react";

const EVENT_NAME: string = "app_preferences_changed_event";

// The backend saved the app preferences (any command). No payload; re-read.
export interface AppPreferencesChangedEvent {
}

export const useAppPreferencesChangedEvent = (
  asyncCallback: (event: AppPreferencesChangedEvent) => Promise<void>
) => {
  useEffect(() => {
    let isUnmounted = false;
    let unlisten: Promise<UnlistenFn>;

    const setup = async () => {
      unlisten = listen<BasicEventWrapper<AppPreferencesChangedEvent>>(
        EVENT_NAME,
        async (wrappedEvent) => {
          await asyncCallback(wrappedEvent.payload.data);
        }
      );

      if (isUnmounted) {
        unlisten.then((f) => f()); // Unsubscribe if unmounted early.
      }
    };

    setup();

    return () => {
      isUnmounted = true;
      unlisten.then((f) => f());
    };
  }, []);
};
