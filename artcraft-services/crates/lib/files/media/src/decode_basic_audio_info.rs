use std::path::{Path, PathBuf};

use symphonia::core::audio::SampleBuffer;
use symphonia::core::formats::{FormatOptions, FormatReader, Track};
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;

use errors::AnyhowResult;

use crate::decode_webm_opus_info::decode_mkv_or_webm;
use crate::open_media_source_stream::{open_bytes_media_source_stream, open_file_media_source_stream};

// Returned if nothing could be decoded
const NO_AUDIO_INFO : BasicAudioInfo = BasicAudioInfo {
  duration_millis: None,
  codec_name: None,
  required_full_decode: false
};

#[derive(Clone)]
pub struct BasicAudioInfo {
  pub codec_name: Option<String>,
  pub duration_millis: Option<u64>,
  pub required_full_decode: bool,
}

enum OriginalMediaSource<'a> {
  FileBytes(&'a [u8]),
  FilePath(PathBuf),
}

/// Decode audio info from an audio or video file containing audio streams.
/// This handles multiple formats and codecs.
pub fn decode_basic_audio_bytes_info(
  audio_bytes: &[u8],
  maybe_mimetype: Option<&str>,
  maybe_extension: Option<&str>,
) -> AnyhowResult<BasicAudioInfo>
{
  let media_source_stream = open_bytes_media_source_stream(audio_bytes)?;
  let original_media_source = OriginalMediaSource::FileBytes(audio_bytes);
  decode_basic_audio_info_inner(media_source_stream, maybe_mimetype, maybe_extension, original_media_source)
}

/// Decode audio info from an audio or video file containing audio streams.
/// This handles multiple formats and codecs.
pub fn decode_basic_audio_file_info<P: AsRef<Path>>(
  file_path: P,
  maybe_mimetype: Option<&str>,
  maybe_extension: Option<&str>,
) -> AnyhowResult<BasicAudioInfo>
{
  let media_source_stream = open_file_media_source_stream(&file_path)?;
  let original_media_source = OriginalMediaSource::FilePath(file_path.as_ref().to_path_buf());

  decode_basic_audio_info_inner(media_source_stream, maybe_mimetype, maybe_extension, original_media_source)
}

fn decode_basic_audio_info_inner(
  media_source_stream: MediaSourceStream,
  maybe_mimetype: Option<&str>,
  maybe_extension: Option<&str>,
  original_media_source: OriginalMediaSource<'_>,
) -> AnyhowResult<BasicAudioInfo> {

  let mut hint = Hint::new();

  if let Some(extension) = maybe_extension {
    hint.with_extension(extension);
  }
  if let Some(mimetype) = maybe_mimetype {
    hint.mime_type(mimetype);
  }

  // Use the default options for metadata and format readers.
  let meta_opts: MetadataOptions = Default::default();
  let fmt_opts: FormatOptions = Default::default();

  // Probe the media source.
  let probed = symphonia::default::get_probe()
      .format(&hint, media_source_stream, &fmt_opts, &meta_opts)?;

  let mut format = probed.format;

  let audio_track = match find_audio_track(&format) {
    None => return Ok(NO_AUDIO_INFO.clone()),
    Some(default_track) => default_track,
  };

  let mut maybe_track_duration = audio_track.codec_params.time_base
      .zip(audio_track.codec_params.n_frames)
      .map(|(time_base, n_frames)| {
        // NB: This yields the duration of the track
        time_base.calc_time(n_frames)
      })
      .map(|time| {
        let duration_millis = time.seconds * 1000;
        let frac_millis = (time.frac * 1000.0).trunc() as u64;
        duration_millis + frac_millis
      });

  let maybe_codec_name = get_codec_short_name(&audio_track);

  if maybe_track_duration.is_none() && maybe_codec_name.as_deref() == Some("opus") {
    // NB: Symphonia doesn't properly support webm containers with opus at time of writing (2023-04-30),
    // so we use some additional libraries to handle it. This requires a second file
    // read for now.
    let media_source_stream = match original_media_source {
      OriginalMediaSource::FileBytes(bytes) => open_bytes_media_source_stream(bytes)?,
      OriginalMediaSource::FilePath(path) => open_file_media_source_stream(&path)?,
    };

    let mut info = decode_mkv_or_webm(media_source_stream)?;
    info.codec_name = maybe_codec_name;

    return Ok(info);
  }

  let mut required_full_decode = false;

  if maybe_track_duration.is_none() {
    // NB: The number of samples could not be determined, so we need to decode the file.
    // See https://github.com/pdeljanov/Symphonia/issues/18#issuecomment-770157948

    maybe_track_duration = read_duration(&mut format)?;
    required_full_decode = true;
  }

  Ok(BasicAudioInfo {
    duration_millis: maybe_track_duration,
    codec_name: maybe_codec_name,
    required_full_decode,
  })
}

