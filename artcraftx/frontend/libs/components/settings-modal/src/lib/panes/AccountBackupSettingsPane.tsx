import { useCallback, useEffect, useState } from "react";
import { twMerge } from "tailwind-merge";
import {
  BackupPreferenceName,
  UpdateBackupPreference,
  useAppPreferencesStore,
  useBackupPreferences,
} from "@storyteller/tauri-api";
import { useRefreshAccountStateEvent } from "@storyteller/tauri-events";
import { Switch } from "@storyteller/ui-switch";
import { SettingsBlock, SettingsRow } from "./SettingsRow";
import {
  CredentialPayload,
  getServiceLogoPath,
  getServiceMeta,
  listCredentials,
} from "./AccountSettings/credential-helpers";

// Credential services that can receive backups: ArtCraft accounts with a web
// session. Mirrors `is_backup_eligible` in the backend (`services::backup`).
const BACKUP_ELIGIBLE_SERVICES = new Set(["artcraft", "artcraft_local", "artcraft_cookies"]);

const isBackupEligible = (credential: CredentialPayload): boolean =>
  credential.kind === "cookies" && BACKUP_ELIGIBLE_SERVICES.has(credential.service);

/**
 * Settings → Account Backup: opt in to copying generations made on other
 * services (Higgsfield, Midjourney, Runway, ...) to an ArtCraft account.
 *
 * The selection always reflects the persisted preference (never a local
 * guess): the pane reads it from the preferences store and writes changes
 * straight back. Turning backups off keeps the chosen account but greys the
 * list out.
 */
export const AccountBackupSettingsPane = () => {
  const backup = useBackupPreferences();
  const refreshPreferences = useAppPreferencesStore((s) => s.refresh);
  const [accounts, setAccounts] = useState<CredentialPayload[]>([]);
  const [saveError, setSaveError] = useState<string | null>(null);

  const loadAccounts = useCallback(async () => {
    try {
      const all = await listCredentials();
      setAccounts(all.filter(isBackupEligible));
    } catch (err) {
      console.error("[backup] could not list credentials:", err);
    }
  }, []);

  useEffect(() => {
    loadAccounts();
    refreshPreferences();
  }, [loadAccounts, refreshPreferences]);

  // A login / logout elsewhere changes which accounts are available.
  useRefreshAccountStateEvent(async () => {
    await loadAccounts();
  });

  const setEnabled = async (enabled: boolean) => {
    setSaveError(null);
    try {
      await UpdateBackupPreference({ preference: BackupPreferenceName.Enabled, value: enabled });
    } catch (err) {
      console.error("[backup] could not save enabled:", err);
      setSaveError(String(err));
    }
    await refreshPreferences();
  };

  const selectAccount = async (credentialId: string) => {
    if (!backup.enabled) return;
    setSaveError(null);
    try {
      await UpdateBackupPreference({
        preference: BackupPreferenceName.ArtcraftCredentialId,
        value: credentialId,
      });
    } catch (err) {
      console.error("[backup] could not save account:", err);
      setSaveError(String(err));
    }
    await refreshPreferences();
  };

  const selectedId = backup.maybe_artcraft_credential_id ?? null;
  const selectionMissing = backup.enabled && (selectedId === null || !accounts.some((a) => a.id === selectedId));

  return (
    <div className="text-base-fg">
      <SettingsBlock
        title="Back up your generations to ArtCraft"
        description={
          <>
            Generations you make with Higgsfield, Runway, and more can be backed up to your
            ArtCraft account, so you always have a copy of your images and videos. Some
            services will delete your data the minute your subscription lapses. ArtCraft will
            never do that.
          </>
        }
      />

      <SettingsRow
        title="Enable backups"
        description="Copy every finished generation from other services to the ArtCraft account below, along with its prompt."
      >
        <Switch enabled={backup.enabled} setEnabled={setEnabled} />
      </SettingsRow>

      <SettingsBlock
        title="Back up to"
        description={
          accounts.length === 0
            ? "Add an ArtCraft account under Accounts to choose where backups go."
            : "Only ArtCraft accounts can receive backups."
        }
      >
        <div
          role="radiogroup"
          aria-label="ArtCraft account for backups"
          aria-disabled={!backup.enabled}
          className={twMerge("flex flex-col gap-1.5", !backup.enabled && "opacity-50")}
        >
          {accounts.map((account) => {
            const selected = account.id === selectedId;
            const meta = getServiceMeta(account.service);
            const label = account.name || meta.label;
            const identity = [account.username, account.email].filter(Boolean).join(" · ");
            return (
              <button
                key={account.id}
                type="button"
                role="radio"
                aria-checked={selected}
                disabled={!backup.enabled}
                onClick={() => selectAccount(account.id)}
                className={twMerge(
                  "flex items-center gap-3 rounded-lg border px-3 py-2.5 text-left transition-colors",
                  backup.enabled ? "cursor-pointer" : "cursor-not-allowed",
                  selected
                    ? "border-bone/40 bg-ui-controls/60 text-bone"
                    : "border-ui-panel-border bg-ui-controls/20 text-base-fg/40",
                  backup.enabled && !selected && "hover:bg-ui-controls/40 hover:text-base-fg/70",
                )}
              >
                <span
                  aria-hidden="true"
                  className={twMerge(
                    "flex h-4 w-4 shrink-0 items-center justify-center rounded-full border",
                    selected ? "border-bone" : "border-current",
                  )}
                >
                  {selected && <span className="h-2 w-2 rounded-full bg-bone" />}
                </span>
                <img
                  src={getServiceLogoPath(account.service)}
                  alt=""
                  className={twMerge("h-5 w-5 shrink-0 object-contain icon-auto-contrast", !selected && "opacity-60")}
                />
                <span className="flex min-w-0 flex-col">
                  <span className="truncate text-sm font-medium">{label}</span>
                  {identity && <span className="truncate text-xs opacity-70">{identity}</span>}
                </span>
              </button>
            );
          })}
        </div>
        {selectionMissing && (
          <span className="text-xs text-amber-500">
            Backups are on, but no ArtCraft account is selected. Nothing will be backed up until you pick one.
          </span>
        )}
        {saveError && <span className="text-xs text-red-400">{saveError}</span>}
      </SettingsBlock>
    </div>
  );
};
