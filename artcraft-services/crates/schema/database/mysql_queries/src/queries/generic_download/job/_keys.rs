//! Primary and foreign keys

/// Since jobs are internal, we'll use IDs as primary keys.
#[derive(Clone, Copy, PartialOrd, PartialEq, Eq, Ord, Debug)]
pub struct GenericDownloadJobId(pub i64);
