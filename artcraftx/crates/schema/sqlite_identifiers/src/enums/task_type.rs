use std::collections::BTreeSet;

use crate::enums::enum_error::EnumError;
#[cfg(test)]
use strum::EnumCount;
#[cfg(test)]
use strum::EnumIter;

#[cfg_attr(test, derive(EnumIter, EnumCount))]
#[derive(Clone, Copy, PartialEq, Eq, Hash, Ord, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskType {
  ImageGeneration,
  VideoGeneration,
  AudioGeneration,
  MeshGeneration,
  SplatGeneration,
}

impl_enum_display_and_debug_using_to_str!(TaskType);
//impl_mysql_enum_coders!(TaskType);
//impl_mysql_from_row!(TaskType);

// NB: We can derive `sqlx::Type` instead of using `impl_mysql_enum_coders`

impl TaskType {
  pub fn to_str(&self) -> &'static str {
    match self {
      Self::ImageGeneration => "image_generation",
      Self::VideoGeneration => "video_generation",
      Self::AudioGeneration => "audio_generation",
      Self::MeshGeneration => "mesh_generation",
      Self::SplatGeneration => "splat_generation",
    }
  }

  pub fn from_str(value: &str) -> Result<Self, EnumError> {
    match value {
      "image_generation" => Ok(Self::ImageGeneration),
      "video_generation" => Ok(Self::VideoGeneration),
      "audio_generation" => Ok(Self::AudioGeneration),
      "mesh_generation" => Ok(Self::MeshGeneration),
      "splat_generation" => Ok(Self::SplatGeneration),
      _ => Err(EnumError::CouldNotConvertFromString(value.to_string())),
    }
  }

  pub fn all_variants() -> BTreeSet<Self> {
    // NB: BTreeSet is sorted
    // NB: BTreeSet::from() isn't const, but not worth using LazyStatic, etc.
    BTreeSet::from([
      Self::ImageGeneration,
      Self::VideoGeneration,
      Self::AudioGeneration,
      Self::MeshGeneration,
      Self::SplatGeneration,
    ])
  }
}

#[cfg(test)]
mod tests {
  use crate::enums::task_type::TaskType;
  use crate::test_helpers::assert_serialization;

  mod explicit_checks {
    use super::*;
    use crate::enums::enum_error::EnumError;

    #[test]
    fn test_serialization() {
      assert_serialization(TaskType::ImageGeneration, "image_generation");
      assert_serialization(TaskType::VideoGeneration, "video_generation");
      assert_serialization(TaskType::AudioGeneration, "audio_generation");
      assert_serialization(TaskType::MeshGeneration, "mesh_generation");
      assert_serialization(TaskType::SplatGeneration, "splat_generation");
    }

    #[test]
    fn to_str() {
      assert_eq!(TaskType::ImageGeneration.to_str(), "image_generation");
      assert_eq!(TaskType::VideoGeneration.to_str(), "video_generation");
      assert_eq!(TaskType::AudioGeneration.to_str(), "audio_generation");
      assert_eq!(TaskType::MeshGeneration.to_str(), "mesh_generation");
      assert_eq!(TaskType::SplatGeneration.to_str(), "splat_generation");
    }

    #[test]
    fn from_str() {
      assert_eq!(TaskType::from_str("image_generation").unwrap(), TaskType::ImageGeneration);
      assert_eq!(TaskType::from_str("video_generation").unwrap(), TaskType::VideoGeneration);
      assert_eq!(TaskType::from_str("audio_generation").unwrap(), TaskType::AudioGeneration);
      assert_eq!(TaskType::from_str("mesh_generation").unwrap(), TaskType::MeshGeneration);
      assert_eq!(TaskType::from_str("splat_generation").unwrap(), TaskType::SplatGeneration);
    }
    
    #[test]
    fn from_str_err() {
      let result = TaskType::from_str("asdf");
      assert!(result.is_err());
      if let Err(EnumError::CouldNotConvertFromString(value)) = result {
        assert_eq!(value, "asdf");
      } else {
        panic!("Expected EnumError::CouldNotConvertFromString");
      }
    }

    #[test]
    fn all_variants() {
      let mut variants = TaskType::all_variants();
      assert_eq!(variants.len(), 5);
      assert_eq!(variants.pop_first(), Some(TaskType::ImageGeneration));
      assert_eq!(variants.pop_first(), Some(TaskType::VideoGeneration));
      assert_eq!(variants.pop_first(), Some(TaskType::AudioGeneration));
      assert_eq!(variants.pop_first(), Some(TaskType::MeshGeneration));
      assert_eq!(variants.pop_first(), Some(TaskType::SplatGeneration));
      assert_eq!(variants.pop_first(), None);
    }
  }

  mod mechanical_checks {
    use super::*;

    #[test]
    fn variant_length() {
      use strum::IntoEnumIterator;
      assert_eq!(TaskType::all_variants().len(), TaskType::iter().len());
    }

    #[test]
    fn round_trip() {
      for variant in TaskType::all_variants() {
        // Test to_str(), from_str(), Display, and Debug.
        assert_eq!(variant, TaskType::from_str(variant.to_str()).unwrap());
        assert_eq!(variant, TaskType::from_str(&format!("{}", variant)).unwrap());
        assert_eq!(variant, TaskType::from_str(&format!("{:?}", variant)).unwrap());
      }
    }
  }
}
