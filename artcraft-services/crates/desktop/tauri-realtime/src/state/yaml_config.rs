use filesys::recursively_find_file_by_name::recursively_find_file_by_name;
use std::fs::File;
use std::io::BufReader;
use std::path::Path;

const CONFIG_FILENAME : &str = "app_config.yaml";

/// Configuration read or derived from a YAML config file at startup
/// This is not meant to contain any runtime config values.
/// This may not be exhaustive.
#[derive(serde::Deserialize, serde::Serialize, Clone, Debug, Default)]
pub struct YamlConfig {
  pub image_height: Option<usize>,
  pub image_width: Option<usize>,
  pub scheduler_steps: Option<usize>,
  pub scheduler_samples: Option<usize>,
  pub seed: Option<u64>,
  pub cfg_scale: Option<f64>,

  /// Override the default location to store data on Linux
  pub linux_default_data_path: Option<String>,

  /// Override the default location to store data on Windows
  pub windows_default_data_path: Option<String>,
  
  /// Override the default location to store data on Mac
  pub mac_default_data_path: Option<String>,
}

impl YamlConfig {
  pub fn read_from_file<P: AsRef<Path>>(path: P) -> anyhow::Result<Self> {
    let file = File::open(path)?;
    let reader = BufReader::new(file);
    let config = serde_yaml::from_reader(reader)?;
    Ok(config)
  }

  pub fn load_from_config_file_recursive() -> YamlConfig {
    let maybe_path = recursively_find_file_by_name(CONFIG_FILENAME, "../../../", 5)
      .ok()
      .flatten();
    if let Some(path) = maybe_path {
      let path = path.canonicalize().unwrap_or(path);
      println!("Attempting to load configs from: {:?}", &path);
      if let Ok(config) = YamlConfig::read_from_file(path) {
        return config;
      }
    }
    println!("Couldn't load config file. Using default configs.");
    YamlConfig::default()
  }
}
