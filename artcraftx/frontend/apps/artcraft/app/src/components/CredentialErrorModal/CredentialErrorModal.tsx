import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Modal } from "@storyteller/ui-modal";
import { Button } from "@storyteller/ui-button";
import { LogIn, UserX } from "lucide-react";
import { ShowCredentialErrorModalEvent, useShowCredentialErrorModalEvent } from "@storyteller/tauri-events";

// `LoginWebsite` values (crates/artcraftx/src/credentials/login_website.rs)
// as users know them.
const LOGIN_WEBSITE_NAMES: Record<string, string> = {
  artcraft: "ArtCraft",
  higgsfield: "Higgsfield",
  magnific: "Magnific",
  midjourney: "Midjourney",
  openart: "OpenArt",
  runway: "Runway",
  xai: "Grok",
};

/**
 * Dismissable modal shown when the backend reports a credential problem with
 * a generation request (no account selected, unknown credential id, or a
 * credential that can't serve the request). Mounted once in MainApp; opens
 * itself on `show_credential_error_modal_event`.
 *
 * When the problem is an expired session, the event names the website to
 * log into again and the credential to refresh, and the modal offers a
 * button that opens that login window. Logging in rewrites that credential
 * in place rather than adding a second account.
 */
export const CredentialErrorModal = () => {
  const [event, setEvent] = useState<ShowCredentialErrorModalEvent | null>(null);

  useShowCredentialErrorModalEvent(async (event) => {
    setEvent(event);
  });

  const close = () => setEvent(null);

  const reloginWebsite = event?.maybe_relogin_website;
  const reloginName = reloginWebsite ? LOGIN_WEBSITE_NAMES[reloginWebsite] ?? reloginWebsite : undefined;

  const openLogin = async () => {
    if (!reloginWebsite) return;
    const credentialId = event?.maybe_credential_id;
    close();
    try {
      await invoke("open_web_login_command", { website: reloginWebsite, credentialId });
    } catch (err) {
      console.error("[credentials] could not open login window:", err);
    }
  };

  return (
    <Modal
      isOpen={event !== null}
      onClose={close}
      title={reloginName ? "Login required" : "Account problem"}
      titleIcon={UserX}
      width={440}
      showClose={true}
    >
      <div className="flex flex-col gap-4 p-1 text-base-fg">
        <p className="text-sm text-base-fg/80">{event?.message}</p>
        {reloginWebsite ? (
          <div className="flex gap-2">
            <Button variant="primary" className="h-9 flex-1" icon={LogIn} onClick={openLogin}>
              Log in to {reloginName}
            </Button>
            <Button variant="secondary" className="h-9" onClick={close}>
              Not now
            </Button>
          </div>
        ) : (
          <Button variant="primary" className="h-9" onClick={close}>
            Dismiss
          </Button>
        )}
      </div>
    </Modal>
  );
};
