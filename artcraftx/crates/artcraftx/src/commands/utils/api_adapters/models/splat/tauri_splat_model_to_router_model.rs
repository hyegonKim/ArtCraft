use router::api::router_splat_model::RouterSplatModel;

use crate::commands::generate::generate_splat::request::TauriSplatModel;

/// Map TauriSplatModel to the router's RouterSplatModel.
pub fn tauri_splat_model_to_router_model(model: TauriSplatModel) -> RouterSplatModel {
  match model {
    TauriSplatModel::Marble0p1Mini => RouterSplatModel::Marble0p1Mini,
    TauriSplatModel::Marble0p1Plus => RouterSplatModel::Marble0p1Plus,
    TauriSplatModel::Marble1p0 => RouterSplatModel::Marble1p0,
    TauriSplatModel::Marble1p0Draft => RouterSplatModel::Marble1p0Draft,
    TauriSplatModel::Marble1p1 => RouterSplatModel::Marble1p1,
    TauriSplatModel::Marble1p1Plus => RouterSplatModel::Marble1p1Plus,
    TauriSplatModel::TripoSplat => RouterSplatModel::TripoSplat,
  }
}
