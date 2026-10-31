//! media
//!
//! The purpose of this library is to package audio/video related functions together until they need
//! to be split apart.
//!

// Never allow these
#![forbid(private_bounds)]
#![forbid(private_interfaces)]
#![forbid(unused_must_use)] // NB: It's unsafe to not close/check some things

// Okay to toggle
#![forbid(unreachable_patterns)]
#![forbid(unused_imports)]
#![forbid(unused_mut)]
#![forbid(unused_variables)]

// Always allow
#![allow(dead_code)]
#![allow(non_snake_case)]

pub (crate) mod decode_webm_opus_info;
pub (crate) mod open_media_source_stream;

pub mod decode_basic_audio_info;
