use std::fmt::Debug;

use serde::Deserialize;
use serde::Serialize;


/// The primary key for uploaded_videos.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Serialize, Deserialize)]
pub struct UploadedVideoToken(pub String);

impl_client_token!(UploadedVideoToken);
