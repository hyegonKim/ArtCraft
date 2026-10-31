use artcraft_client::enums::common::generation::common_model_type::CommonModelType;

use crate::commands::generate::generate_image::tauri_image_model::TauriImageModel;

/// Map TauriImageModel to the legacy CommonModelType (enums crate).
pub fn tauri_image_model_to_common_model_type(model: TauriImageModel) -> CommonModelType {
  match model {
    TauriImageModel::Flux1Dev => CommonModelType::Flux1Dev,
    TauriImageModel::Flux1Schnell => CommonModelType::Flux1Schnell,
    TauriImageModel::FluxPro11 => CommonModelType::FluxPro11,
    TauriImageModel::FluxPro11Ultra => CommonModelType::FluxPro11Ultra,
    TauriImageModel::GrokImage => CommonModelType::GrokImagineImage,
    TauriImageModel::GrokImagineImageQuality => CommonModelType::GrokImagineImageQuality,
    TauriImageModel::Recraft3 => CommonModelType::Recraft3,
    TauriImageModel::GptImage1 => CommonModelType::GptImage1,
    TauriImageModel::GptImage1p5 => CommonModelType::GptImage1p5,
    TauriImageModel::GptImage2 => CommonModelType::GptImage2,
    TauriImageModel::NanoBanana => CommonModelType::NanoBanana,
    TauriImageModel::NanoBanana2 => CommonModelType::NanoBanana2,
    TauriImageModel::NanoBanana2Lite => CommonModelType::NanoBanana2, // NB: no dedicated legacy type; nearest.
    TauriImageModel::NanoBananaPro => CommonModelType::NanoBananaPro,
    TauriImageModel::Seedream4 => CommonModelType::Seedream4,
    TauriImageModel::Seedream4p5 => CommonModelType::Seedream4p5,
    TauriImageModel::Seedream5Lite => CommonModelType::Seedream5Lite,
    TauriImageModel::Seedream5p0Pro => CommonModelType::Seedream5p0Pro,
    TauriImageModel::Seedream5p0ProUltra => CommonModelType::Seedream5p0ProUltra,
    TauriImageModel::Midjourney7 => CommonModelType::Midjourney7,
    TauriImageModel::Midjourney7Niji => CommonModelType::Midjourney7Niji,
    TauriImageModel::Midjourney8 => CommonModelType::Midjourney8,
    TauriImageModel::FluxProKontextMax => CommonModelType::FluxProKontextMax,
    TauriImageModel::QwenEdit2511Angles => CommonModelType::QwenEdit2511Angles,
    TauriImageModel::Flux2LoraAngles => CommonModelType::Flux2LoraAngles,
    TauriImageModel::FluxDevJuggernaut => CommonModelType::FluxDevJuggernaut,
    TauriImageModel::FluxPro1 => CommonModelType::FluxPro1,
  }
}
