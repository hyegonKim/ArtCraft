use crate::state::app_dir::AppDataRoot;
use crate::state::os_platform::OsPlatform;
use crate::state::yaml_config::YamlConfig;
use crate::stubs::model_config::ModelConfig;

const DEFAULT_SD_IMAGE_WIDTH: usize = 1024;
const DEFAULT_SD_IMAGE_HEIGHT: usize = 1024;


/// A central place to configure app-wide details.
pub struct AppConfig {
  pub image_height: usize,
  pub image_width: usize,
  pub scheduler_steps: usize,
  pub scheduler_samples: usize,
  
  pub seed: Option<u64>,

  pub cfg_scale: Option<f64>,
  
  /// Root location for application data.
  pub app_data_root: AppDataRoot,
  
  pub model_config: ModelConfig,
}

impl AppConfig {
  
  pub fn init() -> anyhow::Result<Self> {
    let yaml_configs = YamlConfig::load_from_config_file_recursive();
    
    println!("Configs: {:?}", yaml_configs);
    
    let os_platform = OsPlatform::get();

    let default_data_root = match os_platform {
      OsPlatform::Linux => yaml_configs.linux_default_data_path.as_deref(),
      OsPlatform::MacOs => yaml_configs.mac_default_data_path.as_deref(),
      OsPlatform::Windows => yaml_configs.windows_default_data_path.as_deref(),
    };
    
    println!("Possible user-defined data root: {:?}", default_data_root);

    let app_data_root = match default_data_root {
      None => AppDataRoot::create_default()?,
      Some(path) => AppDataRoot::create_existing(path)?,
    };

    println!("App data root: {:?}", app_data_root.path());

    Ok(Self {
      image_height: yaml_configs.image_height.unwrap_or(DEFAULT_SD_IMAGE_HEIGHT),
      image_width: yaml_configs.image_width.unwrap_or(DEFAULT_SD_IMAGE_WIDTH),
      scheduler_steps: yaml_configs.scheduler_steps.unwrap_or(4),
      scheduler_samples: yaml_configs.scheduler_samples.unwrap_or(15),
      seed: yaml_configs.seed,
      cfg_scale: yaml_configs.cfg_scale,
      app_data_root,
      model_config: ModelConfig::init()?,
    })
  }
}
