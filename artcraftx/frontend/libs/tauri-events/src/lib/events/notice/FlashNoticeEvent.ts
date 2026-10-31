import { listen, UnlistenFn } from '@tauri-apps/api/event';
import { BasicEventWrapper } from '../../common/BasicEventWrapper';
import { useEffect } from 'react';

const EVENT_NAME : string = 'flash_notice_event';

/** An informational (non-error) flash from the backend — e.g. "Higgsfield
 *  is checking your media for Intellectual Property and Likeness". */
export interface FlashNoticeEvent {
  message: string,
}

export const useFlashNoticeEvent = (asyncCallback: (event: FlashNoticeEvent) => Promise<void>) => {
  useEffect(() => {
    let isUnmounted = false;
    let unlisten: Promise<UnlistenFn>;

    const setup = async () => {
      unlisten = listen<BasicEventWrapper<FlashNoticeEvent>>(EVENT_NAME, async (wrappedEvent) => {
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
