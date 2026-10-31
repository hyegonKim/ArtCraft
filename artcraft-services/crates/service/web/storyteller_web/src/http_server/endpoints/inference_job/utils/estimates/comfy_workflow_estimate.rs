use std::cmp::max;

use mysql_queries::payloads::generic_inference_args::generic_inference_args::PolymorphicInferenceArgs;
use mysql_queries::payloads::generic_inference_args::inner_payloads::workflow_payload::WorkflowArgs;

use crate::http_server::endpoints::inference_job::utils::estimates::percent::percent;
use crate::http_server::endpoints::inference_job::utils::extractors::extract_comfy_workflow_args::extract_comfy_workflow_args;

const COMFY_JOB_DEFAULT_VIDEO_LENGTH_SECONDS : u64 = 3;

// TODO: These numbers are made up. We should measure the average job durations.
const COMFY_JOB_AVERAGE_EXECUTION_SECONDS_PER_SECOND: u64 = 40;
const COMFY_JOB_AVERAGE_FACE_DETAILER_EXECUTION_SECONDS_PER_SECOND: u64 = 40;
const COMFY_JOB_AVERAGE_FACE_FUSION_EXECUTION_SECONDS_PER_SECOND: u64 = 40;

// TODO: These numbers are made up. We should measure the average job durations.
const COMFY_FALLBACK_AVERAGE_JOB_DURATION_SECONDS: u64 = 60 * 5;

pub fn comfy_workflow_estimate(maybe_args: Option<&PolymorphicInferenceArgs>, job_duration_seconds: u64) -> u8 {
  let args = match maybe_args {
    Some(args) => args,
    None => return percent(job_duration_seconds, COMFY_FALLBACK_AVERAGE_JOB_DURATION_SECONDS),
  };

  let args = match extract_comfy_workflow_args(args) {
    Some(args) => args,
    None => return percent(job_duration_seconds, COMFY_FALLBACK_AVERAGE_JOB_DURATION_SECONDS),
  };

  let video_length_seconds = comfy_video_length(&args);

  // TODO: Better estimate
  let mut estimated_job_duration_seconds =
      COMFY_JOB_AVERAGE_EXECUTION_SECONDS_PER_SECOND * video_length_seconds;

  // TODO: Better estimate
  if args.use_face_detailer.unwrap_or(false) {
    estimated_job_duration_seconds +=
        video_length_seconds * COMFY_JOB_AVERAGE_FACE_DETAILER_EXECUTION_SECONDS_PER_SECOND;
  }

  // TODO: Better estimate
  if args.lipsync_enabled.unwrap_or(false) || args.enable_lipsync.unwrap_or(false) {
    estimated_job_duration_seconds +=
        video_length_seconds * COMFY_JOB_AVERAGE_FACE_FUSION_EXECUTION_SECONDS_PER_SECOND;
  }

  percent(job_duration_seconds, estimated_job_duration_seconds)
}

fn comfy_video_length(args: &WorkflowArgs) -> u64 {
  let trim_start_millis = args.trim_start_milliseconds
      .or_else(|| args.trim_start_seconds.map(|s| s as u64 * 1_000))
      .unwrap_or(0);

  let trim_end_millis = args.trim_end_milliseconds
      .or_else(|| args.trim_end_seconds.map(|s| s as u64 * 1_000))
      .unwrap_or(3_000);

  let mut video_length_seconds = COMFY_JOB_DEFAULT_VIDEO_LENGTH_SECONDS;

  if trim_end_millis > trim_start_millis {
    let video_length_millis = trim_end_millis - trim_start_millis;
    video_length_seconds = video_length_millis / 1000;
    video_length_seconds = max(video_length_seconds, COMFY_JOB_DEFAULT_VIDEO_LENGTH_SECONDS);
  }

  video_length_seconds
}
