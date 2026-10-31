use crate::types::ids::WorkspaceId;
use serde::Deserialize;

/// The workspace's credit balances, returned with every enqueue.
#[derive(Clone, Debug, Deserialize)]
pub struct Wallet {
  pub workspace_id: WorkspaceId,

  /// Credits available right now (the enqueue's cost has been deducted).
  pub credits_balance: f64,

  #[serde(default)]
  pub subscription_balance: Option<f64>,

  #[serde(default)]
  pub total_credits: Option<f64>,

  #[serde(default)]
  pub on_demand_credits: Option<f64>,

  #[serde(default)]
  pub expire_days: Option<i64>,

  /// RFC 3339 timestamp.
  #[serde(default)]
  pub wallet_created_at: Option<String>,

  /// RFC 3339 timestamp.
  #[serde(default)]
  pub next_credit_allocation_date: Option<String>,
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn wallet_parses() {
    let json = r#"{"workspace_id":"00000000-0000-0000-0000-000000000001","credits_balance":600,"subscription_balance":120000,"wallet_created_at":"2026-01-01T00:00:00.000000Z","next_credit_allocation_date":null,"total_credits":120000,"on_demand_credits":0,"expire_days":90}"#;
    let wallet: Wallet = serde_json::from_str(json).unwrap();
    assert_eq!(wallet.workspace_id.as_str(), "00000000-0000-0000-0000-000000000001");
    assert_eq!(wallet.credits_balance, 600.0);
    assert_eq!(wallet.expire_days, Some(90));
    assert!(wallet.next_credit_allocation_date.is_none());
  }
}
