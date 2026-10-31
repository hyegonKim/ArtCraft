use crate::events::basic_sendable_event_trait::BasicSendableEvent;
use crate::events::functional_events::credits_balance_changed_event::CreditsBalanceChangedEvent;
use crate::events::functional_events::subscription_plan_changed_event::SubscriptionPlanChangedEvent;
use crate::state::data_dir::app_data_root::AppDataRoot;
use crate::services::storyteller::windows::open_storyteller_billing_window::BILLING_WINDOW_NAME;
use anyhow::anyhow;
use errors::AnyhowResult;
use log::{error, info};
use tauri::{AppHandle, Manager, WebviewWindow};

pub async fn storyteller_billing_window_thread(
  app: AppHandle,
  _app_data_root: AppDataRoot,
) {
  loop {
    let billing_webview_window = match app.get_webview_window(BILLING_WINDOW_NAME) {
      Some(webview) => webview,
      None => {
        info!("Exit billing window thread.");
        return; // NB: Only exit if we don't have the webview.
      }
    };

    let result = check_billing_window(
      &app,
      &billing_webview_window,
    ).await;

    match result {
      Err(err) => {
        error!("Error checking billing window: {:?}", err);
      }
      Ok(false) => {} // Continue iteration and try again...
      Ok(true) => {
        info!("Checkout complete; exiting");
        if let Err(err) = billing_webview_window.close() {
          error!("Error closing billing window: {:?}", err);
        }

        // TODO: We can distinguish between subscription and credits impacts if we send a parameter
        //  from the webview. For now, just refresh everything.

        // Refresh the credits view
        CreditsBalanceChangedEvent{}.send_infallible(&app);
        SubscriptionPlanChangedEvent{}.send_infallible(&app);

        // And in case there's a race condition (likely), do it again after a delay.
        tokio::time::sleep(std::time::Duration::from_millis(5_000)).await;
        CreditsBalanceChangedEvent{}.send_infallible(&app);
        SubscriptionPlanChangedEvent{}.send_infallible(&app);

        return;
      }
    }

    tokio::time::sleep(std::time::Duration::from_millis(1_000)).await;
  }
}

/// Returns true if we can exit.
async fn check_billing_window(
  _app_handle: &AppHandle,
  webview_window: &WebviewWindow,
) -> AnyhowResult<bool> {

  let url = webview_window.url()?;

  let hostname= url
      .host()
      .ok_or(anyhow!("no host in url"))?
      .to_string();

  match hostname.as_str() {
    "getartcraft.com" |
    "storyteller.ai" => {
      // Checkout done. Fall-through.
    },
    "stripe.com" | "checkout.stripe.com" => {
      return Ok(false) // Still in checkout flow.
    }
    _ => {
      return Ok(false) // Unknown hostname...
    }
  }

  let path = url
      .path()
      .to_string();
  
  // TODO: This is brittle.
  
  let success = path.contains("checkout_success");
  
  info!("Checkout success: {}", success);

  // TODO: Send success events:

  // let event = RefreshAccountStateEvent {
  //   provider: Some(GenerationSource::Midjourney),
  // };
  //
  // if let Err(err) = event.send(&app_handle) {
  //   error!("Failed to send RefreshAccountStateEvent: {:?}", err); // Fail open
  // }

  Ok(true)
}
