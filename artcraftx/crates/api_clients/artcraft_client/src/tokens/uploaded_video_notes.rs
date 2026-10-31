use std::fmt::Debug;

use serde::Deserialize;
use serde::Serialize;


/// The primary key for uploaded_video_notes.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Serialize, Deserialize)]
pub struct UploadVideoNoteToken(pub String);

impl_client_token!(UploadVideoNoteToken);
