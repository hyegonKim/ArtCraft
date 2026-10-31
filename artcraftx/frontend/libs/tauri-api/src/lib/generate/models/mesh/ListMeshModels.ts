import { invoke } from "@tauri-apps/api/core";
import { MeshModelListing, ListingProviderOffering } from "@storyteller/model-list";
import { CommandResult } from "../../../common/CommandStatus";

export interface ListMeshModelsSuccess extends CommandResult {
  payload: ListMeshModelsPayload;
}

export interface ListMeshModelsPayload {
  // Every mesh model in picker order, including disabled ones (`is_disabled`).
  models: MeshModelListing[];
  // Which providers offer which of those models (first listing = default).
  providers: ListingProviderOffering<MeshModelListing>[];
}

// List every mesh model the app knows about (`list_mesh_models_command`, served from the
// Rust `models` crate).
export const ListMeshModels = async (): Promise<ListMeshModelsSuccess> =>
  (await invoke("list_mesh_models_command")) as ListMeshModelsSuccess;
