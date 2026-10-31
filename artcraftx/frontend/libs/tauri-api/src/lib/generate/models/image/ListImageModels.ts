import { invoke } from "@tauri-apps/api/core";
import { ImageModelListing, ListingProviderOffering } from "@storyteller/model-list";
import { CommandResult } from "../../../common/CommandStatus";

export interface ListImageModelsSuccess extends CommandResult {
  payload: ListImageModelsPayload;
}

export interface ListImageModelsPayload {
  // Every image model in picker order, including disabled ones (`is_disabled`).
  models: ImageModelListing[];
  // Which providers offer which of those models (first listing = default).
  providers: ListingProviderOffering<ImageModelListing>[];
}

// List every image model the app knows about (`list_image_models_command`, served from the
// Rust `models` crate).
export const ListImageModels = async (): Promise<ListImageModelsSuccess> =>
  (await invoke("list_image_models_command")) as ListImageModelsSuccess;
