import { listen, UnlistenFn } from '@tauri-apps/api/event';
import { BasicEventWrapper } from '../../common/BasicEventWrapper';
import { useEffect } from 'react';

const EVENT_NAME : string = 'progress_notice_event';

export enum ProgressNoticeState {
  Started = "started",
  Finished = "finished",
}

/** A long-running backend step the user has to wait on — e.g. Higgsfield's
 *  IP/likeness check on uploaded media. `started` and `finished` share a
 *  `notice_id`; the message only comes with `started`. */
export interface ProgressNoticeEvent {
  notice_id: string,
  state: ProgressNoticeState,
  maybe_message?: string | null,
}

export const useProgressNoticeEvent = (asyncCallback: (event: ProgressNoticeEvent) => Promise<void>) => {
  useEffect(() => {
    let isUnmounted = false;
    let unlisten: Promise<UnlistenFn>;

    const setup = async () => {
      unlisten = listen<BasicEventWrapper<ProgressNoticeEvent>>(EVENT_NAME, async (wrappedEvent) => {
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
