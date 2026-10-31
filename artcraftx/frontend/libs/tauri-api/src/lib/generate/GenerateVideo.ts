import { invoke } from "@tauri-apps/api/core";
import { CommandResult } from "../common/CommandStatus";
import {
  CommonAspectRatio,
  CommonResolution,
  VideoModel,
} from "@storyteller/model-list";
import { GenerationProvider } from "@storyteller/api-enums";
import type { MediaSource } from "../common/MediaSource.js";

export interface GenerateVideoRequest {
  // Stable id (`credential_{entropy}`) of the stored credential (account)
  // to generate with. The backend loads it from disk and routes to that
  // credential's service.
  credential_id?: string;

  // The provider to use (defaults to Artcraft/Storyteller).
  provider?: GenerationProvider;

  // The model to use.
  model?: VideoModel | string;

  // Text prompt.
  prompt?: string;

  // Negative prompt.
  negative_prompt?: string;

  // Starting frame.
  start_frame_image_media_token?: string;

  // Ending frame.
  end_frame_image_media_token?: string;

  // Reference media tokens.
  reference_image_media_tokens?: string[];
  reference_video_media_tokens?: string[];
  reference_audio_media_tokens?: string[];
  reference_character_tokens?: string[];

  // Three-way media sources (bytes | local path | media token). Each wins
  // over its legacy `*_media_token(s)` twin; local files never touch the
  // ArtCraft cloud unless the target provider requires it.
  start_frame_image_source?: MediaSource;
  end_frame_image_source?: MediaSource;
  reference_image_sources?: MediaSource[];
  reference_video_sources?: MediaSource[];
  reference_audio_sources?: MediaSource[];

  aspect_ratio?: CommonAspectRatio;
  resolution?: CommonResolution;

  duration_seconds?: number;
  generate_audio?: boolean;
  video_batch_count?: number;

  // Deprecated on the Rust side (still read by some legacy handlers).
  sora_orientation?: "portrait" | "landscape";
  grok_aspect_ratio?: "portrait" | "landscape" | "square";

  // Frontend metadata.
  frontend_caller?: string;
  frontend_subscriber_id?: string;
  frontend_subscriber_payload?: string;
}

interface RawGenerateVideoRequest {
  credential_id?: string;
  provider?: GenerationProvider;
  model?: string;
  prompt?: string;
  negative_prompt?: string;
  start_frame_image_media_token?: string;
  end_frame_image_media_token?: string;
  reference_image_media_tokens?: string[];
  reference_video_media_tokens?: string[];
  reference_audio_media_tokens?: string[];
  reference_character_tokens?: string[];
  start_frame_image_source?: MediaSource;
  end_frame_image_source?: MediaSource;
  reference_image_sources?: MediaSource[];
  reference_video_sources?: MediaSource[];
  reference_audio_sources?: MediaSource[];
  aspect_ratio?: CommonAspectRatio;
  resolution?: CommonResolution;
  duration_seconds?: number;
  generate_audio?: boolean;
  video_batch_count?: number;
  sora_orientation?: "portrait" | "landscape";
  grok_aspect_ratio?: "portrait" | "landscape" | "square";
  frontend_caller?: string;
  frontend_subscriber_id?: string;
  frontend_subscriber_payload?: string;
}

export enum GenerateVideoErrorType {
  ModelNotSpecified = "model_not_specified",
  NoProviderAvailable = "no_provider_available",
  ServerError = "server_error",
  NeedsFalApiKey = "needs_fal_api_key",
  FalError = "fal_error",
  NeedsStorytellerCredentials = "needs_storyteller_credentials",
}

export interface GenerateVideoError extends CommandResult {
  error_type: GenerateVideoErrorType;
  error_message?: string;
}

export type GenerateVideoPayload = Record<string, never>;

export interface GenerateVideoSuccess extends CommandResult {
  payload: GenerateVideoPayload;
}

