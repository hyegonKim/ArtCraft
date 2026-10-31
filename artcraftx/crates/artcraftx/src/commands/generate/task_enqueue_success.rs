use crate::events::generation_events::common::{GenerationAction, GenerationModel, GenerationServiceProvider};
use crate::utils::enum_conversion::generation_source::to_generation_service_provider;
use crate::state::database::task_database::TaskDatabase;
use core_types::enums::generation_source::GenerationSource;
use sqlite_identifiers::enums::task_model_type::TaskModelType;
use sqlite_identifiers::enums::task_status::TaskStatus;
use sqlite_identifiers::enums::task_type::TaskType;
use sqlite_identifiers::enums::tauri_command_caller::TauriCommandCaller;
use sqlite_database::error::SqliteTasksError;
use sqlite_database::queries::create::create_task::{create_task, CreateTaskArgs};
use sqlite_identifiers::ids::prompt_token::PromptToken;
use sqlite_identifiers::ids::task_id::TaskId;

pub struct TaskEnqueueSuccess {
  pub task_type: TaskType,
  pub model: Option<GenerationModel>,
  pub provider: GenerationSource,
  pub provider_job_id: Option<String>,
  /// Whether the job produces more than one file.
  pub is_batch_generation: bool,
  pub maybe_queue_status_url: Option<String>,
  pub maybe_queue_response_url: Option<String>,
  pub maybe_prompt_token: Option<PromptToken>,
}

impl TaskEnqueueSuccess{
  pub fn to_frontend_event_action(&self) -> GenerationAction {
    match self.task_type {
      TaskType::ImageGeneration => GenerationAction::GenerateImage,
      TaskType::SplatGeneration => GenerationAction::GenerateGaussian,
      TaskType::VideoGeneration => GenerationAction::GenerateVideo,
      TaskType::AudioGeneration => GenerationAction::GenerateAudio,
      TaskType::MeshGeneration => GenerationAction::ImageTo3d,
    }
  }
  
  pub fn to_frontend_event_service(&self) -> GenerationServiceProvider {
    to_generation_service_provider(self.provider)
  }
  
  pub async fn insert_into_task_database(&self, task_database: &TaskDatabase) -> Result<TaskId, SqliteTasksError> {
    self.insert_into_task_database_with_frontend_payload(
      task_database,
      None,
      None,
      None,
    ).await
  }

