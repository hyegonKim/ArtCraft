use std::collections::BTreeSet;

#[cfg(test)]
use strum::EnumCount;
#[cfg(test)]
use strum::EnumIter;

/// Defines the names of the Tauri-sent events that the frontend subscribes to.
/// These event names are also stored in the database, so keep them short-ish.
///
/// NB: Events should end in "_event" so they're easy to grep for in Javascript.
#[cfg_attr(test, derive(EnumIter, EnumCount))]
#[derive(Clone, Copy, PartialEq, Eq, Hash, Ord, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TauriEventName {
  // TODO: Get rid of kebab case.
  /// General purpose event:
  /// Generation enqueued
  #[serde(rename = "generation-enqueue-success-event")]
  GenerationEnqueueSuccessEvent,

  // TODO: Get rid of kebab case.
  /// General purpose event:
  /// Generation failed to enqueue
  #[serde(rename = "generation-enqueue-failure-event")]
  GenerationEnqueueFailureEvent,

  // TODO: Get rid of kebab case.
  /// General purpose event:
  /// Generation completed successfully
  #[serde(rename = "generation-complete-event")]
  GenerationCompleteEvent,

  // TODO: Get rid of kebab case.
  /// General purpose event:
  /// Generation failed
  #[serde(rename = "generation-failed-event")]
  GenerationFailedEvent,

  /// Informational event:
  /// Credits were purchased or spent (but we don't know how much)
  #[serde(rename = "credits_balance_changed_event")]
  CreditsBalanceChangedEvent,

  /// Informational event:
  /// The app preferences were saved; re-read them.
  #[serde(rename = "app_preferences_changed_event")]
  AppPreferencesChangedEvent,

  /// Informational event:
  /// Subscription was changed (but we don't know the details - upgrade, cancel, etc.)
  #[serde(rename = "subscription_plan_changed_event")]
  SubscriptionPlanChangedEvent,

  /// Special event:
  /// Image generation is complete
  #[serde(rename = "text_to_image_generation_complete_event")]
  TextToImageGenerationCompleteEvent,

  /// Special event:
  /// Object (3D mesh) generation is complete
  #[serde(rename = "object_generation_complete_event")]
  ObjectGenerationCompleteEvent,
  
  /// Special event:
  /// Gaussian generation is complete
  #[serde(rename = "gaussian_generation_complete_event")]
  GaussianGenerationCompleteEvent,

  /// Special event:
  /// Video generation is complete
  #[serde(rename = "video_generation_complete_event")]
  VideoGenerationCompleteEvent,

  /// Special event:
  /// Refresh account states
  #[serde(rename = "refresh_account_state_event")]
  RefreshAccountStateEvent,

  /// Special event:
  /// The thumbnail worker produced a thumbnail for a downloaded file
  #[serde(rename = "local_thumbnail_ready_event")]
  LocalThumbnailReadyEvent,

  /// Special event:
  /// Show a billing modal (a suggestion to pay, etc. for a specific provider)
  #[serde(rename = "show_provider_billing_modal_event")]
  ShowProviderBillingModalEvent,

  /// Special event:
  /// Show a dismissable modal explaining a credential problem (missing,
  /// unknown, or unusable credential for a generation request)
  #[serde(rename = "show_credential_error_modal_event")]
  ShowCredentialErrorModalEvent,

  /// Notice event:
  /// Flash an informational (non-error) message, e.g. "Your library was
  /// refreshed".
  #[serde(rename = "flash_notice_event")]
  FlashNoticeEvent,

  /// Notice event:
  /// A long-running backend step started or finished, e.g. Higgsfield's
  /// IP/likeness check on uploaded media. The frontend shows a progress
  /// toast on `started` and takes it down on `finished`.
  #[serde(rename = "progress_notice_event")]
  ProgressNoticeEvent,

  /// Warning event:
  /// Flash a user input error message
  #[serde(rename = "flash_user_input_error_event")]
  FlashUserInputErrorEvent,

  /// Warning event:
  /// File with that name was already downloaded
  #[serde(rename = "flash_file_download_error_event")]
  FlashFileDownloadErrorEvent,
}

