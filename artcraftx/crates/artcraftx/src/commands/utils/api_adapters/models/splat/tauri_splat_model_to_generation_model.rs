use crate::events::generation_events::common::GenerationModel;
use crate::commands::generate::generate_splat::request::TauriSplatModel;

/// Map TauriSplatModel to the frontend-event GenerationModel.
pub fn tauri_splat_model_to_generation_model(model: TauriSplatModel) -> GenerationModel {
  match model {
    TauriSplatModel::Marble0p1Mini => GenerationModel::WorldlabsMarble0p1Mini,
    TauriSplatModel::Marble0p1Plus => GenerationModel::WorldlabsMarble0p1Plus,
    TauriSplatModel::Marble1p0 => GenerationModel::Marble1p0,
    TauriSplatModel::Marble1p0Draft => GenerationModel::Marble1p0Draft,
    TauriSplatModel::Marble1p1 => GenerationModel::Marble1p1,
    TauriSplatModel::Marble1p1Plus => GenerationModel::Marble1p1Plus,
    TauriSplatModel::TripoSplat => GenerationModel::TripoSplat,
  }
}
