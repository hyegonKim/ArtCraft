import { listen, UnlistenFn } from '@tauri-apps/api/event';
import { BasicEventWrapper } from '../../common/BasicEventWrapper';
import { useEffect } from 'react';

const EVENT_NAME : string = 'show_credential_error_modal_event';

export interface ShowCredentialErrorModalEvent {
  message: string,
  /**
   * When the fix is to log into a website again, the `LoginWebsite` value
   * (e.g. "higgsfield") to pass to `open_web_login_command`.
   */
  maybe_relogin_website?: string,
  /** The credential that re-login should refresh in place. */
  maybe_credential_id?: string,
}

export const useShowCredentialErrorModalEvent = (asyncCallback: (event: ShowCredentialErrorModalEvent) => Promise<void>) => {
  useEffect(() => {
    let isUnmounted = false;
    let unlisten: Promise<UnlistenFn>;

    const setup = async () => {
      unlisten = listen<BasicEventWrapper<ShowCredentialErrorModalEvent>>(EVENT_NAME, async (wrappedEvent) => {
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
