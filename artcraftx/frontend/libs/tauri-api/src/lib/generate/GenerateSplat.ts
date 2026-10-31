import { invoke } from "@tauri-apps/api/core";
import { CommandResult } from "../common/CommandStatus";
import type { MediaSource } from "../common/MediaSource.js";

export interface GenerateSplatRequest {
  // Stable id (`credential_{entropy}`) of the stored credential (account)
  // to generate with. The backend loads it from disk and routes to that
  // credential's service.
  credential_id?: string;

  // The model to use (an omni model id, e.g. "marble_1p1").
  model?: string;

  // Text prompt.
  prompt?: string;

  reference_image_media_tokens?: string[];
  reference_video_media_token?: string;

  // Three-way media sources (bytes | local path | media token). Each wins
  // over its legacy token twin.
  reference_image_sources?: MediaSource[];
  reference_video_source?: MediaSource;

  is_panoramic?: boolean;
  disable_recaption?: boolean;


  // Frontend metadata.
  frontend_caller?: string;
  frontend_subscriber_id?: string;
  frontend_subscriber_payload?: string;
}

interface RawGenerateSplatRequest {
  credential_id?: string;
  model?: string;
  prompt?: string;
  reference_image_media_tokens?: string[];
  reference_video_media_token?: string;
  reference_image_sources?: MediaSource[];
  reference_video_source?: MediaSource;
  is_panoramic?: boolean;
  disable_recaption?: boolean;
  frontend_caller?: string;
  frontend_subscriber_id?: string;
  frontend_subscriber_payload?: string;
}

export enum GenerateSplatErrorType {
  ModelNotSpecified = "model_not_specified",
  ServerError = "server_error",
  // Problem with the selected account credential; the backend also flashes
  // a dismissable modal.
  CredentialProblem = "credential_problem",
}

export interface GenerateSplatError extends CommandResult {
  error_type: GenerateSplatErrorType;
  error_message?: string;
}

export interface GenerateSplatSuccess extends CommandResult {}

export type GenerateSplatResult = GenerateSplatSuccess | GenerateSplatError;

export const GenerateSplat = async (
  request: GenerateSplatRequest,
): Promise<GenerateSplatResult> => {
  const mutableRequest: RawGenerateSplatRequest = {};

  if (!!request.credential_id) mutableRequest.credential_id = request.credential_id;
  if (!!request.model) mutableRequest.model = request.model;
  if (!!request.prompt) mutableRequest.prompt = request.prompt;
  if (!!request.reference_image_media_tokens && request.reference_image_media_tokens.length > 0) {
    mutableRequest.reference_image_media_tokens = request.reference_image_media_tokens;
  }
  if (!!request.reference_video_media_token) mutableRequest.reference_video_media_token = request.reference_video_media_token;
  if (!!request.reference_image_sources && request.reference_image_sources.length > 0) {
    mutableRequest.reference_image_sources = request.reference_image_sources;
  }
  if (!!request.reference_video_source) mutableRequest.reference_video_source = request.reference_video_source;
  if (typeof request.is_panoramic === "boolean") mutableRequest.is_panoramic = request.is_panoramic;
  if (typeof request.disable_recaption === "boolean") mutableRequest.disable_recaption = request.disable_recaption;
  if (!!request.frontend_caller) mutableRequest.frontend_caller = request.frontend_caller;
  if (!!request.frontend_subscriber_id) {
    mutableRequest.frontend_subscriber_id = request.frontend_subscriber_id;
  }
  if (!!request.frontend_subscriber_payload) {
    mutableRequest.frontend_subscriber_payload = request.frontend_subscriber_payload;
  }

  const result = await invoke("generate_splat_command", {
    request: mutableRequest,
  });

  return result as GenerateSplatResult;
};
