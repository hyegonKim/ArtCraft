use crate::state::app_preferences::settings::app_sound_file::{optional_sound, AppSoundFile};
use serde_derive::{Deserialize, Serialize};

/// Which sound (if any) plays for each app event.
///
/// `None` means silent for that event and is stored as `"none"` (see
/// [`optional_sound`]). Missing fields in an older preferences file fall back
/// to the defaults.
#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AppSoundPreferences {
  /// Master switch: play sounds on events at all.
  pub play_sounds: bool,

  /// A file was deleted.
  #[serde(with = "optional_sound")]
  pub delete_file: Option<AppSoundFile>,

  /// A generation was accepted by the provider (image enqueue can be async).
  #[serde(with = "optional_sound")]
  pub enqueue_success: Option<AppSoundFile>,

  /// A generation was rejected on enqueue.
  #[serde(with = "optional_sound")]
  pub enqueue_failure: Option<AppSoundFile>,

  /// A generation finished.
  #[serde(with = "optional_sound")]
  pub generation_success: Option<AppSoundFile>,

  /// A generation failed.
  #[serde(with = "optional_sound")]
  pub generation_failure: Option<AppSoundFile>,
}

/// The app events that can play a sound; one per field of
/// [`AppSoundPreferences`]. Serialized as the field names.
#[derive(Debug, Clone, Copy, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AppSoundEvent {
  DeleteFile,
  EnqueueSuccess,
  EnqueueFailure,
  GenerationSuccess,
  GenerationFailure,
}

impl AppSoundPreferences {
  pub fn get(&self, event: AppSoundEvent) -> Option<&AppSoundFile> {
    match event {
      AppSoundEvent::DeleteFile => self.delete_file.as_ref(),
      AppSoundEvent::EnqueueSuccess => self.enqueue_success.as_ref(),
      AppSoundEvent::EnqueueFailure => self.enqueue_failure.as_ref(),
      AppSoundEvent::GenerationSuccess => self.generation_success.as_ref(),
      AppSoundEvent::GenerationFailure => self.generation_failure.as_ref(),
    }
  }

  /// The out-of-the-box sound for an event (from the `Default` impl).
  pub fn default_for(event: AppSoundEvent) -> Option<AppSoundFile> {
    Self::default().get(event).cloned()
  }

  /// Put the event back to its out-of-the-box sound.
  pub fn reset_to_default(&mut self, event: AppSoundEvent) {
    self.set(event, Self::default_for(event));
  }

  /// `None` silences the event.
  pub fn set(&mut self, event: AppSoundEvent, sound: Option<AppSoundFile>) {
    let slot = match event {
      AppSoundEvent::DeleteFile => &mut self.delete_file,
      AppSoundEvent::EnqueueSuccess => &mut self.enqueue_success,
      AppSoundEvent::EnqueueFailure => &mut self.enqueue_failure,
      AppSoundEvent::GenerationSuccess => &mut self.generation_success,
      AppSoundEvent::GenerationFailure => &mut self.generation_failure,
    };
    *slot = sound;
  }
}

impl Default for AppSoundPreferences {
  fn default() -> Self {
    Self {
      play_sounds: true,
      delete_file: Some(AppSoundFile::Trash),
      enqueue_success: Some(AppSoundFile::Done),
      enqueue_failure: Some(AppSoundFile::SpikeThrow),
      generation_success: Some(AppSoundFile::SpecialFlower),
      generation_failure: Some(AppSoundFile::Crumble),
    }
  }
}
