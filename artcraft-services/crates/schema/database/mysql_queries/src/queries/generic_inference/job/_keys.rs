//! Primary and foreign keys

/// Since jobs are internal, we'll use IDs as primary keys.
#[derive(Clone, Copy, PartialOrd, Ord, PartialEq, Eq, Debug, Serialize, Deserialize, sqlx::Type)]
#[sqlx(transparent)]
pub struct GenericInferenceJobId(pub i64);
