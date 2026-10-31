import { invoke } from "@tauri-apps/api/core";
import { CommandResult } from "../common/CommandStatus";
import type { MediaSource } from "../common/MediaSource.js";

export interface GenerateMeshRequest {
  // Stable id (`credential_{entropy}`) of the stored credential (account)
  // to generate with. The backend loads it from disk and routes to that
  // credential's service.
  credential_id?: string;

  // The model to use (an omni model id, e.g. "hunyuan_3d_3").
  model?: string;

  // Text prompt.
  prompt?: string;

  reference_image_media_tokens?: string[];

  // Three-way reference images (bytes | local path | media token). Wins
  // over `reference_image_media_tokens`.
  reference_image_sources?: MediaSource[];

  front_image_media_token?: string;
  back_image_media_token?: string;
  left_image_media_token?: string;
  right_image_media_token?: string;
  input_mesh_media_token?: string;
  mesh_output_type?: string;
  polygon_type?: string;
  face_count?: number;
  enable_pbr?: boolean;
  enable_texture?: boolean;
  texture_quality?: string;
  geometry_quality?: string;


  // Frontend metadata.
  frontend_caller?: string;
  frontend_subscriber_id?: string;
  frontend_subscriber_payload?: string;
}

interface RawGenerateMeshRequest {
  credential_id?: string;
  model?: string;
  prompt?: string;
  reference_image_media_tokens?: string[];
  reference_image_sources?: MediaSource[];
  front_image_media_token?: string;
  back_image_media_token?: string;
  left_image_media_token?: string;
  right_image_media_token?: string;
  input_mesh_media_token?: string;
  mesh_output_type?: string;
  polygon_type?: string;
  face_count?: number;
  enable_pbr?: boolean;
  enable_texture?: boolean;
  texture_quality?: string;
  geometry_quality?: string;
  frontend_caller?: string;
  frontend_subscriber_id?: string;
  frontend_subscriber_payload?: string;
}

export enum GenerateMeshErrorType {
  ModelNotSpecified = "model_not_specified",
  ServerError = "server_error",
  // Problem with the selected account credential; the backend also flashes
  // a dismissable modal.
  CredentialProblem = "credential_problem",
}

export interface GenerateMeshError extends CommandResult {
  error_type: GenerateMeshErrorType;
  error_message?: string;
}

export interface GenerateMeshSuccess extends CommandResult {}

export type GenerateMeshResult = GenerateMeshSuccess | GenerateMeshError;

export const GenerateMesh = async (
  request: GenerateMeshRequest,
): Promise<GenerateMeshResult> => {
  const mutableRequest: RawGenerateMeshRequest = {};

  if (!!request.credential_id) mutableRequest.credential_id = request.credential_id;
  if (!!request.model) mutableRequest.model = request.model;
  if (!!request.prompt) mutableRequest.prompt = request.prompt;
  if (!!request.reference_image_media_tokens && request.reference_image_media_tokens.length > 0) {
    mutableRequest.reference_image_media_tokens = request.reference_image_media_tokens;
  }
  if (!!request.reference_image_sources && request.reference_image_sources.length > 0) {
    mutableRequest.reference_image_sources = request.reference_image_sources;
  }
  if (!!request.front_image_media_token) mutableRequest.front_image_media_token = request.front_image_media_token;
  if (!!request.back_image_media_token) mutableRequest.back_image_media_token = request.back_image_media_token;
  if (!!request.left_image_media_token) mutableRequest.left_image_media_token = request.left_image_media_token;
  if (!!request.right_image_media_token) mutableRequest.right_image_media_token = request.right_image_media_token;
  if (!!request.input_mesh_media_token) mutableRequest.input_mesh_media_token = request.input_mesh_media_token;
  if (!!request.mesh_output_type) mutableRequest.mesh_output_type = request.mesh_output_type;
  if (!!request.polygon_type) mutableRequest.polygon_type = request.polygon_type;
  if (typeof request.face_count === "number") mutableRequest.face_count = request.face_count;
  if (typeof request.enable_pbr === "boolean") mutableRequest.enable_pbr = request.enable_pbr;
  if (typeof request.enable_texture === "boolean") mutableRequest.enable_texture = request.enable_texture;
  if (!!request.texture_quality) mutableRequest.texture_quality = request.texture_quality;
  if (!!request.geometry_quality) mutableRequest.geometry_quality = request.geometry_quality;
  if (!!request.frontend_caller) mutableRequest.frontend_caller = request.frontend_caller;
  if (!!request.frontend_subscriber_id) {
    mutableRequest.frontend_subscriber_id = request.frontend_subscriber_id;
  }
  if (!!request.frontend_subscriber_payload) {
    mutableRequest.frontend_subscriber_payload = request.frontend_subscriber_payload;
  }

  const result = await invoke("generate_mesh_command", {
    request: mutableRequest,
  });

  return result as GenerateMeshResult;
};
