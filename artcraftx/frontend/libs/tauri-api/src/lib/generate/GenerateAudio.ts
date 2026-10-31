import { invoke } from "@tauri-apps/api/core";
import { CommandResult } from "../common/CommandStatus";
import type { MediaSource } from "../common/MediaSource.js";

export interface GenerateAudioRequest {
  // Stable id (`credential_{entropy}`) of the stored credential (account)
  // to generate with. The backend loads it from disk and routes to that
  // credential's service.
  credential_id?: string;

  // The model to use (an omni model id, e.g. "suno_music").
  model?: string;

  // Text prompt.
  prompt?: string;

  style_prompt?: string;
  audio_media_tokens?: string[];
  image_media_tokens?: string[];

  // Three-way media sources (bytes | local path | media token). Each wins
  // over its legacy token twin.
  audio_sources?: MediaSource[];
  image_sources?: MediaSource[];

  keep_lyrics?: boolean;
  is_instrumental?: boolean;
  is_loopable?: boolean;
  bpm?: number;
  musical_key?: string;
  sample_rate_hz?: number;
  speed?: number;
  volume?: number;
  pitch?: number;


  // Frontend metadata.
  frontend_caller?: string;
  frontend_subscriber_id?: string;
  frontend_subscriber_payload?: string;
}

interface RawGenerateAudioRequest {
  credential_id?: string;
  model?: string;
  prompt?: string;
  style_prompt?: string;
  audio_media_tokens?: string[];
  image_media_tokens?: string[];
  audio_sources?: MediaSource[];
  image_sources?: MediaSource[];
  keep_lyrics?: boolean;
  is_instrumental?: boolean;
  is_loopable?: boolean;
  bpm?: number;
  musical_key?: string;
  sample_rate_hz?: number;
  speed?: number;
  volume?: number;
  pitch?: number;
  frontend_caller?: string;
  frontend_subscriber_id?: string;
  frontend_subscriber_payload?: string;
}

export enum GenerateAudioErrorType {
  ModelNotSpecified = "model_not_specified",
  ServerError = "server_error",
  // Problem with the selected account credential; the backend also flashes
  // a dismissable modal.
  CredentialProblem = "credential_problem",
}

export interface GenerateAudioError extends CommandResult {
  error_type: GenerateAudioErrorType;
  error_message?: string;
}

export interface GenerateAudioSuccess extends CommandResult {}

export type GenerateAudioResult = GenerateAudioSuccess | GenerateAudioError;

export const GenerateAudio = async (
  request: GenerateAudioRequest,
): Promise<GenerateAudioResult> => {
  const mutableRequest: RawGenerateAudioRequest = {};

  if (!!request.credential_id) mutableRequest.credential_id = request.credential_id;
  if (!!request.model) mutableRequest.model = request.model;
  if (!!request.prompt) mutableRequest.prompt = request.prompt;
  if (!!request.style_prompt) mutableRequest.style_prompt = request.style_prompt;
  if (!!request.audio_media_tokens && request.audio_media_tokens.length > 0) {
    mutableRequest.audio_media_tokens = request.audio_media_tokens;
  }
  if (!!request.image_media_tokens && request.image_media_tokens.length > 0) {
    mutableRequest.image_media_tokens = request.image_media_tokens;
  }
  if (!!request.audio_sources && request.audio_sources.length > 0) {
    mutableRequest.audio_sources = request.audio_sources;
  }
  if (!!request.image_sources && request.image_sources.length > 0) {
    mutableRequest.image_sources = request.image_sources;
  }
  if (typeof request.keep_lyrics === "boolean") mutableRequest.keep_lyrics = request.keep_lyrics;
  if (typeof request.is_instrumental === "boolean") mutableRequest.is_instrumental = request.is_instrumental;
  if (typeof request.is_loopable === "boolean") mutableRequest.is_loopable = request.is_loopable;
  if (typeof request.bpm === "number") mutableRequest.bpm = request.bpm;
  if (!!request.musical_key) mutableRequest.musical_key = request.musical_key;
  if (typeof request.sample_rate_hz === "number") mutableRequest.sample_rate_hz = request.sample_rate_hz;
  if (typeof request.speed === "number") mutableRequest.speed = request.speed;
  if (typeof request.volume === "number") mutableRequest.volume = request.volume;
  if (typeof request.pitch === "number") mutableRequest.pitch = request.pitch;
  if (!!request.frontend_caller) mutableRequest.frontend_caller = request.frontend_caller;
  if (!!request.frontend_subscriber_id) {
    mutableRequest.frontend_subscriber_id = request.frontend_subscriber_id;
  }
  if (!!request.frontend_subscriber_payload) {
    mutableRequest.frontend_subscriber_payload = request.frontend_subscriber_payload;
  }

  const result = await invoke("generate_audio_command", {
    request: mutableRequest,
  });

  return result as GenerateAudioResult;
};
