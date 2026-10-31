use router::api::router_mesh_model::RouterMeshModel;

use crate::commands::generate::generate_mesh::request::TauriMeshModel;

/// Map TauriMeshModel to the router's RouterMeshModel.
pub fn tauri_mesh_model_to_router_model(model: TauriMeshModel) -> RouterMeshModel {
  match model {
    TauriMeshModel::Hunyuan3d2p0 => RouterMeshModel::Hunyuan3d2p0,
    TauriMeshModel::Hunyuan3d2p1 => RouterMeshModel::Hunyuan3d2p1,
    TauriMeshModel::Hunyuan3d3 => RouterMeshModel::Hunyuan3d3,
    TauriMeshModel::Hunyuan3d3Sketch => RouterMeshModel::Hunyuan3d3Sketch,
    TauriMeshModel::Hunyuan3d3p1Pro => RouterMeshModel::Hunyuan3d3p1Pro,
    TauriMeshModel::Hunyuan3d3p1Rapid => RouterMeshModel::Hunyuan3d3p1Rapid,
    TauriMeshModel::Hunyuan3d3p1Part => RouterMeshModel::Hunyuan3d3p1Part,
    TauriMeshModel::Hunyuan3d3p1SmartTopology => RouterMeshModel::Hunyuan3d3p1SmartTopology,
    TauriMeshModel::Tripo3dH3p1 => RouterMeshModel::Tripo3dH3p1,
    TauriMeshModel::MeshyV6 => RouterMeshModel::MeshyV6,
    TauriMeshModel::Rodin2p5Fast => RouterMeshModel::Rodin2p5Fast,
  }
}
