import { invoke } from "@tauri-apps/api/core";

// Mirrors `crates/artcraftx/src/state/promptbox`.

export type PromptboxModality = "image" | "video" | "mesh" | "splat" | "audio";

export interface ModalityPromptboxState {
  // Credential id the prompt box generates with (an account, not a provider).
  selected_account_id?: string;
  // Model id as served by the Rust `models` crate.
  selected_model?: string;
  // Model/mode-specific options; opaque to the backend, owned by the frontend.
  options: Record<string, unknown>;
}

export interface PromptboxState {
  version: number;
  image: ModalityPromptboxState;
  video: ModalityPromptboxState;
  mesh: ModalityPromptboxState;
  splat: ModalityPromptboxState;
  audio: ModalityPromptboxState;
  // model id -> credential id last used with it.
  last_account_by_model: Record<string, string>;
}

export interface GetPromptboxStateResult {
  state: PromptboxState;
}

// Everything the prompt boxes remembered from the last run. Hydrate from it,
// dropping anything that no longer exists.
export const GetPromptboxState = async (): Promise<GetPromptboxStateResult> =>
  (await invoke("get_promptbox_state_command")) as GetPromptboxStateResult;

// The full state to persist. The backend replaces what it has wholesale and
// owns `version`, so the frontend never sends one.
export type UpdatePromptboxStateRequest = Omit<PromptboxState, "version">;

export interface UpdatePromptboxStateResult {
  state: PromptboxState;
}

export const UpdatePromptboxState = async (state: UpdatePromptboxStateRequest): Promise<UpdatePromptboxStateResult> =>
  (await invoke("update_promptbox_state_command", { state })) as UpdatePromptboxStateResult;
