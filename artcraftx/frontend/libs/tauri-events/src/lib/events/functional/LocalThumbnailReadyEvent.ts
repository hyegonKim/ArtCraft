import { listen, UnlistenFn } from '@tauri-apps/api/event';
import { BasicEventWrapper } from '../../common/BasicEventWrapper';
import { useEffect } from 'react';

const EVENT_NAME : string = 'local_thumbnail_ready_event';

/** The backend's thumbnail worker finished a downloaded file. Paths are
 *  absolute; render them with `convertFileSrc`. */
export interface LocalThumbnailReadyEvent {
  /** The source file the thumbnail is for (keys the frontend cache). */
  file_path: string,
  /** Static jpg in the thumbnail cache. */
  thumbnail_path: string,
  /** ~1s animated webp, videos only. */
  maybe_animated_preview_path?: string | null,
}

export const useLocalThumbnailReadyEvent = (asyncCallback: (event: LocalThumbnailReadyEvent) => Promise<void>) => {
  useEffect(() => {
    let isUnmounted = false;
    let unlisten: Promise<UnlistenFn>;

    const setup = async () => {
      unlisten = listen<BasicEventWrapper<LocalThumbnailReadyEvent>>(EVENT_NAME, async (wrappedEvent) => {
        await asyncCallback(wrappedEvent.payload.data);
      });

      if (isUnmounted) {
        unlisten.then(f => f()); // Unsubscribe if unmounted early.
      }
    };

    setup();

    return () => {
      isUnmounted = true;
      unlisten.then(f => f());
    };

  }, []);
}
