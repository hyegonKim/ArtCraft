import { invoke } from "@tauri-apps/api/core";
import { AppBackupPreferences } from "./GetAppPreferences";

export enum BackupPreferenceName {
  Enabled = "enabled",
  ArtcraftCredentialId = "artcraft_credential_id",
}

export type UpdateBackupPreferenceRequest =
  | { preference: BackupPreferenceName.Enabled, value: boolean }
  // `null` clears the selection. The backend rejects ids that aren't an
  // ArtCraft web-session credential.
  | { preference: BackupPreferenceName.ArtcraftCredentialId, value: string | null };

export interface UpdateBackupPreferenceResult {
  // The full backup preferences after the change.
  backup: AppBackupPreferences,
}

// Change one Account Backup preference.
export const UpdateBackupPreference = async (request: UpdateBackupPreferenceRequest) : Promise<UpdateBackupPreferenceResult> => {
  let result = await invoke("update_backup_preference_command", {
    request: {
      preference: request.preference,
      value: request.value,
    }
  });
  return (result as UpdateBackupPreferenceResult);
}