fn find_audio_track(format: &Box<dyn FormatReader>) -> Option<&Track> {
  // For audio files, the default track is sufficient.
  // Video files include multiple streams, and they will need to be searched.

  for track in format.tracks() {
    match track.codec_params.codec {
      symphonia::core::codecs::CODEC_TYPE_NULL => continue, // NB: Eg. unsupported, video, etc.
      _ => return Some(track),
    }
  }

  format.default_track()
}

fn get_codec_short_name(track: &Track) -> Option<String> {
  let codec_registry = symphonia::default::get_codecs();

  if let Some(codec_descriptor) = codec_registry.get_codec(track.codec_params.codec) {
    return Some(codec_descriptor.short_name.to_string());
  }

  match track.codec_params.codec {
    // NB: Opus (which is newer than Vorbis and for streaming) support has not landed yet, but the
    // Symphonia library knows which tracks are Opus-encoded.
    // https://github.com/pdeljanov/Symphonia/issues/8
    symphonia::core::codecs::CODEC_TYPE_OPUS => Some("opus".to_string()),
    _ => None,
  }
}

fn read_duration(format: &mut Box<dyn FormatReader>) -> AnyhowResult<Option<u64>> {
  // NB: Code adapted from symphonia repo example code.
  let audio_track = match find_audio_track(format) {
    None => return Ok(None),
    Some(track) => track,
  };

  let decoder_opts = Default::default();

  // Create a decoder for the track.
  let mut decoder = symphonia::default::get_codecs()
      .make(&audio_track.codec_params, &decoder_opts)?;

  let channel_count = match audio_track.codec_params.channels {
    None => return Ok(None),
    Some(channels) => channels.count(),
  };

  let sample_rate = match audio_track.codec_params.sample_rate {
    None => return Ok(None),
    Some(sample_rate) => sample_rate,
  };

  // Store the track identifier, we'll use it to filter packets.
  let track_id = audio_track.id;

  let mut sample_count = 0;
  let mut sample_buf = None;

  loop {
    // Get the next packet from the format reader.
    let packet = match format.next_packet() {
      Ok(packet) => packet,
      Err(_e) => break,
    };

    // If the packet does not belong to the selected track, skip it.
    if packet.track_id() != track_id {
      continue;
    }

    // Decode the packet into audio samples, ignoring any decode errors.
    match decoder.decode(&packet) {
      Ok(audio_buf) => {
        // The decoded audio samples may now be accessed via the audio buffer if per-channel
        // slices of samples in their native decoded format is desired. Use-cases where
        // the samples need to be accessed in an interleaved order or converted into
        // another sample format, or a byte buffer is required, are covered by copying the
        // audio buffer into a sample buffer or raw sample buffer, respectively. In the
        // example below, we will copy the audio buffer into a sample buffer in an
        // interleaved order while also converting to a f32 sample format.

        // If this is the *first* decoded packet, create a sample buffer matching the
        // decoded audio buffer format.
        if sample_buf.is_none() {
          // Get the audio buffer specification.
          let spec = *audio_buf.spec();

          // Get the capacity of the decoded buffer. Note: This is capacity, not length!
          let duration = audio_buf.capacity() as u64;

          // Create the f32 sample buffer.
          sample_buf = Some(SampleBuffer::<f32>::new(duration, spec));
        }

        // Copy the decoded audio buffer into the sample buffer in an interleaved format.
        if let Some(buf) = &mut sample_buf {
          buf.copy_interleaved_ref(audio_buf);

          // The samples may now be access via the `samples()` function.
          sample_count += buf.samples().len();
        }
      }
      Err(symphonia::core::errors::Error::DecodeError(_)) => (),
      Err(_) => break,
    }
  }

  let channel_samples = sample_count / channel_count;
  let duration_millis = ((channel_samples as f32) / (sample_rate as f32) * 1000.0) as u64;

  Ok(Some(duration_millis))
}

#[cfg(test)]
mod tests {
  use std::path::PathBuf;

  use errors::AnyhowResult;

  use crate::decode_basic_audio_info::decode_basic_audio_bytes_info;

  fn test_file(path_from_repo_root: &str) -> PathBuf {
    // https://doc.rust-lang.org/cargo/reference/environment-variables.html
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push(format!("../../../../{}", path_from_repo_root));
    path
  }

