import { invoke } from "@tauri-apps/api/core";
import { CommandResult, CommandSuccessStatus } from "../common/CommandStatus";

// Subset of the omni `OmniGenSplatCostAndGenerateRequest` fields that affect
// pricing. `model` is an omni model id string so new server models never
// break this wrapper.
export interface EstimateSplatCostRequest {
  model: string;
  reference_image_media_tokens?: string[];
  reference_video_media_token?: string;
  is_panoramic?: boolean;
}

// Mirrors the Rust `OmniGenSplatCostResponse`.
export interface EstimateSplatCostPayload {
  success: boolean;
  cost_in_credits?: number;
  cost_in_usd_cents?: number;
  is_free: boolean;
  is_unlimited: boolean;
  is_rate_limited: boolean;
  has_watermark: boolean;
  failures_are_refunded?: boolean | null;
}

export interface EstimateSplatCostSuccess extends CommandResult {
  payload: EstimateSplatCostPayload;
}

export interface EstimateSplatCostErrorPayload {
  success: boolean;
  error_message: string;
}

export interface EstimateSplatCostErrorResult extends CommandResult {
  error_details?: EstimateSplatCostErrorPayload;
}

export type EstimateSplatCostResult =
  | EstimateSplatCostSuccess
  | EstimateSplatCostErrorResult;

export const EstimateSplatCost = async (
  request: EstimateSplatCostRequest,
): Promise<EstimateSplatCostResult> => {
  const result = await invoke("estimate_splat_cost_command", { request });
  return result as EstimateSplatCostResult;
};

export function isEstimateSplatCostSuccess(
  r: EstimateSplatCostResult,
): r is EstimateSplatCostSuccess {
  return r.status === CommandSuccessStatus.Success;
}
