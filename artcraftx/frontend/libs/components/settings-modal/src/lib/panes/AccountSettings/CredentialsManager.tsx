import { useCallback, useEffect, useState } from "react";
import { useRefreshAccountStateEvent } from "@storyteller/tauri-events";
import { Button } from "@storyteller/ui-button";
import { Key, LogIn, Pen, Plus, Trash2 } from "lucide-react";
import {
  CredentialPayload,
  ServiceMeta,
  getServiceLogoPath,
  getServiceMeta,
  listCredentials,
  refreshGrokStatsig,
  openWebLogin,
} from "./credential-helpers";
import { AddCredentialModal } from "./AddCredentialModal";
import { ArtcraftLoginModal } from "./ArtcraftLoginModal";
import { EditCredentialModal } from "./EditCredentialModal";
import { DeleteCredentialModal } from "./DeleteCredentialModal";

/**
 * The accounts pane body: lists credential files from the credentials
 * directory (starts empty for new installs) and offers add / edit / delete.
 */
export const CredentialsManager = () => {
  const [credentials, setCredentials] = useState<CredentialPayload[]>([]);
  const [isLoading, setIsLoading] = useState(true);
  const [isAddOpen, setIsAddOpen] = useState(false);
  const [passwordLoginService, setPasswordLoginService] =
    useState<ServiceMeta | null>(null);
  const [editing, setEditing] = useState<CredentialPayload | null>(null);
  const [deleting, setDeleting] = useState<CredentialPayload | null>(null);

  const refresh = useCallback(async () => {
    try {
      const result = await listCredentials();
      setCredentials(result);
    } catch (e) {
      console.error("Failed to list credentials", e);
    } finally {
      setIsLoading(false);
    }
  }, []);

  useEffect(() => {
    refresh();
    // Preemptively top up the Grok statsig material if it's missing/stale.
    // Fire-and-forget; no-op unless a Grok credential exists and is due.
    refreshGrokStatsig();
  }, [refresh]);

  // A completed web login saves a credential and emits this event; reload so
  // the new (or refreshed) credential shows up without reopening the modal.
  useRefreshAccountStateEvent(async () => {
    await refresh();
  });

  return (
    <div className="flex flex-col gap-3 text-base-fg">
      <div className="flex items-center justify-between">
        <span className="text-sm text-base-fg/60">
          API keys and website logins
        </span>
        <Button
          variant="primary"
          className="h-8"
          icon={Plus}
          onClick={() => setIsAddOpen(true)}
        >
          Add
        </Button>
      </div>

      {isLoading ? null : credentials.length === 0 ? (
        <div className="flex flex-col items-center gap-2 rounded-lg border border-dashed border-ui-panel-border px-6 py-10 text-center">
          <Key size="1em" className="text-xl text-base-fg/30" />
          <p className="text-sm text-base-fg/60">No accounts yet.</p>
          <p className="text-xs text-base-fg/40">
            Add an API key or log into a website to get started.
          </p>
        </div>
      ) : (
        <div className="flex flex-col gap-1.5">
          {credentials.map((credential) => (
            <CredentialRow
              key={credential.id}
              credential={credential}
              onEdit={() => setEditing(credential)}
              onDelete={() => setDeleting(credential)}
            />
          ))}
        </div>
      )}

      <AddCredentialModal
        isOpen={isAddOpen}
        onClose={() => setIsAddOpen(false)}
        onAdded={refresh}
        onChooseArtcraftLogin={(service) => {
          setIsAddOpen(false);
          setPasswordLoginService(service);
        }}
      />
      <ArtcraftLoginModal
        service={passwordLoginService}
        onClose={() => setPasswordLoginService(null)}
        onLoggedIn={refresh}
        onBack={() => {
          setPasswordLoginService(null);
          setIsAddOpen(true);
        }}
      />
      <EditCredentialModal
        credential={editing}
        onClose={() => setEditing(null)}
        onSaved={refresh}
      />
      <DeleteCredentialModal
        credential={deleting}
        onClose={() => setDeleting(null)}
        onDeleted={refresh}
      />
    </div>
  );
};

