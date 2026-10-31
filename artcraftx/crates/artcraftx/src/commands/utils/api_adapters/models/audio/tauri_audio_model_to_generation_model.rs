use crate::events::generation_events::common::GenerationModel;
use crate::commands::generate::generate_audio::request::TauriAudioModel;

/// Map TauriAudioModel to the frontend-event GenerationModel.
pub fn tauri_audio_model_to_generation_model(model: TauriAudioModel) -> GenerationModel {
  match model {
    TauriAudioModel::SunoMusic => GenerationModel::SunoMusic,
    TauriAudioModel::SunoRemix => GenerationModel::SunoRemix,
    TauriAudioModel::SunoSounds => GenerationModel::SunoSounds,
    TauriAudioModel::SunoSample => GenerationModel::SunoSample,
    TauriAudioModel::SeedAudio1p0 => GenerationModel::SeedAudio1p0,
  }
}
