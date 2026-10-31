use crate::commands::generate::common::maybe_notify_frontend_of_billing_errors::maybe_notify_frontend_of_billing_errors;
use crate::commands::generate::common::maybe_notify_frontend_of_credential_errors::maybe_notify_frontend_of_credential_errors;
use crate::commands::generate::common::maybe_notify_frontend_of_grok_errors::maybe_notify_frontend_of_grok_errors;
use crate::commands::generate::generate_error::GenerateError;
use tauri::AppHandle;

pub async fn notify_frontend_of_errors(
  app: &AppHandle,
  error: &GenerateError,
) {
  maybe_notify_frontend_of_billing_errors(app, error).await;
  maybe_notify_frontend_of_credential_errors(app, error).await;
  maybe_notify_frontend_of_grok_errors(app, error).await;
}
