use anyhow::anyhow;
use log::warn;
use sqlx::MySqlPool;

use errors::AnyhowResult;

#[derive(Serialize, Clone)]
pub struct OnPremWorkerStats {
  pub total_records_sampled: i64,
  pub on_prem_count: i64,
  pub cloud_count: i64,
}

/// Return this when there are no results.
const NO_RESULTS_SENTINEL : OnPremWorkerStats = OnPremWorkerStats {
  total_records_sampled: -1,
  on_prem_count: -1,
  cloud_count: -1,
};

pub async fn get_on_prem_worker_stats(
  mysql_pool: &MySqlPool,
  sample_size: u32,
) -> AnyhowResult<OnPremWorkerStats> {

  let sample_size_wide = sample_size as i64;

  // NB: Lookup failure is Err(RowNotFound).
  let maybe_result = sqlx::query_as!(
      OnPremRaw,
        r#"
select count(*) as on_prem_count
from (
  select is_generated_on_prem
  from tts_results
  order by id desc limit ?
) as sample
where sample.is_generated_on_prem IS TRUE;
      "#,
      sample_size
    )
      .fetch_one(mysql_pool)
      .await;

  let result : OnPremWorkerStats = match maybe_result {
    Ok(result) => OnPremWorkerStats {
      total_records_sampled: sample_size_wide,
      on_prem_count: result.on_prem_count,
      cloud_count: sample_size_wide.saturating_sub(result.on_prem_count),
    },
    Err(sqlx::Error::RowNotFound) => NO_RESULTS_SENTINEL.clone(),
    Err(e) => {
      warn!("get on prem worker stats error: {:?}", e);
      return Err(anyhow!("couldn't fetch on prem worker stats"));
    }
  };

  Ok(result)
}


struct OnPremRaw {
  on_prem_count: i64,
}