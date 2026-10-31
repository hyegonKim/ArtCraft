import { invoke } from "@tauri-apps/api/core";
import { AudioModelListing, ListingProviderOffering } from "@storyteller/model-list";
import { CommandResult } from "../../../common/CommandStatus";

export interface ListAudioModelsSuccess extends CommandResult {
  payload: ListAudioModelsPayload;
}

export interface ListAudioModelsPayload {
  // Every audio model in picker order, including disabled ones (`is_disabled`).
  models: AudioModelListing[];
  // Which providers offer which of those models (first listing = default).
  providers: ListingProviderOffering<AudioModelListing>[];
}

// List every audio model the app knows about (`list_audio_models_command`, served from the
// Rust `models` crate).
export const ListAudioModels = async (): Promise<ListAudioModelsSuccess> =>
  (await invoke("list_audio_models_command")) as ListAudioModelsSuccess;
