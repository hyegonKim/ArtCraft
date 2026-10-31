//! Typed vocabulary shared across endpoints: closed-set enums for request
//! parameters (with an `Other` escape hatch when parsing responses) and the
//! response structures several endpoints return.

pub(crate) mod string_enum;

pub mod enqueue_jobs_response;
pub mod ids;
pub mod image_aspect_ratio;
pub mod image_batch_size;
pub mod image_dimensions;
pub mod image_quality;
pub mod image_resolution;
pub mod image_seed;
pub mod job_media;
pub mod job_params;
pub mod job_set_type;
pub mod job_status;
pub mod media_input;
pub mod media_mime_type;
pub mod media_reference;
pub mod media_role;
pub mod nano_banana_aspect_ratio;
pub mod presigned_media_upload;
pub mod seedream_aspect_ratio;
pub mod thinking_level;
pub mod video_aspect_ratio;
pub mod video_bitrate_mode;
pub mod video_dimensions;
pub mod video_duration;
pub mod video_mode;
pub mod video_resolution;
pub mod wallet;
