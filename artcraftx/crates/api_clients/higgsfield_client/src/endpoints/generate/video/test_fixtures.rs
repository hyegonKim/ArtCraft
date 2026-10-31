//! Builds a scrubbed enqueue response around a captured `job_sets[0]`
//! (server-normalized params), so each binding's parse test uses the real
//! shape without repeating the wallet / workspace boilerplate.

pub fn enqueue_response(job_set_type: &str, cost: u32, params_json: &str, with_cluster_hash: bool) -> String {
  let cluster_hash = if with_cluster_hash { r#""cluster_hash":"d8cd56aedaaaeedac3a5ed262f5eed0e","# } else { "" };
  format!(
    r#"{{"id":"00000000-0000-0000-0000-00000000aaaa","job_sets":[{{"id":"00000000-0000-0000-0000-00000000bbbb","type":"{job_set_type}","project_id":"00000000-0000-0000-0000-00000000aaaa","created_at":1788155400.1,"parent_id":null,{cluster_hash}"cost":{cost},"params":{params_json},"jobs":[{{"id":"00000000-0000-0000-0000-00000000cccc","status":"queued","ip_check_finished":null,"ip_detected":null,"result":null,"results":null,"board_ids":[],"published_at":null,"meta":{{}},"created_at":1788155400.2,"user_id":"user_TESTUSER0000000000000000000","trace_id":"00000000-0000-0000-0000-00000000cccc","representation":null,"folder_ids":[],"is_favourite":false}}],"client_meta":null,"chain_id":null}}],"has_more":false,"wallet":{{"workspace_id":"00000000-0000-0000-0000-00000000aaaa","credits_balance":0,"subscription_balance":110000,"wallet_created_at":"2026-08-31T03:26:11.027426Z","next_credit_allocation_date":null,"total_credits":120000,"on_demand_credits":0,"expire_days":90}},"workspace_details":{{"id":"00000000-0000-0000-0000-00000000aaaa","name":null,"type":"private","user_role":"owner","clerk_organization_id":null,"avatar_url":null,"bio":null,"grace_period_type":null,"sso_status":"idle","is_enterprise_sub_workspace":false,"sub_workspace_block":null}},"free_gens_v2":{{"items":[]}},"generation_seconds":{{"items":[]}},"folder_credits":null}}"#
  )
}