impl_enum_display_and_debug_using_to_str!(TauriEventName);

// NB: We can derive `sqlx::Type` instead of using `impl_mysql_enum_coders`

impl TauriEventName {
  pub fn to_str(&self) -> &'static str {
    match self {
      Self::GenerationEnqueueSuccessEvent => "generation-enqueue-success-event",
      Self::GenerationEnqueueFailureEvent => "generation-enqueue-failure-event",
      Self::GenerationCompleteEvent => "generation-complete-event",
      Self::GenerationFailedEvent => "generation-failed-event",
      Self::CreditsBalanceChangedEvent => "credits_balance_changed_event",
      Self::AppPreferencesChangedEvent => "app_preferences_changed_event",
      Self::SubscriptionPlanChangedEvent => "subscription_plan_changed_event",
      Self::TextToImageGenerationCompleteEvent => "text_to_image_generation_complete_event",
      Self::ObjectGenerationCompleteEvent => "object_generation_complete_event",
      Self::GaussianGenerationCompleteEvent => "gaussian_generation_complete_event",
      Self::VideoGenerationCompleteEvent => "video_generation_complete_event",
      Self::RefreshAccountStateEvent => "refresh_account_state_event",
      Self::LocalThumbnailReadyEvent => "local_thumbnail_ready_event",
      Self::ShowProviderBillingModalEvent => "show_provider_billing_modal_event",
      Self::ShowCredentialErrorModalEvent => "show_credential_error_modal_event",
      Self::FlashNoticeEvent => "flash_notice_event",
      Self::ProgressNoticeEvent => "progress_notice_event",
      Self::FlashUserInputErrorEvent => "flash_user_input_error_event",
      Self::FlashFileDownloadErrorEvent => "flash_file_download_error_event",
    }
  }

  pub fn from_str(job_status: &str) -> Result<Self, String> {
    match job_status {
      "generation-enqueue-success-event" => Ok(Self::GenerationEnqueueSuccessEvent),
      "generation-enqueue-failure-event" => Ok(Self::GenerationEnqueueFailureEvent),
      "generation-complete-event" => Ok(Self::GenerationCompleteEvent),
      "generation-failed-event" => Ok(Self::GenerationFailedEvent),
      "credits_balance_changed_event" => Ok(Self::CreditsBalanceChangedEvent),
      "app_preferences_changed_event" => Ok(Self::AppPreferencesChangedEvent),
      "subscription_plan_changed_event" => Ok(Self::SubscriptionPlanChangedEvent),
      "text_to_image_generation_complete_event" => Ok(Self::TextToImageGenerationCompleteEvent),
      "object_generation_complete_event" => Ok(Self::ObjectGenerationCompleteEvent),
      "gaussian_generation_complete_event" => Ok(Self::GaussianGenerationCompleteEvent),
      "video_generation_complete_event" => Ok(Self::VideoGenerationCompleteEvent),
      "refresh_account_state_event" => Ok(Self::RefreshAccountStateEvent),
      "local_thumbnail_ready_event" => Ok(Self::LocalThumbnailReadyEvent),
      "show_provider_billing_modal_event" => Ok(Self::ShowProviderBillingModalEvent),
      "show_credential_error_modal_event" => Ok(Self::ShowCredentialErrorModalEvent),
      "flash_notice_event" => Ok(Self::FlashNoticeEvent),
      "progress_notice_event" => Ok(Self::ProgressNoticeEvent),
      "flash_user_input_error_event" => Ok(Self::FlashUserInputErrorEvent),
      "flash_file_download_error_event" => Ok(Self::FlashFileDownloadErrorEvent),
      _ => Err(format!("invalid tauri_event_name: {:?}", job_status)),
    }
  }

  pub fn all_variants() -> BTreeSet<Self> {
    // NB: BTreeSet is sorted
    // NB: BTreeSet::from() isn't const, but not worth using LazyStatic, etc.
    BTreeSet::from([
      Self::GenerationEnqueueSuccessEvent,
      Self::GenerationEnqueueFailureEvent,
      Self::GenerationCompleteEvent,
      Self::GenerationFailedEvent,
      Self::CreditsBalanceChangedEvent,
      Self::AppPreferencesChangedEvent,
      Self::SubscriptionPlanChangedEvent,
      Self::TextToImageGenerationCompleteEvent,
      Self::ObjectGenerationCompleteEvent,
      Self::GaussianGenerationCompleteEvent,
      Self::VideoGenerationCompleteEvent,
      Self::RefreshAccountStateEvent,
      Self::LocalThumbnailReadyEvent,
      Self::ShowProviderBillingModalEvent,
      Self::ShowCredentialErrorModalEvent,
      Self::FlashNoticeEvent,
      Self::ProgressNoticeEvent,
      Self::FlashUserInputErrorEvent,
      Self::FlashFileDownloadErrorEvent,
    ])
  }
}