/**
 * Auto-assigned usernames (notably Midjourney's, e.g. "u3869671173") are just
 * "u" followed by digits and carry no human meaning.
 */
const AUTO_ASSIGNED_USERNAME_PATTERN = /^u\d+$/;

/**
 * Identity line for a cookie (website-login) account. Prefer a human-looking
 * username; when the username is auto-assigned, fall back to the email so we
 * don't surface a meaningless "u{random}" handle.
 */
const cookieIdentityLine = (credential: CredentialPayload): string => {
  const username = credential.username?.trim();
  const email = credential.email?.trim();

  const parts: string[] = [];
  if (username && !AUTO_ASSIGNED_USERNAME_PATTERN.test(username)) {
    parts.push(username);
  }
  if (email) {
    parts.push(email);
  }
  // No human username and no email: fall back to the raw username, if any.
  if (parts.length === 0 && username) {
    parts.push(username);
  }

  return parts.join(" · ") || "Website login";
};

const CredentialRow = ({
  credential,
  onEdit,
  onDelete,
}: {
  credential: CredentialPayload;
  onEdit: () => void;
  onDelete: () => void;
}) => {
  const meta = getServiceMeta(credential.service);
  const isApiKey = credential.kind === "api_key";
  // API keys: "Service (optional name)" over a partial key (prefix +
  // asterisks). Cookies: label over whatever identity info the file carries.
  const primaryLine = isApiKey
    ? credential.name
      ? `${meta.label} (${credential.name})`
      : meta.label
    : credential.name || meta.label;
  const secondaryLine = isApiKey
    ? `${credential.api_key_preview ?? ""}${"*".repeat(12)}`
    : cookieIdentityLine(credential);
  // The provider rejected this session; nothing is attempted with it until
  // the user logs in again (which refreshes this credential in place).
  const needsRelogin = credential.relogin_required_since !== null;
  const canRelogin = needsRelogin && !!meta.loginWebsite;

  return (
    <div
      className={`group flex items-center gap-3 rounded-lg border border-ui-panel-border bg-ui-controls/30 px-3 py-2.5 ${
        isApiKey ? "cursor-pointer hover:bg-ui-controls/60" : ""
      }`}
      onClick={isApiKey ? onEdit : undefined}
    >
      <img
        src={getServiceLogoPath(credential.service)}
        alt={`${meta.label} logo`}
        className="h-6 w-6 shrink-0 object-contain icon-auto-contrast"
      />
      <div className="flex min-w-0 grow flex-col">
        <span className="truncate text-sm font-medium">{primaryLine}</span>
        <span
          className={`truncate text-xs text-base-fg/50 ${isApiKey ? "font-mono" : ""}`}
        >
          {secondaryLine}
        </span>
        {needsRelogin && (
          <span className="truncate text-xs text-amber-500">
            Session expired. {meta.label} needs you to log in again.
          </span>
        )}
      </div>
      {canRelogin && (
        <Button
          variant="primary"
          className="h-8"
          icon={LogIn}
          onClick={(e: React.MouseEvent) => {
            e.stopPropagation();
            openWebLogin(meta.loginWebsite!, credential.id);
          }}
        >
          Log in
        </Button>
      )}
      {isApiKey && (
        <Button
          variant="secondary"
          className="h-8 w-8 opacity-0 transition-opacity group-hover:opacity-100"
          icon={Pen}
          onClick={(e: React.MouseEvent) => {
            e.stopPropagation();
            onEdit();
          }}
        />
      )}
      <Button
        variant="secondary"
        className="h-8 w-8 opacity-0 transition-opacity group-hover:opacity-100"
        icon={Trash2}
        onClick={(e: React.MouseEvent) => {
          e.stopPropagation();
          onDelete();
        }}
      />
    </div>
  );
};