export type GenerateVideoResult = GenerateVideoSuccess | GenerateVideoError;

export const GenerateVideo = async (
  request: GenerateVideoRequest,
): Promise<GenerateVideoResult> => {
  let modelName: string | undefined;

  if (!!request.model) {
    if (typeof request.model === "string") {
      modelName = request.model;
    } else if (typeof request.model.tauriId === "string") {
      modelName = request.model.tauriId;
    }
  }

  if (!modelName) {
    throw new Error(
      "No model specified in request: " + JSON.stringify(request),
    );
  }

  const mutableRequest: RawGenerateVideoRequest = {
    model: modelName,
  };

  if (!!request.credential_id) mutableRequest.credential_id = request.credential_id;
  if (!!request.provider) mutableRequest.provider = request.provider;
  if (!!request.prompt) mutableRequest.prompt = request.prompt;
  if (!!request.negative_prompt) mutableRequest.negative_prompt = request.negative_prompt;
  if (!!request.start_frame_image_media_token) {
    mutableRequest.start_frame_image_media_token = request.start_frame_image_media_token;
  }
  if (!!request.end_frame_image_media_token) {
    mutableRequest.end_frame_image_media_token = request.end_frame_image_media_token;
  }
  if (!!request.reference_image_media_tokens && request.reference_image_media_tokens.length > 0) {
    mutableRequest.reference_image_media_tokens = request.reference_image_media_tokens;
  }
  if (!!request.reference_video_media_tokens && request.reference_video_media_tokens.length > 0) {
    mutableRequest.reference_video_media_tokens = request.reference_video_media_tokens;
  }
  if (!!request.reference_audio_media_tokens && request.reference_audio_media_tokens.length > 0) {
    mutableRequest.reference_audio_media_tokens = request.reference_audio_media_tokens;
  }
  if (!!request.reference_character_tokens && request.reference_character_tokens.length > 0) {
    mutableRequest.reference_character_tokens = request.reference_character_tokens;
  }
  if (!!request.start_frame_image_source) {
    mutableRequest.start_frame_image_source = request.start_frame_image_source;
  }
  if (!!request.end_frame_image_source) {
    mutableRequest.end_frame_image_source = request.end_frame_image_source;
  }
  if (!!request.reference_image_sources && request.reference_image_sources.length > 0) {
    mutableRequest.reference_image_sources = request.reference_image_sources;
  }
  if (!!request.reference_video_sources && request.reference_video_sources.length > 0) {
    mutableRequest.reference_video_sources = request.reference_video_sources;
  }
  if (!!request.reference_audio_sources && request.reference_audio_sources.length > 0) {
    mutableRequest.reference_audio_sources = request.reference_audio_sources;
  }
  if (!!request.aspect_ratio) mutableRequest.aspect_ratio = request.aspect_ratio;
  if (!!request.resolution) mutableRequest.resolution = request.resolution;
  if (typeof request.duration_seconds === "number") {
    mutableRequest.duration_seconds = request.duration_seconds;
  }
  if (typeof request.generate_audio === "boolean") {
    mutableRequest.generate_audio = request.generate_audio;
  }
  if (typeof request.video_batch_count === "number") {
    mutableRequest.video_batch_count = request.video_batch_count;
  }
  if (!!request.sora_orientation) mutableRequest.sora_orientation = request.sora_orientation;
  if (!!request.grok_aspect_ratio) mutableRequest.grok_aspect_ratio = request.grok_aspect_ratio;
  if (!!request.frontend_caller) mutableRequest.frontend_caller = request.frontend_caller;
  if (!!request.frontend_subscriber_id) {
    mutableRequest.frontend_subscriber_id = request.frontend_subscriber_id;
  }
  if (!!request.frontend_subscriber_payload) {
    mutableRequest.frontend_subscriber_payload = request.frontend_subscriber_payload;
  }

  const result = await invoke("generate_video_command", {
    request: mutableRequest,
  });

  return result as GenerateVideoResult;
};
