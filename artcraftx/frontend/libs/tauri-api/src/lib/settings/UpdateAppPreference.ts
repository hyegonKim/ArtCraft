
import { invoke } from "@tauri-apps/api/core";
import { PreferredDownloadDirectory, PreferredDownloadFilename } from "./GetAppPreferences";

// One preference change. Per-event sounds go through UpdateSoundPreference.
export interface UpdateAppPreferencesRequest {
  preference: PreferenceName,
  value: boolean | PreferredDownloadDirectory | PreferredDownloadFilename,
}

export enum PreferenceName {
  PreferredDownloadDirectory = "preferred_download_directory",
  PreferredDownloadFilename = "preferred_download_filename",
  PlaySounds = "play_sounds",
}

export interface UpdateAppPreferencesResult {
  success: boolean
}

export const UpdateAppPreferences = async (request: UpdateAppPreferencesRequest) : Promise<UpdateAppPreferencesResult> => {
  let result = await invoke("update_app_preferences_command", { 
    request: {
      preference: request.preference,
      value: request.value,
    }
  });
  return (result as UpdateAppPreferencesResult);
}