#[cfg(test)]
mod tests {
  use crate::enums::tauri::ux::tauri_event_name::TauriEventName;
  use crate::enums::test_helpers::assert_serialization;

  mod explicit_checks {
    use super::*;

    #[test]
    fn test_serialization() {
      assert_serialization(TauriEventName::GenerationEnqueueSuccessEvent, "generation-enqueue-success-event");
      assert_serialization(TauriEventName::GenerationEnqueueFailureEvent, "generation-enqueue-failure-event");
      assert_serialization(TauriEventName::GenerationCompleteEvent, "generation-complete-event");
      assert_serialization(TauriEventName::GenerationFailedEvent, "generation-failed-event");
      assert_serialization(TauriEventName::CreditsBalanceChangedEvent, "credits_balance_changed_event");
      assert_serialization(TauriEventName::AppPreferencesChangedEvent, "app_preferences_changed_event");
      assert_serialization(TauriEventName::SubscriptionPlanChangedEvent, "subscription_plan_changed_event");
      assert_serialization(TauriEventName::TextToImageGenerationCompleteEvent, "text_to_image_generation_complete_event");
      assert_serialization(TauriEventName::ObjectGenerationCompleteEvent, "object_generation_complete_event");
      assert_serialization(TauriEventName::GaussianGenerationCompleteEvent, "gaussian_generation_complete_event");
      assert_serialization(TauriEventName::VideoGenerationCompleteEvent, "video_generation_complete_event");
      assert_serialization(TauriEventName::RefreshAccountStateEvent, "refresh_account_state_event");
      assert_serialization(TauriEventName::LocalThumbnailReadyEvent, "local_thumbnail_ready_event");
      assert_serialization(TauriEventName::ShowProviderBillingModalEvent, "show_provider_billing_modal_event"); 
      assert_serialization(TauriEventName::ShowCredentialErrorModalEvent, "show_credential_error_modal_event");
      assert_serialization(TauriEventName::FlashNoticeEvent, "flash_notice_event");
      assert_serialization(TauriEventName::ProgressNoticeEvent, "progress_notice_event");
      assert_serialization(TauriEventName::FlashUserInputErrorEvent, "flash_user_input_error_event");
      assert_serialization(TauriEventName::FlashFileDownloadErrorEvent, "flash_file_download_error_event");
    }

