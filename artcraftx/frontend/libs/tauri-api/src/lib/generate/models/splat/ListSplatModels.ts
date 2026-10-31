import { invoke } from "@tauri-apps/api/core";
import { SplatModelListing, ListingProviderOffering } from "@storyteller/model-list";
import { CommandResult } from "../../../common/CommandStatus";

export interface ListSplatModelsSuccess extends CommandResult {
  payload: ListSplatModelsPayload;
}

export interface ListSplatModelsPayload {
  // Every splat model in picker order, including disabled ones (`is_disabled`).
  models: SplatModelListing[];
  // Which providers offer which of those models (first listing = default).
  providers: ListingProviderOffering<SplatModelListing>[];
}

// List every splat model the app knows about (`list_splat_models_command`, served from the
// Rust `models` crate).
export const ListSplatModels = async (): Promise<ListSplatModelsSuccess> =>
  (await invoke("list_splat_models_command")) as ListSplatModelsSuccess;
