import { invoke } from "@tauri-apps/api/core";
import { VideoModelListing, ListingProviderOffering } from "@storyteller/model-list";
import { CommandResult } from "../../../common/CommandStatus";

export interface ListVideoModelsSuccess extends CommandResult {
  payload: ListVideoModelsPayload;
}

export interface ListVideoModelsPayload {
  // Every video model in picker order, including disabled ones (`is_disabled`).
  models: VideoModelListing[];
  // Which providers offer which of those models (first listing = default).
  providers: ListingProviderOffering<VideoModelListing>[];
}

// List every video model the app knows about (`list_video_models_command`, served from the
// Rust `models` crate).
export const ListVideoModels = async (): Promise<ListVideoModelsSuccess> =>
  (await invoke("list_video_models_command")) as ListVideoModelsSuccess;
