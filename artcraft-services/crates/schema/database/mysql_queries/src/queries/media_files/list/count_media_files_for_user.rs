use sqlx::{Executor, MySql, QueryBuilder};

use super::user_media_file_filters::UserMediaFileFilters;

pub struct CountMediaFilesForUserArgs<'a, T> {
  pub filters: UserMediaFileFilters<'a>,
  pub mysql_executor: T,
}

/// Legacy numbered pagination needs an exact total. Count only this user's rows,
/// without related-table lookups or global low-selectivity index intersections.
pub async fn count_media_files_for_user<'c, T>(args: CountMediaFilesForUserArgs<'_, T>) -> Result<i64, sqlx::Error>
where
  T: Executor<'c, Database = MySql>,
{
  let mut query =
    QueryBuilder::<MySql>::new("SELECT COUNT(*) FROM media_files m FORCE INDEX (fk_maybe_creator_user_token)");
  args.filters.push_predicates(&mut query);
  query.build_query_scalar().fetch_one(args.mysql_executor).await
}
