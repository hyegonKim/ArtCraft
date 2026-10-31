// TODO(bt,2024-06-28): This note previously asked to remove users of these constants, but it's probably
//  preferable to keep them close to MySQL usage and not put them in `shared_constants` with unrelated stuff.

/// The default MySql connection string for use in development
pub const DEFAULT_MYSQL_CONNECTION_STRING: &str = "mysql://storyteller:password@localhost/storyteller";

/// The number of results we return by default for paginated queries.
pub const DEFAULT_MYSQL_QUERY_RESULT_PAGE_SIZE: u16 = 25;