  // TODO: This belongs somewhere else, not as a method of an event struct.
  pub async fn insert_into_task_database_with_frontend_payload(
    &self,
    task_database: &TaskDatabase,
    frontend_caller: Option<TauriCommandCaller>,
    frontend_subscriber_id: Option<&str>,
    frontend_subscriber_payload: Option<&str>,
  ) -> Result<TaskId, SqliteTasksError> {
    // TODO: Move this mapping elsewhere, or remove the other models.
    let model_type = match self.model {
      None => None,
      Some(GenerationModel::Flux1Dev) => Some(TaskModelType::Flux1Dev),
      Some(GenerationModel::FluxDevJuggernaut) => Some(TaskModelType::FluxDevJuggernaut),
      Some(GenerationModel::Flux1Schnell) => Some(TaskModelType::Flux1Schnell),
      Some(GenerationModel::FluxPro1) => Some(TaskModelType::FluxPro1), // NB: This is for inpainting.
      Some(GenerationModel::FluxPro11) => Some(TaskModelType::FluxPro11),
      Some(GenerationModel::FluxPro11Ultra) => Some(TaskModelType::FluxPro11Ultra),
      Some(GenerationModel::FluxProKontextMax) => Some(TaskModelType::FluxProKontextMax),
      Some(GenerationModel::Gemini25Flash) => Some(TaskModelType::Gemini25Flash),
      Some(GenerationModel::NanoBanana) => Some(TaskModelType::NanoBanana),
      Some(GenerationModel::NanoBanana2) => Some(TaskModelType::NanoBanana2),
      Some(GenerationModel::NanoBanana2Lite) => Some(TaskModelType::NanoBanana2Lite),
      Some(GenerationModel::NanoBananaPro) => Some(TaskModelType::NanoBananaPro),
      Some(GenerationModel::GptImage1) => Some(TaskModelType::GptImage1),
      Some(GenerationModel::GptImage1p5) => Some(TaskModelType::GptImage1p5),
      Some(GenerationModel::GptImage2) => Some(TaskModelType::GptImage2),
      Some(GenerationModel::Seedream4) => Some(TaskModelType::Seedream4),
      Some(GenerationModel::Seedream4p5) => Some(TaskModelType::Seedream4p5),
      Some(GenerationModel::Seedream5Lite) => Some(TaskModelType::Seedream5Lite),
      Some(GenerationModel::Seedream5p0Pro) => Some(TaskModelType::Seedream5p0Pro),
      Some(GenerationModel::Seedream5p0ProUltra) => Some(TaskModelType::Seedream5p0ProUltra),
      Some(GenerationModel::QwenEdit2511Angles) => Some(TaskModelType::QwenEdit2511Angles),
      Some(GenerationModel::Flux2LoraAngles) => Some(TaskModelType::Flux2LoraAngles),
      Some(GenerationModel::GrokImage) => Some(TaskModelType::GrokImage),
      Some(GenerationModel::Recraft3) => Some(TaskModelType::Recraft3),
      Some(GenerationModel::GrokVideo) => Some(TaskModelType::GrokVideo),
      Some(GenerationModel::GrokImagineVideo1p5) => Some(TaskModelType::GrokImagineVideo1p5),
      Some(GenerationModel::Kling21Pro) => Some(TaskModelType::Kling21Pro),
      Some(GenerationModel::Kling21Master) => Some(TaskModelType::Kling21Master),
      Some(GenerationModel::Kling2p5TurboPro) => Some(TaskModelType::Kling2p5TurboPro),
      Some(GenerationModel::Kling2p6Pro) => Some(TaskModelType::Kling2p6Pro),
      Some(GenerationModel::Kling3p0Standard) => Some(TaskModelType::Kling3p0Standard),
      Some(GenerationModel::Kling3p0Pro) => Some(TaskModelType::Kling3p0Pro),
      Some(GenerationModel::HappyHorse1p0) => Some(TaskModelType::HappyHorse1p0),
      Some(GenerationModel::Seedance10Lite) => Some(TaskModelType::Seedance10Lite),
      Some(GenerationModel::Seedance1p5Pro) => Some(TaskModelType::Seedance1p5Pro),
      Some(GenerationModel::Seedance2p0) => Some(TaskModelType::Seedance2p0),
      Some(GenerationModel::Seedance2p0Fast) => Some(TaskModelType::Seedance2p0Fast),
      Some(GenerationModel::Seedance2p0Mini) => Some(TaskModelType::Seedance2p0Mini),
      Some(GenerationModel::Seedance2p5) => Some(TaskModelType::Seedance2p5),
      Some(GenerationModel::Seedance2p5Edit) => Some(TaskModelType::Seedance2p5Edit),
      Some(GenerationModel::MinimaxH3) => Some(TaskModelType::MinimaxH3),
      Some(GenerationModel::Sora2) => Some(TaskModelType::Sora2),
      Some(GenerationModel::Sora2Pro) => Some(TaskModelType::Sora2Pro),
      Some(GenerationModel::Veo2) => Some(TaskModelType::Veo2),
      Some(GenerationModel::Veo3) => Some(TaskModelType::Veo3),
      Some(GenerationModel::Veo3p1) => Some(TaskModelType::Veo3p1),
      Some(GenerationModel::Veo3p1Fast) => Some(TaskModelType::Veo3p1Fast),
      Some(GenerationModel::Veo3Fast) => Some(TaskModelType::Veo3Fast),
      Some(GenerationModel::Veo3p1Lite) => Some(TaskModelType::Veo3p1Lite),
      Some(GenerationModel::ViduQ3) => Some(TaskModelType::ViduQ3),
      Some(GenerationModel::ViduQ3Turbo) => Some(TaskModelType::ViduQ3Turbo),
      Some(GenerationModel::Hunyuan3d2_0) => Some(TaskModelType::Hunyuan3d2_0),
      Some(GenerationModel::Hunyuan3d2_1) => Some(TaskModelType::Hunyuan3d2_1),
      Some(GenerationModel::Hunyuan3d3) => Some(TaskModelType::Hunyuan3d3),
      Some(GenerationModel::Hunyuan3d3Sketch) => Some(TaskModelType::Hunyuan3d3Sketch),
      Some(GenerationModel::Hunyuan3d3p1Pro) => Some(TaskModelType::Hunyuan3d3p1Pro),
      Some(GenerationModel::Hunyuan3d3p1Rapid) => Some(TaskModelType::Hunyuan3d3p1Rapid),
      Some(GenerationModel::Hunyuan3d3p1Part) => Some(TaskModelType::Hunyuan3d3p1Part),
      Some(GenerationModel::Hunyuan3d3p1SmartTopology) => Some(TaskModelType::Hunyuan3d3p1SmartTopology),
      Some(GenerationModel::Tripo3dH3p1) => Some(TaskModelType::Tripo3dH3p1),
      Some(GenerationModel::MeshyV6) => Some(TaskModelType::MeshyV6),
      Some(GenerationModel::Rodin2p5Fast) => Some(TaskModelType::Rodin2p5Fast),
      Some(GenerationModel::Marble1p0) => Some(TaskModelType::Marble1p0),
      Some(GenerationModel::Marble1p0Draft) => Some(TaskModelType::Marble1p0Draft),
      Some(GenerationModel::Marble1p1) => Some(TaskModelType::Marble1p1),
      Some(GenerationModel::Marble1p1Plus) => Some(TaskModelType::Marble1p1Plus),
      Some(GenerationModel::TripoSplat) => Some(TaskModelType::TripoSplat),
      Some(GenerationModel::SunoMusic) => Some(TaskModelType::SunoMusic),
      Some(GenerationModel::SunoRemix) => Some(TaskModelType::SunoRemix),
      Some(GenerationModel::SunoSounds) => Some(TaskModelType::SunoSounds),
      Some(GenerationModel::SunoSample) => Some(TaskModelType::SunoSample),
      Some(GenerationModel::SeedAudio1p0) => Some(TaskModelType::SeedAudio1p0),
      Some(GenerationModel::WorldlabsMarble) => Some(TaskModelType::WorldlabsMarble),
      Some(GenerationModel::WorldlabsMarble0p1Mini) => Some(TaskModelType::WorldlabsMarble0p1Mini),
      Some(GenerationModel::WorldlabsMarble0p1Plus) => Some(TaskModelType::WorldlabsMarble0p1Plus),
      Some(GenerationModel::Midjourney) => Some(TaskModelType::Midjourney), // NB: This is a generic Midjourney model, version unknown.
      Some(GenerationModel::Midjourney7) => Some(TaskModelType::Midjourney7),
      Some(GenerationModel::Midjourney7Niji) => Some(TaskModelType::Midjourney7Niji),
      Some(GenerationModel::Midjourney8) => Some(TaskModelType::Midjourney8),

      // TODO: These seem wrong -
      Some(GenerationModel::Kling1_6) => Some(TaskModelType::Kling16Pro), // NB: `VideoModel::Kling16Pro`.
      Some(GenerationModel::Kling2_0) => None, // TODO: unused elsewhere?
    };

    create_task(CreateTaskArgs {
      db: task_database.get_connection(),
      status: TaskStatus::Pending,
      task_type: self.task_type,
      model_type,
      provider: self.provider,
      provider_job_id: self.provider_job_id.as_deref(),
      is_batch_generation: self.is_batch_generation,
      queue_status_url: self.maybe_queue_status_url.as_deref(),
      queue_response_url: self.maybe_queue_response_url.as_deref(),
      prompt_token: self.maybe_prompt_token.as_ref(),
      frontend_caller,
      frontend_subscriber_id,
      frontend_subscriber_payload,
    }).await
  }
}