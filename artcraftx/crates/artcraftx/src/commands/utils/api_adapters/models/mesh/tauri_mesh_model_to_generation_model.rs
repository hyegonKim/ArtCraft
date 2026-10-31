use crate::events::generation_events::common::GenerationModel;
use crate::commands::generate::generate_mesh::request::TauriMeshModel;

/// Map TauriMeshModel to the frontend-event GenerationModel.
pub fn tauri_mesh_model_to_generation_model(model: TauriMeshModel) -> GenerationModel {
  match model {
    TauriMeshModel::Hunyuan3d2p0 => GenerationModel::Hunyuan3d2_0,
    TauriMeshModel::Hunyuan3d2p1 => GenerationModel::Hunyuan3d2_1,
    TauriMeshModel::Hunyuan3d3 => GenerationModel::Hunyuan3d3,
    TauriMeshModel::Hunyuan3d3Sketch => GenerationModel::Hunyuan3d3Sketch,
    TauriMeshModel::Hunyuan3d3p1Pro => GenerationModel::Hunyuan3d3p1Pro,
    TauriMeshModel::Hunyuan3d3p1Rapid => GenerationModel::Hunyuan3d3p1Rapid,
    TauriMeshModel::Hunyuan3d3p1Part => GenerationModel::Hunyuan3d3p1Part,
    TauriMeshModel::Hunyuan3d3p1SmartTopology => GenerationModel::Hunyuan3d3p1SmartTopology,
    TauriMeshModel::Tripo3dH3p1 => GenerationModel::Tripo3dH3p1,
    TauriMeshModel::MeshyV6 => GenerationModel::MeshyV6,
    TauriMeshModel::Rodin2p5Fast => GenerationModel::Rodin2p5Fast,
  }
}
