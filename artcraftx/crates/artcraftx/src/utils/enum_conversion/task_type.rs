use crate::events::generation_events::common::GenerationAction;
use sqlite_identifiers::enums::task_type::TaskType;

pub fn to_generation_action(task_type: TaskType) -> GenerationAction {
  match task_type {
    TaskType::ImageGeneration => GenerationAction::GenerateImage,
    TaskType::VideoGeneration => GenerationAction::GenerateVideo,
    TaskType::AudioGeneration => GenerationAction::GenerateAudio,
    TaskType::MeshGeneration => GenerationAction::ImageTo3d,
    TaskType::SplatGeneration => GenerationAction::GenerateGaussian,
  }
}
