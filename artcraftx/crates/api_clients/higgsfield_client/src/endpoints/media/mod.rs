//! Uploading reference files (images, video, audio) for generation requests.
//!
//! An upload is three calls, mirroring the web app, with a presign +
//! confirm pair per file family:
//!
//! | Family | Presign                                   | Confirm                      | Descriptor `type` |
//! |--------|-------------------------------------------|------------------------------|-------------------|
//! | image  | [`create_reference_media`] (one) or [`create_media_batch`] (many) | [`confirm_media_upload`] | `media_input` |
//! | video  | [`create_video_upload`]                   | [`confirm_video_upload`]     | `video_input`     |
//! | audio  | [`create_audio_upload`]                   | [`confirm_audio_upload`]     | `audio_input`     |
//!
//! 1. Presign: the gateway allocates a [`MediaId`] and returns a CDN URL
//!    plus a presigned storage URL.
//! 2. [`upload_media_bytes`] — `PUT` the raw bytes to the storage URL.
//! 3. Confirm — tell the gateway the bytes are there (and kick off its
//!    NSFW / IP checks).
//!
//! Generation requests then reference the file by its
//! [`MediaInput`](crate::types::media_input::MediaInput) descriptor
//! (`{id, type, url}`) — never the bytes.
//! `HiggsfieldSession::upload_reference_media` does all three in one call
//! for any family.
//!
//! Some models (Seedance 2.x) refuse media whose intellectual-property
//! check hasn't run (`400 "IP check not finished for input media"`):
//! confirm with `force_ip_check: true` and poll [`get_media_status`] until
//! `ip_check_finished` — or set `ReferenceMediaFile::with_ip_check` and let
//! the session do it. [`get_input_media`] looks a descriptor up by id.
//!
//! [`get_media_status`]: get_media_status::get_media_status
//! [`get_input_media`]: get_input_media::get_input_media
//!
//! [`MediaId`]: crate::types::ids::MediaId
//! [`create_reference_media`]: create_reference_media::create_reference_media
//! [`create_media_batch`]: create_media_batch::create_media_batch
//! [`create_video_upload`]: create_video_upload::create_video_upload
//! [`create_audio_upload`]: create_audio_upload::create_audio_upload
//! [`upload_media_bytes`]: upload_media_bytes::upload_media_bytes
//! [`confirm_media_upload`]: confirm_media_upload::confirm_media_upload
//! [`confirm_video_upload`]: confirm_video_upload::confirm_video_upload
//! [`confirm_audio_upload`]: confirm_audio_upload::confirm_audio_upload

pub mod confirm_audio_upload;
pub mod confirm_media_upload;
pub mod confirm_video_upload;
pub mod create_audio_upload;
pub mod create_media_batch;
pub mod create_reference_media;
pub mod create_video_upload;
pub mod get_input_media;
pub mod get_media_status;
pub mod upload_media_bytes;