    #[test]
    fn to_str() {
      assert_eq!(TauriEventName::GenerationEnqueueSuccessEvent.to_str(), "generation-enqueue-success-event");
      assert_eq!(TauriEventName::GenerationEnqueueFailureEvent.to_str(), "generation-enqueue-failure-event");
      assert_eq!(TauriEventName::GenerationCompleteEvent.to_str(), "generation-complete-event");
      assert_eq!(TauriEventName::GenerationFailedEvent.to_str(), "generation-failed-event");
      assert_eq!(TauriEventName::CreditsBalanceChangedEvent.to_str(), "credits_balance_changed_event");
      assert_eq!(TauriEventName::AppPreferencesChangedEvent.to_str(), "app_preferences_changed_event");
      assert_eq!(TauriEventName::SubscriptionPlanChangedEvent.to_str(), "subscription_plan_changed_event");
      assert_eq!(TauriEventName::TextToImageGenerationCompleteEvent.to_str(), "text_to_image_generation_complete_event");
      assert_eq!(TauriEventName::ObjectGenerationCompleteEvent.to_str(), "object_generation_complete_event");
      assert_eq!(TauriEventName::GaussianGenerationCompleteEvent.to_str(), "gaussian_generation_complete_event");
      assert_eq!(TauriEventName::VideoGenerationCompleteEvent.to_str(), "video_generation_complete_event");
      assert_eq!(TauriEventName::RefreshAccountStateEvent.to_str(), "refresh_account_state_event");
      assert_eq!(TauriEventName::LocalThumbnailReadyEvent.to_str(), "local_thumbnail_ready_event");
      assert_eq!(TauriEventName::ShowProviderBillingModalEvent.to_str(), "show_provider_billing_modal_event");
      assert_eq!(TauriEventName::FlashNoticeEvent.to_str(), "flash_notice_event");
      assert_eq!(TauriEventName::ProgressNoticeEvent.to_str(), "progress_notice_event");
      assert_eq!(TauriEventName::FlashUserInputErrorEvent.to_str(), "flash_user_input_error_event");
      assert_eq!(TauriEventName::FlashFileDownloadErrorEvent.to_str(), "flash_file_download_error_event");
    }

    #[test]
    fn from_str() {
      assert_eq!(TauriEventName::from_str("generation-enqueue-success-event").unwrap(), TauriEventName::GenerationEnqueueSuccessEvent);
      assert_eq!(TauriEventName::from_str("generation-enqueue-failure-event").unwrap(), TauriEventName::GenerationEnqueueFailureEvent);
      assert_eq!(TauriEventName::from_str("generation-complete-event").unwrap(), TauriEventName::GenerationCompleteEvent);
      assert_eq!(TauriEventName::from_str("generation-failed-event").unwrap(), TauriEventName::GenerationFailedEvent);
      assert_eq!(TauriEventName::from_str("credits_balance_changed_event").unwrap(), TauriEventName::CreditsBalanceChangedEvent);
      assert_eq!(TauriEventName::from_str("app_preferences_changed_event").unwrap(), TauriEventName::AppPreferencesChangedEvent);
      assert_eq!(TauriEventName::from_str("subscription_plan_changed_event").unwrap(), TauriEventName::SubscriptionPlanChangedEvent);
      assert_eq!(TauriEventName::from_str("text_to_image_generation_complete_event").unwrap(), TauriEventName::TextToImageGenerationCompleteEvent);
      assert_eq!(TauriEventName::from_str("object_generation_complete_event").unwrap(), TauriEventName::ObjectGenerationCompleteEvent);
      assert_eq!(TauriEventName::from_str("gaussian_generation_complete_event").unwrap(), TauriEventName::GaussianGenerationCompleteEvent);
      assert_eq!(TauriEventName::from_str("video_generation_complete_event").unwrap(), TauriEventName::VideoGenerationCompleteEvent);
      assert_eq!(TauriEventName::from_str("refresh_account_state_event").unwrap(), TauriEventName::RefreshAccountStateEvent);
      assert_eq!(TauriEventName::from_str("local_thumbnail_ready_event").unwrap(), TauriEventName::LocalThumbnailReadyEvent);
      assert_eq!(TauriEventName::from_str("show_provider_billing_modal_event").unwrap(), TauriEventName::ShowProviderBillingModalEvent);
      assert_eq!(TauriEventName::from_str("show_credential_error_modal_event").unwrap(), TauriEventName::ShowCredentialErrorModalEvent);
      assert_eq!(TauriEventName::from_str("flash_notice_event").unwrap(), TauriEventName::FlashNoticeEvent);
      assert_eq!(TauriEventName::from_str("progress_notice_event").unwrap(), TauriEventName::ProgressNoticeEvent);
      assert_eq!(TauriEventName::from_str("flash_user_input_error_event").unwrap(), TauriEventName::FlashUserInputErrorEvent);
      assert_eq!(TauriEventName::from_str("flash_file_download_error_event").unwrap(), TauriEventName::FlashFileDownloadErrorEvent);
    }

