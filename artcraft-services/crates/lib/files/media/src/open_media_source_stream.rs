use std::fs::File;
use std::io::{BufReader, Cursor};
use std::path::Path;

use symphonia::core::io::{MediaSourceStream, ReadOnlySource};

use errors::AnyhowResult;

pub fn open_file_media_source_stream<P: AsRef<Path>>(file_path: P) -> AnyhowResult<MediaSourceStream> {
  let file = File::open(&file_path)?;
  let file_reader = BufReader::new(file);

  //let reader = Cursor::new(file_reader);
  let source = ReadOnlySource::new(file_reader);
  let media_source_stream =
      MediaSourceStream::new(Box::new(source), Default::default());

  Ok(media_source_stream)
}

pub fn open_bytes_media_source_stream(audio_bytes: &[u8]) -> AnyhowResult<MediaSourceStream> {
  // FIXME(bt, 2022-12-21): This is horribly inefficient.
  let bytes = audio_bytes.to_vec();
  let reader = Cursor::new(bytes);

  let source = ReadOnlySource::new(reader);
  let media_source_stream =
      MediaSourceStream::new(Box::new(source), Default::default());

  Ok(media_source_stream)
}
