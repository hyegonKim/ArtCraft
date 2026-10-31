import { invoke } from "@tauri-apps/api/core";

export interface GetAppPreferencesResult {
  preferences: AppPreferencesPayload,
}

// Mirrors the backend's `AppPreferences` (grouped like the on-disk TOML).
export interface AppPreferencesPayload {
  sounds: AppSoundPreferences,
  downloads: AppDownloadPreferences,
  prompt: AppPromptPreferences,
  backup: AppBackupPreferences,
}

// Backing up generations made on other services (Higgsfield, Midjourney,
// Runway, ...) to one of the user's ArtCraft accounts.
export interface AppBackupPreferences {
  // Master switch. Off until the user opts in.
  enabled: boolean,
  // The ArtCraft credential id results are uploaded to. Kept while backups
  // are off so re-enabling restores the choice. Absent when never chosen.
  maybe_artcraft_credential_id?: string | null,
}

export interface AppPromptPreferences {
  // When on, Enter submits the prompt and Shift+Enter inserts a newline.
  // When off, both insert a newline and only the button submits.
  enter_to_generate: boolean,
}

export interface AppSoundPreferences {
  // Master switch: play sounds on events at all.
  play_sounds: boolean,

  // Which sound plays for each event. A catalog key (defined in the frontend
  // `SoundManager`), a custom .wav, or "none" for silent.
  delete_file: AppSoundFile,
  enqueue_success: AppSoundFile,
  enqueue_failure: AppSoundFile,
  generation_success: AppSoundFile,
  generation_failure: AppSoundFile,
}

// A sound catalog key (e.g. "done"), a custom .wav, or "none" (silent).
export type AppSoundFile = string | CustomWavSound;

export interface CustomWavSound {
  // Absolute path to the user's .wav file.
  custom_wav: string,
}

export const SILENT_SOUND = "none";

export const isCustomWavSound = (sound: AppSoundFile | undefined | null): sound is CustomWavSound =>
  typeof sound === "object" && sound !== null && typeof sound.custom_wav === "string";

export interface AppDownloadPreferences {
  // Preferred download directory
  preferred_download_directory: PreferredDownloadDirectory,

  // How downloaded generation files are named on disk.
  preferred_download_filename: PreferredDownloadFilename,
}

export type PreferredDownloadDirectory = SystemDirectory | CustomDirectory;

// "artcraft_convention" or a custom format string wrapper.
// Custom formats accept {model}, {date}, {YYYY}, {YY}, {MM}, {DD}, {HH},
// {mm}, {SS}, and {batch_index} tokens.
export type PreferredDownloadFilename = "artcraft_convention" | CustomFilenameFormat;

export interface CustomFilenameFormat {
  custom_format: string,
}

export interface SystemDirectory {
  // If the directory is a system directory.
  system: string,
}

export interface CustomDirectory {
  // If the directory is a custom user directory.
  custom: string,
}

export const GetAppPreferences = async () : Promise<GetAppPreferencesResult> => {
  let result = await invoke("get_app_preferences_command");
  return (result as GetAppPreferencesResult);
}
