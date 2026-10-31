import { invoke } from "@tauri-apps/api/core";
import { CommandResult, CommandSuccessStatus } from "../common/CommandStatus";

// Subset of the omni `OmniGenAudioCostAndGenerateRequest` fields that affect
// pricing. `model` is an omni model id string so new server models never
// break this wrapper.
export interface EstimateAudioCostRequest {
  model: string;
  audio_media_tokens?: string[];
  image_media_tokens?: string[];
  sample_rate_hz?: number;
}

// Mirrors the Rust `OmniGenAudioCostResponse`.
export interface EstimateAudioCostPayload {
  success: boolean;
  cost_in_credits?: number;
  cost_in_usd_cents?: number;
  is_free: boolean;
  is_unlimited: boolean;
  is_rate_limited: boolean;
  has_watermark: boolean;
  failures_are_refunded?: boolean | null;
}

export interface EstimateAudioCostSuccess extends CommandResult {
  payload: EstimateAudioCostPayload;
}

export interface EstimateAudioCostErrorPayload {
  success: boolean;
  error_message: string;
}

export interface EstimateAudioCostErrorResult extends CommandResult {
  error_details?: EstimateAudioCostErrorPayload;
}

export type EstimateAudioCostResult =
  | EstimateAudioCostSuccess
  | EstimateAudioCostErrorResult;

export const EstimateAudioCost = async (
  request: EstimateAudioCostRequest,
): Promise<EstimateAudioCostResult> => {
  const result = await invoke("estimate_audio_cost_command", { request });
  return result as EstimateAudioCostResult;
};

export function isEstimateAudioCostSuccess(
  r: EstimateAudioCostResult,
): r is EstimateAudioCostSuccess {
  return r.status === CommandSuccessStatus.Success;
}
