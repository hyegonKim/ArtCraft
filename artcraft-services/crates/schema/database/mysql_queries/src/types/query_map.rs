use sqlx::mysql::MySqlArguments;
use sqlx::MySql;


/// Hopefully this doesn't become a migration-blocking chore. It's meant to be a modest
/// shortcut from:
///
///  fn query<'q>(...) ->
///   sqlx::query::Map<'static, MySql, impl Send + FnMut(MySqlRow) -> Result<MyRecordType, sqlx::Error>, MySqlArguments>
///
/// To the slightly shorter (but regrettably not short enough):
///
/// fn query(...) ->
///   QueryMap<impl Send + FnMut(MySqlRow) -> Result<MyRecordType, sqlx::Error>>
///
/// We could get rid of the `impl` with a type alias if Rust supported `impl trait`
/// or `trait aliases`:
///
/// - https://rust-lang.github.io/impl-trait-initiative/explainer/tait.html
/// - https://doc.rust-lang.org/beta/unstable-book/language-features/trait-alias.html
/// - https://github.com/rust-lang/rust/issues/63063
/// - https://github.com/rust-lang/rust/issues/41517
pub type QueryMap<T> = sqlx::query::Map<'static, MySql, T, MySqlArguments> ;
