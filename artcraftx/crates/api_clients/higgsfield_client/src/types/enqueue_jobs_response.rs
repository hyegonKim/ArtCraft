use crate::types::ids::{JobId, JobSetId, UserId, WorkspaceId};
use crate::types::job_params::JobParams;
use crate::types::job_set_type::JobSetType;
use crate::types::job_status::JobStatus;
use crate::types::wallet::Wallet;
use serde::Deserialize;
use serde_json::Value;

/// What every `/fnf/jobs/...` enqueue returns: the job set(s) that were
/// created, plus the wallet after the cost was deducted.
#[derive(Clone, Debug, Deserialize)]
pub struct EnqueueJobsResponse {
  /// The workspace the jobs were created in.
  pub id: WorkspaceId,

  /// One job set per enqueue in practice.
  pub job_sets: Vec<JobSet>,

  #[serde(default)]
  pub has_more: bool,

  #[serde(default)]
  pub wallet: Option<Wallet>,

  #[serde(default)]
  pub workspace_details: Option<WorkspaceDetails>,
}

impl EnqueueJobsResponse {
  /// The ids of every job created, across all job sets, in order. These are
  /// what to poll.
  pub fn job_ids(&self) -> Vec<JobId> {
    self.job_sets.iter()
        .flat_map(|job_set| job_set.jobs.iter().map(|job| job.id.clone()))
        .collect()
  }

  /// The first job set (there's normally exactly one).
  pub fn first_job_set(&self) -> Option<&JobSet> {
    self.job_sets.first()
  }
}

/// One enqueue request: `batch_size` jobs sharing the same params.
#[derive(Clone, Debug, Deserialize)]
pub struct JobSet {
  pub id: JobSetId,

  #[serde(rename = "type")]
  pub job_set_type: JobSetType,

  #[serde(default)]
  pub project_id: Option<WorkspaceId>,

  /// Unix epoch seconds (fractional).
  pub created_at: f64,

  #[serde(default)]
  pub parent_id: Option<JobSetId>,

  /// Credits charged for the whole set.
  #[serde(default)]
  pub cost: Option<f64>,

  #[serde(default)]
  pub params: JobParams,

  pub jobs: Vec<EnqueuedJob>,

  #[serde(default)]
  pub client_meta: Option<Value>,

  #[serde(default)]
  pub chain_id: Option<String>,
}

/// A job as it appears right after enqueue (no results yet).
#[derive(Clone, Debug, Deserialize)]
pub struct EnqueuedJob {
  pub id: JobId,

  pub status: JobStatus,

  /// Unix epoch seconds (fractional).
  pub created_at: f64,

  #[serde(default)]
  pub user_id: Option<UserId>,

  #[serde(default)]
  pub trace_id: Option<String>,

  #[serde(default)]
  pub cluster_hash: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct WorkspaceDetails {
  pub id: WorkspaceId,

  #[serde(default)]
  pub name: Option<String>,

  /// e.g. `private`.
  #[serde(default, rename = "type")]
  pub workspace_type: Option<String>,

  /// e.g. `owner`.
  #[serde(default)]
  pub user_role: Option<String>,

  #[serde(default)]
  pub is_enterprise_sub_workspace: bool,
}

#[cfg(test)]
pub(crate) mod tests {
  use super::*;

  /// The enqueue response for a Nano Banana Pro job (ids / user scrubbed).
  pub(crate) const NANO_BANANA_ENQUEUE_RESPONSE: &str = r#"{"id":"00000000-0000-0000-0000-00000000aaaa","job_sets":[{"id":"00000000-0000-0000-0000-00000000bbbb","type":"nano_banana_2","project_id":"00000000-0000-0000-0000-00000000aaaa","created_at":1788147223.616406,"parent_id":null,"cost":200,"params":{"width":864,"height":1184,"aspect_ratio":"3:4","resolution":"1k","batch_size":1,"input_images":[],"input_image":null,"application":null,"surface":null,"prompt":"a dinosaur on a skateboard"},"jobs":[{"id":"00000000-0000-0000-0000-00000000cccc","status":"waiting","ip_check_finished":null,"ip_detected":null,"result":null,"results":null,"board_ids":[],"published_at":null,"meta":{},"created_at":1788147223.626676,"user_id":"user_TESTUSER0000000000000000000","trace_id":"00000000-0000-0000-0000-00000000cccc","folder_ids":[],"is_favourite":false}],"client_meta":null,"chain_id":null}],"has_more":false,"wallet":{"workspace_id":"00000000-0000-0000-0000-00000000aaaa","credits_balance":600,"subscription_balance":120000,"wallet_created_at":"2026-08-31T03:26:11.027426Z","next_credit_allocation_date":null,"total_credits":120000,"on_demand_credits":0,"expire_days":90},"workspace_details":{"id":"00000000-0000-0000-0000-00000000aaaa","name":null,"type":"private","user_role":"owner","clerk_organization_id":null,"avatar_url":null,"bio":null,"grace_period_type":null,"sso_status":"idle","is_enterprise_sub_workspace":false,"sub_workspace_block":null},"free_gens_v2":{"items":[]},"generation_seconds":{"items":[]},"folder_credits":null}"#;