  // NB: The following tests make assertions on data that gets uploaded to us from the browser APIs.
  mod browser_api_data {
    use crate::decode_basic_audio_info::decode_basic_audio_file_info;

    use super::*;

    #[test]
    fn chromium_upload_webm_with_opus_audio() -> AnyhowResult<()> {
      // NB: Browser mimetype is "video/webm"
      // NB: ffprobe -i output:
      // Input #0, matroska,webm, from 'test_data/browser_api_recording/chromium_web_audio_upload.bin':
      //   Metadata:
      //     encoder         : Chrome
      //   Duration: N/A, start: 0.000000, bitrate: N/A
      //   Stream #0:0(eng): Audio: opus, 48000 Hz, mono, fltp (default)
      let path = test_file("test_data/browser_api_recording/chromium_web_audio_upload.bin");
      let info = decode_basic_audio_file_info(path, None, None)?;
      assert_eq!(info.codec_name, Some("opus".to_string()));
      assert_eq!(info.required_full_decode, true);
      assert_eq!(info.duration_millis, Some(9119));
      Ok(())
    }
  }

  mod audio {
    use super::*;

    #[test]
    fn aac() -> AnyhowResult<()> {
      // NB: ffprobe -i output:
      // [aac @ 0x55d4e03a4ec0] Estimating duration from bitrate, this may be inaccurate
      // Input #0, aac, from 'audio/aac/golden_sun_elemental_stars_cyanne.aac':
      //   Duration: 00:00:40.05, bitrate: 128 kb/s
      //   Stream #0:0: Audio: aac (LC), 16000 Hz, stereo, fltp, 128 kb/s
      // It might be faster to use ffmpeg's estimation method.
      let path = test_file("test_data/audio/aac/golden_sun_elemental_stars_cyanne.aac");
      let bytes = std::fs::read(path)?;
      let info = decode_basic_audio_bytes_info(&bytes, None, None)?;
      assert_eq!(info.codec_name, Some("aac".to_string()));
      assert_eq!(info.duration_millis, Some(40128));
      assert_eq!(info.required_full_decode, true);
      Ok(())
    }

    #[test]
    fn flac() -> AnyhowResult<()> {
      let path = test_file("test_data/audio/flac/zelda_ocarina_small_item.flac");
      let bytes = std::fs::read(path)?;
      let info = decode_basic_audio_bytes_info(&bytes, None, None)?;
      assert_eq!(info.codec_name, Some("flac".to_string()));
      assert_eq!(info.duration_millis, Some(5120));
      assert_eq!(info.required_full_decode, false);
      Ok(())
    }

    #[test]
    fn flac_wrong_extension() -> AnyhowResult<()> {
      let path = test_file("test_data/audio/flac/zelda_ocarina_small_item.flac");
      let bytes = std::fs::read(path)?;
      let incorrect_mimetype = Some("audio/wav");
      let incorrect_extension = Some("wav");
      let info = decode_basic_audio_bytes_info(&bytes, incorrect_mimetype, incorrect_extension)?;
      assert_eq!(info.codec_name, Some("flac".to_string()));
      assert_eq!(info.duration_millis, Some(5120));
      assert_eq!(info.required_full_decode, false);
      Ok(())
    }

    // NB: Requires symphonia 'aac' and 'isomp4' feature flags
    #[test]
    fn m4a() -> AnyhowResult<()> {
      let path = test_file("test_data/audio/m4a/super_mario_bros_lost_life.m4a");
      let bytes = std::fs::read(path)?;
      let info = decode_basic_audio_bytes_info(&bytes, None, None)?;
      assert_eq!(info.codec_name, Some("alac".to_string()));
      assert_eq!(info.duration_millis, Some(5493));
      assert_eq!(info.required_full_decode, false);
      Ok(())
    }

    // NB: Requires symphonia 'mp3' feature flag
    #[test]
    fn mp3() -> AnyhowResult<()> {
      let path = test_file("test_data/audio/mp3/super_mario_rpg_beware_the_forests_mushrooms.mp3");
      let bytes = std::fs::read(path)?;
      let info = decode_basic_audio_bytes_info(&bytes, None, None)?;
      assert_eq!(info.codec_name, Some("mp3".to_string()));
      assert_eq!(info.duration_millis, Some(15023));
      assert_eq!(info.required_full_decode, false);
      Ok(())
    }

