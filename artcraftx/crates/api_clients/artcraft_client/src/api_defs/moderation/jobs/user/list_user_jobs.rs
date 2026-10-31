use chrono::{DateTime, Utc};
use crate::enums::by_table::generic_inference_jobs::inference_job_external_third_party::InferenceJobExternalThirdParty;
use crate::enums::by_table::wallet_ledger_entries::wallet_ledger_entry_type::WalletLedgerEntryType;
use crate::enums::common::job_status_plus::JobStatusPlus;
use serde_derive::{Deserialize, Serialize};
use crate::tokens::generic_inference_jobs::InferenceJobToken;
use sqlite_identifiers::ids::media_file_token::MediaFileToken;
use crate::tokens::users::UserToken;
use crate::tokens::wallet_ledger_entries::WalletLedgerEntryToken;

pub const LIST_USER_JOBS_PATH: &str = "/v1/moderation/jobs/user/{user_token}/list";

#[derive(Deserialize)]
pub struct ListUserJobsPathInfo {
  pub user_token: UserToken,
}

#[derive(Serialize)]
pub struct ListUserJobsResponse {
  pub success: bool,
  pub jobs: Vec<ListUserJobsEntry>,
}

#[derive(Serialize)]
pub struct ListUserJobsEntry {
  pub job_status: JobStatusPlus,
  pub job_failure_reason: Option<String>,
  pub credits_delta: Option<i32>,
  pub maybe_linked_refund_ledger_token: Option<WalletLedgerEntryToken>,
  pub on_success_result_media_token: Option<MediaFileToken>,
  pub job_token: InferenceJobToken,
  pub wallet_ledger_entry_token: Option<WalletLedgerEntryToken>,
  pub wallet_ledger_entry_type: Option<WalletLedgerEntryType>,
  pub maybe_external_third_party: Option<InferenceJobExternalThirdParty>,
  pub maybe_external_third_party_id: Option<String>,
  pub created_at: DateTime<Utc>,
  pub updated_at: DateTime<Utc>,
}