  /// The enqueue response for a GPT Image 2 job (ids / user scrubbed).
  pub(crate) const GPT_IMAGE_ENQUEUE_RESPONSE: &str = r#"{"id":"00000000-0000-0000-0000-00000000aaaa","job_sets":[{"id":"00000000-0000-0000-0000-00000000dddd","type":"gpt_image_2","project_id":"00000000-0000-0000-0000-00000000aaaa","created_at":1788148993.964493,"parent_id":null,"cluster_hash":"d8cd56aedaaaeedac3a5ed262f5eed0e","cost":850.0,"params":{"width":1152,"height":2048,"prompt":"a corgi on a bike","medias":[],"aspect_ratio":"9:16","quality":"high","resolution":"2k","model":"videotape-alpha","remove_bg":false,"reference_elements":[]},"jobs":[{"id":"00000000-0000-0000-0000-00000000eeee","status":"queued","ip_check_finished":null,"ip_detected":null,"result":null,"results":null,"board_ids":[],"published_at":null,"meta":{},"created_at":1788148993.971706,"user_id":"user_TESTUSER0000000000000000000","trace_id":"00000000-0000-0000-0000-00000000eeee","cluster_hash":"d8cd56aedaaaeedac3a5ed262f5eed0e","representation":null,"folder_ids":[],"is_favourite":false}],"client_meta":null,"chain_id":null}],"has_more":false,"wallet":{"workspace_id":"00000000-0000-0000-0000-00000000aaaa","credits_balance":0,"subscription_balance":119350,"wallet_created_at":"2026-08-31T03:26:11.027426Z","next_credit_allocation_date":null,"total_credits":120000,"on_demand_credits":0,"expire_days":90},"workspace_details":{"id":"00000000-0000-0000-0000-00000000aaaa","name":null,"type":"private","user_role":"owner","clerk_organization_id":null,"avatar_url":null,"bio":null,"grace_period_type":null,"sso_status":"idle","is_enterprise_sub_workspace":false,"sub_workspace_block":null},"free_gens_v2":{"items":[]},"generation_seconds":{"items":[]},"folder_credits":null}"#;

  #[test]
  fn nano_banana_enqueue_response_parses() {
    let response: EnqueueJobsResponse = serde_json::from_str(NANO_BANANA_ENQUEUE_RESPONSE).unwrap();
    assert_eq!(response.id.as_str(), "00000000-0000-0000-0000-00000000aaaa");
    assert_eq!(response.job_sets.len(), 1);

    let job_set = response.first_job_set().unwrap();
    assert_eq!(job_set.job_set_type, JobSetType::NanoBanana2);
    assert_eq!(job_set.cost, Some(200.0));
    assert_eq!(job_set.params.prompt.as_deref(), Some("a dinosaur on a skateboard"));
    assert_eq!(job_set.jobs.len(), 1);
    assert_eq!(job_set.jobs[0].status, JobStatus::Waiting);

    assert_eq!(response.job_ids(), vec![JobId::new("00000000-0000-0000-0000-00000000cccc")]);
    assert_eq!(response.wallet.as_ref().unwrap().credits_balance, 600.0);
    assert_eq!(response.workspace_details.as_ref().unwrap().user_role.as_deref(), Some("owner"));
  }

  #[test]
  fn gpt_image_enqueue_response_parses() {
    let response: EnqueueJobsResponse = serde_json::from_str(GPT_IMAGE_ENQUEUE_RESPONSE).unwrap();
    let job_set = response.first_job_set().unwrap();
    assert_eq!(job_set.job_set_type, JobSetType::GptImage2);
    assert_eq!(job_set.cost, Some(850.0));
    assert_eq!(job_set.jobs[0].status, JobStatus::Queued);
    assert_eq!(job_set.jobs[0].cluster_hash.as_deref(), Some("d8cd56aedaaaeedac3a5ed262f5eed0e"));
    assert_eq!(job_set.params.model.as_deref(), Some("videotape-alpha"));
  }
}
