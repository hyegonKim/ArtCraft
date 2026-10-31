use crate::events::basic_sendable_event_trait::BasicSendableEvent;
use crate::events::functional_events::app_preferences_changed_event::AppPreferencesChangedEvent;
use tauri::AppHandle;

/// Tell the frontend the preferences changed so it drops its cached copy.
/// Every command that writes preferences calls this after a successful save.
pub fn notify_app_preferences_changed(app: &AppHandle) {
  AppPreferencesChangedEvent {}.send_infallible(app);
}