    #[test]
    fn all_variants() {
      let mut variants = TauriEventName::all_variants();
      assert_eq!(variants.len(), 19);
      assert_eq!(variants.pop_first(), Some(TauriEventName::GenerationEnqueueSuccessEvent));
      assert_eq!(variants.pop_first(), Some(TauriEventName::GenerationEnqueueFailureEvent));
      assert_eq!(variants.pop_first(), Some(TauriEventName::GenerationCompleteEvent));
      assert_eq!(variants.pop_first(), Some(TauriEventName::GenerationFailedEvent));
      assert_eq!(variants.pop_first(), Some(TauriEventName::CreditsBalanceChangedEvent));
      assert_eq!(variants.pop_first(), Some(TauriEventName::AppPreferencesChangedEvent));
      assert_eq!(variants.pop_first(), Some(TauriEventName::SubscriptionPlanChangedEvent));
      assert_eq!(variants.pop_first(), Some(TauriEventName::TextToImageGenerationCompleteEvent));
      assert_eq!(variants.pop_first(), Some(TauriEventName::ObjectGenerationCompleteEvent));
      assert_eq!(variants.pop_first(), Some(TauriEventName::GaussianGenerationCompleteEvent));
      assert_eq!(variants.pop_first(), Some(TauriEventName::VideoGenerationCompleteEvent));
      assert_eq!(variants.pop_first(), Some(TauriEventName::RefreshAccountStateEvent));
      assert_eq!(variants.pop_first(), Some(TauriEventName::LocalThumbnailReadyEvent));
      assert_eq!(variants.pop_first(), Some(TauriEventName::ShowProviderBillingModalEvent));
      assert_eq!(variants.pop_first(), Some(TauriEventName::ShowCredentialErrorModalEvent));
      assert_eq!(variants.pop_first(), Some(TauriEventName::FlashNoticeEvent));
      assert_eq!(variants.pop_first(), Some(TauriEventName::ProgressNoticeEvent));
      assert_eq!(variants.pop_first(), Some(TauriEventName::FlashUserInputErrorEvent));
      assert_eq!(variants.pop_first(), Some(TauriEventName::FlashFileDownloadErrorEvent));
      assert_eq!(variants.pop_first(), None);
    }
  }

  mod mechanical_checks {
    use super::*;
    use strum::IntoEnumIterator;

    #[test]
    fn variant_length() {
      assert_eq!(TauriEventName::all_variants().len(), TauriEventName::iter().len());
    }

    #[test]
    fn round_trip() {
      for variant in TauriEventName::all_variants() {
        // Test to_str(), from_str(), Display, and Debug.
        assert_eq!(variant, TauriEventName::from_str(variant.to_str()).unwrap());
        assert_eq!(variant, TauriEventName::from_str(&format!("{}", variant)).unwrap());
        assert_eq!(variant, TauriEventName::from_str(&format!("{:?}", variant)).unwrap());
      }
    }

    //#[test]
    //fn serialized_length_ok_for_database() {
    //  const MAX_LENGTH : usize = 16;
    //  for variant in TauriEventName::all_variants() {
    //    let serialized = variant.to_str();
    //    assert!(serialized.len() > 0, "variant {:?} is too short", variant);
    //    assert!(serialized.len() <= MAX_LENGTH, "variant {:?} is too long", variant);
    //  }
    //}
  }
}
