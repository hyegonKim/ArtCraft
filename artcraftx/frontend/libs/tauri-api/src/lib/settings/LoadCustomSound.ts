import { invoke } from "@tauri-apps/api/core";
import { AppSoundEvent } from "./UpdateSoundPreference";

// The bytes of the custom .wav configured for an event. The backend reads the
// path from the saved preference, so only files picked in Settings are
// readable. Throws when the event has no custom sound or the file is missing
// (the backend logs a warning).
export const LoadCustomSound = async (event: AppSoundEvent) : Promise<ArrayBuffer> => {
  let result = await invoke("load_custom_sound_command", { event });
  return (result as ArrayBuffer);
}
