import { invoke } from "@tauri-apps/api/core";
import { AppSoundFile, AppSoundPreferences } from "./GetAppPreferences";

// The app events that can play a sound (one per field of AppSoundPreferences).
export type AppSoundEvent =
  | "delete_file"
  | "enqueue_success"
  | "enqueue_failure"
  | "generation_success"
  | "generation_failure";

export interface UpdateSoundPreferenceRequest {
  event: AppSoundEvent,
  // A catalog key, a custom .wav, or undefined / "none" for silent.
  sound: AppSoundFile | undefined,
}

export interface UpdateSoundPreferenceResult {
  // The full sound preferences after the change.
  sounds: AppSoundPreferences,
}

// Change the sound for one event. Rejects (throws the reason) when a custom
// file doesn't exist or isn't a .wav.
export const UpdateSoundPreference = async (request: UpdateSoundPreferenceRequest) : Promise<UpdateSoundPreferenceResult> => {
  let result = await invoke("update_sound_preference_command", {
    request: {
      action: "set",
      event: request.event,
      sound: request.sound,
    }
  });
  return (result as UpdateSoundPreferenceResult);
}

// Put one event's sound back to the app default (defined on the backend).
export const ResetSoundPreference = async (event: AppSoundEvent) : Promise<UpdateSoundPreferenceResult> => {
  let result = await invoke("update_sound_preference_command", {
    request: {
      action: "reset_to_default",
      event,
    }
  });
  return (result as UpdateSoundPreferenceResult);
}
