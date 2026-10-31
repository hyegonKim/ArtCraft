use std::error::Error;
use std::fmt::{Display, Formatter};

#[derive(Debug)]
pub enum GrokClientError {

  /// An error was encountered in building the Wreq client
  WreqClientError(wreq::Error),

  /// Error serializing a message to send the websocket
  WebsocketRequestSerializationError(serde_json::Error),

  /// Error locking the websocket for sending/receiving
  WebsocketLockError,

  /// Error reading from a websocket.
  WebsocketReadError(wreq::Error),

  /// Error sending to an open websocket.
  WebsocketSendError(wreq::Error),

  /// Can't open a local file for uploading.
  CannotOpenLocalFileForUpload(std::io::Error),

  /// Can't read a local file for uploading.
  CannotReadLocalFileForUpload(std::io::Error),

  /// Couldn't write to the file
  CannotOpenLocalFileForWriting(std::io::Error),

  /// Couldn't write to the file
  CannotWriteLocalFile(std::io::Error),

  /// File for upload has an invalid path.
  FileForUploadHasInvalidPath,

  /// Error parsing HTML
  HtmlParsingError,

  /// Our script logic is out of date
  ScriptLogicOutOfDate,

  /// Our script logic is out of date (script 1)
  Script1LogicOutOfDate,

  /// Our script logic is out of date (script 2)
  Script2LogicOutOfDate,

  /// Something is broken with timeout math
  TimeoutMathBroken,

  /// Can't make request because cookies aren't present
  NoCookiesPresent,

  /// Unknown error generating video
  ErrorGeneratingVideo,
}

impl Error for GrokClientError {}

impl Display for GrokClientError {
  fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
    match self {
      Self::WreqClientError(err) => write!(f, "Wreq client error (during client creation): {}", err),
      Self::WebsocketRequestSerializationError(err) => write!(f, "Websocket request serialization error: {}", err),
      Self::WebsocketLockError => write!(f, "Websocket lock error"),
      Self::WebsocketReadError(err) => write!(f, "Websocket read error: {}", err),
      Self::WebsocketSendError(err) => write!(f, "Websocket send error: {}", err),
      Self::CannotOpenLocalFileForUpload(err) => write!(f, "Cannot open local file for upload: {}", err),
      Self::CannotReadLocalFileForUpload(err) => write!(f, "Cannot read local file for upload: {}", err),
      Self::CannotOpenLocalFileForWriting(err) => write!(f, "Cannot open local file for writing: {}", err),
      Self::CannotWriteLocalFile(err) => write!(f, "Cannot write local file: {}", err),
      Self::FileForUploadHasInvalidPath => write!(f, "File for upload has invalid path"),
      Self::HtmlParsingError => write!(f, "Html parsing error"),
      Self::ScriptLogicOutOfDate => write!(f, "Script logic out of date"),
      Self::Script1LogicOutOfDate => write!(f, "Script logic out of date (script 1)"),
      Self::Script2LogicOutOfDate => write!(f, "Script logic out of date (script 2)"),
      Self::TimeoutMathBroken => write!(f, "Timeout math is broken"),
      Self::NoCookiesPresent => write!(f, "No cookies present"),
      Self::ErrorGeneratingVideo => write!(f, "Error generating video"),
    }
  }
}
