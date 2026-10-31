import { invoke } from "@tauri-apps/api/core";
import { CommandResult, CommandSuccessStatus } from "../common/CommandStatus";

// Subset of the omni `OmniGenMeshCostAndGenerateRequest` fields that affect
// pricing. `model` is an omni model id string so new server models never
// break this wrapper.
export interface EstimateMeshCostRequest {
  model: string;
  reference_image_media_tokens?: string[];
  mesh_output_type?: string;
  polygon_type?: string;
  enable_pbr?: boolean;
  enable_texture?: boolean;
  texture_quality?: string;
  geometry_quality?: string;
}

// Mirrors the Rust `OmniGenMeshCostResponse`.
export interface EstimateMeshCostPayload {
  success: boolean;
  cost_in_credits?: number;
  cost_in_usd_cents?: number;
  is_free: boolean;
  is_unlimited: boolean;
  is_rate_limited: boolean;
  has_watermark: boolean;
  failures_are_refunded?: boolean | null;
}

export interface EstimateMeshCostSuccess extends CommandResult {
  payload: EstimateMeshCostPayload;
}

export interface EstimateMeshCostErrorPayload {
  success: boolean;
  error_message: string;
}

export interface EstimateMeshCostErrorResult extends CommandResult {
  error_details?: EstimateMeshCostErrorPayload;
}

export type EstimateMeshCostResult =
  | EstimateMeshCostSuccess
  | EstimateMeshCostErrorResult;

export const EstimateMeshCost = async (
  request: EstimateMeshCostRequest,
): Promise<EstimateMeshCostResult> => {
  const result = await invoke("estimate_mesh_cost_command", { request });
  return result as EstimateMeshCostResult;
};

export function isEstimateMeshCostSuccess(
  r: EstimateMeshCostResult,
): r is EstimateMeshCostSuccess {
  return r.status === CommandSuccessStatus.Success;
}
