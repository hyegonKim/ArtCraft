use symphonia::core::formats::{FormatOptions, FormatReader};
use symphonia::core::io::MediaSourceStream;
use symphonia::default::formats::MkvReader;

use errors::{anyhow, AnyhowResult};

use crate::decode_basic_audio_info::BasicAudioInfo;

pub fn decode_mkv_or_webm(
  media_source_stream: MediaSourceStream
) -> AnyhowResult<BasicAudioInfo> {

  let options = FormatOptions::default();
  let mut reader = MkvReader::try_new(media_source_stream, &options)?;

  // The `matroska-demuxer` crate (ver "0.4.0") (as an alternative) exposes this as
  // mkv.info().timestamp_scale()
  let time_base = reader.default_track()
      .map(|track| track.codec_params.time_base)
      .flatten()
      .ok_or_else(|| anyhow!("file did not have a default track!"))?;

  let mut last_timestamp = 0;

  while let Ok(packet) = reader.next_packet() {
    // Eg. a typical value might be `9119`.
    last_timestamp = packet.ts;
  }

  let seconds_per_packet = (time_base.numer as f64) / (time_base.denom as f64);
  let duration_seconds = (last_timestamp as f64) * seconds_per_packet;
  let duration_millis = (duration_seconds * 1000.0) as u64;

  Ok(BasicAudioInfo {
    codec_name: None, // NB: Caller will need to fill this out.
    duration_millis: Some(duration_millis),
    required_full_decode: true,
  })
}
