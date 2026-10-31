use router::api::router_audio_model::RouterAudioModel;

use crate::commands::generate::generate_audio::request::TauriAudioModel;

/// Map TauriAudioModel to the router's RouterAudioModel.
pub fn tauri_audio_model_to_router_model(model: TauriAudioModel) -> RouterAudioModel {
  match model {
    TauriAudioModel::SunoMusic => RouterAudioModel::SunoMusic,
    TauriAudioModel::SunoRemix => RouterAudioModel::SunoRemix,
    TauriAudioModel::SunoSounds => RouterAudioModel::SunoSounds,
    TauriAudioModel::SunoSample => RouterAudioModel::SunoSample,
    TauriAudioModel::SeedAudio1p0 => RouterAudioModel::SeedAudio1p0,
  }
}