    #[test]
    fn ogg() -> AnyhowResult<()> {
      // According to ffprobe (which ever so slightly disagrees with our calculation),
      //   Duration: 00:00:04.90, start: 0.000000, bitrate: 83 kb/s
      //   length          : 4.94
      let path = test_file("test_data/audio/ogg/banjo-kazooie_jiggy_appearance.ogg");
      let bytes = std::fs::read(path)?;
      let info = decode_basic_audio_bytes_info(&bytes, None, None)?;
      assert_eq!(info.codec_name, Some("vorbis".to_string()));
      assert_eq!(info.duration_millis, Some(4903)); // NB: This disagrees with ffprobe, but it's pretty close.
      assert_eq!(info.required_full_decode, true);
      Ok(())
    }

    #[test]
    fn wav_pcm_s16le_16khz() -> AnyhowResult<()> {
      let path = test_file("test_data/audio/wav/sm64_mario_its_me.wav");
      let bytes = std::fs::read(path)?;
      let info = decode_basic_audio_bytes_info(&bytes, None, None)?;
      assert_eq!(info.codec_name, Some("pcm_s16le".to_string()));
      assert_eq!(info.duration_millis, Some(1891));
      assert_eq!(info.required_full_decode, false);
      Ok(())
    }

    #[test]
    fn wav_pcm_s16le_44khz() -> AnyhowResult<()> {
      let path = test_file("test_data/audio/wav/smrpg_correct.wav");
      let bytes = std::fs::read(path)?;
      let info = decode_basic_audio_bytes_info(&bytes, None, None)?;
      assert_eq!(info.codec_name, Some("pcm_s16le".to_string()));
      assert_eq!(info.duration_millis, Some(847));
      assert_eq!(info.required_full_decode, false);
      Ok(())
    }

    #[test]
    fn wav_pcm_f32() -> AnyhowResult<()> {
      let path = test_file("test_data/audio/wav/smrpg_battlestart_f32.wav");
      let bytes = std::fs::read(path)?;
      let info = decode_basic_audio_bytes_info(&bytes, None, None)?;
      assert_eq!(info.codec_name, Some("pcm_f32le".to_string()));
      assert_eq!(info.duration_millis, Some(708));
      assert_eq!(info.required_full_decode, false);
      Ok(())
    }

    #[test]
    fn wav_unsigned() -> AnyhowResult<()> {
      let path = test_file("test_data/audio/wav/smrpg_item_mushroom_8u.wav");
      let bytes = std::fs::read(path)?;
      let info = decode_basic_audio_bytes_info(&bytes, None, None)?;
      assert_eq!(info.codec_name, Some("pcm_u8".to_string()));
      assert_eq!(info.duration_millis, Some(1741));
      assert_eq!(info.required_full_decode, false);
      Ok(())
    }
  }

  mod video {
    use super::*;

    #[test]
    fn mkv_h264_video_opus_audio() -> AnyhowResult<()> {
      let path = test_file("test_data/video/mkv/fake_you.mkv");
      let bytes = std::fs::read(path)?;
      let info = decode_basic_audio_bytes_info(&bytes, None, None)?;
      assert_eq!(info.codec_name, Some("opus".to_string()));
      assert_eq!(info.duration_millis, Some(15007));
      assert_eq!(info.required_full_decode, false);
      Ok(())
    }

    #[test]
    fn mov_h264_video_aac_audio() -> AnyhowResult<()> {
      let path = test_file("test_data/video/mov/majoras_mask_intro.mov");
      let bytes = std::fs::read(path)?;
      let info = decode_basic_audio_bytes_info(&bytes, None, None)?;
      assert_eq!(info.codec_name, Some("aac".to_string()));
      assert_eq!(info.duration_millis, Some(30128));
      assert_eq!(info.required_full_decode, false);
      Ok(())
    }

    #[test]
    fn mp4_h264_video_aac_audio() -> AnyhowResult<()> {
      let path = test_file("test_data/video/mp4/golden_sun_garoh.mp4");
      let bytes = std::fs::read(path)?;
      let info = decode_basic_audio_bytes_info(&bytes, None, None)?;
      assert_eq!(info.codec_name, Some("aac".to_string()));
      assert_eq!(info.duration_millis, Some(15295));
      assert_eq!(info.required_full_decode, false);
      Ok(())
    }

    #[test]
    fn webm_vp9_video_opus_audio() -> AnyhowResult<()> {
      let path = test_file("test_data/video/webm/laser_pong.webm");
      let bytes = std::fs::read(path)?;
      let info = decode_basic_audio_bytes_info(&bytes, None, None)?;
      assert_eq!(info.codec_name, Some("opus".to_string()));
      assert_eq!(info.duration_millis, Some(10016));
      assert_eq!(info.required_full_decode, false);
      Ok(())
    }
  }
}
