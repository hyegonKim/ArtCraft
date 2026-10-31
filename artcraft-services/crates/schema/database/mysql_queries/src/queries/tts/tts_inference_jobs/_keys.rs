//! Primary and foreign keys for TTS-related models

/// Since jobs are internal, we'll use IDs as primary keys.
#[derive(Clone, Copy, PartialOrd, PartialEq, Eq, Ord, Debug)]
pub struct TtsInferenceJobId(pub i64);
