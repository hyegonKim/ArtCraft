import { invoke } from "@tauri-apps/api/core";
import { AppPromptPreferences } from "./GetAppPreferences";

export enum PromptPreferenceName {
  EnterToGenerate = "enter_to_generate",
}

export interface UpdatePromptPreferenceRequest {
  preference: PromptPreferenceName,
  value: boolean,
}

export interface UpdatePromptPreferenceResult {
  // The full prompt preferences after the change.
  prompt: AppPromptPreferences,
}

// Change one prompt-box preference.
export const UpdatePromptPreference = async (request: UpdatePromptPreferenceRequest) : Promise<UpdatePromptPreferenceResult> => {
  let result = await invoke("update_prompt_preference_command", {
    request: {
      preference: request.preference,
      value: request.value,
    }
  });
  return (result as UpdatePromptPreferenceResult);
}
