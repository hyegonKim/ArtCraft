use std::collections::BTreeSet;

use crate::enums::enum_error::EnumError;
#[cfg(test)]
use strum::EnumCount;
#[cfg(test)]
use strum::EnumIter;

/// Which frontend page/component invoked a Tauri command.
/// These values are stored in the tasks database, so keep them short-ish.
#[cfg_attr(test, derive(EnumIter, EnumCount))]
#[derive(Clone, Copy, PartialEq, Eq, Hash, Ord, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TauriCommandCaller {
  /// The image generation page
  ImagePage,
  /// The video generation page
  VideoPage,
  /// The audio generation page
  AudioPage,
  /// The mesh (3D object) generation page
  MeshPage,
  /// The splat (3D world) generation page
  SplatPage,
}

impl_enum_display_and_debug_using_to_str!(TauriCommandCaller);

// NB: We can derive `sqlx::Type` instead of using `impl_mysql_enum_coders`

impl TauriCommandCaller {
  pub fn to_str(&self) -> &'static str {
    match self {
      Self::ImagePage => "image_page",
      Self::VideoPage => "video_page",
      Self::AudioPage => "audio_page",
      Self::MeshPage => "mesh_page",
      Self::SplatPage => "splat_page",
    }
  }

  pub fn from_str(value: &str) -> Result<Self, EnumError> {
    match value {
      "image_page" => Ok(Self::ImagePage),
      "video_page" => Ok(Self::VideoPage),
      "audio_page" => Ok(Self::AudioPage),
      "mesh_page" => Ok(Self::MeshPage),
      "splat_page" => Ok(Self::SplatPage),
      _ => Err(EnumError::CouldNotConvertFromString(value.to_string())),
    }
  }

  pub fn all_variants() -> BTreeSet<Self> {
    // NB: BTreeSet is sorted
    // NB: BTreeSet::from() isn't const, but not worth using LazyStatic, etc.
    BTreeSet::from([
      Self::ImagePage,
      Self::VideoPage,
      Self::AudioPage,
      Self::MeshPage,
      Self::SplatPage,
    ])
  }
}

#[cfg(test)]
mod tests {
  use crate::enums::enum_error::EnumError;
  use crate::enums::tauri_command_caller::TauriCommandCaller;
  use crate::test_helpers::assert_serialization;

  mod explicit_checks {
    use super::*;

    #[test]
    fn test_serialization() {
      assert_serialization(TauriCommandCaller::ImagePage, "image_page");
      assert_serialization(TauriCommandCaller::VideoPage, "video_page");
      assert_serialization(TauriCommandCaller::AudioPage, "audio_page");
      assert_serialization(TauriCommandCaller::MeshPage, "mesh_page");
      assert_serialization(TauriCommandCaller::SplatPage, "splat_page");
    }

    #[test]
    fn to_str() {
      assert_eq!(TauriCommandCaller::ImagePage.to_str(), "image_page");
      assert_eq!(TauriCommandCaller::VideoPage.to_str(), "video_page");
      assert_eq!(TauriCommandCaller::AudioPage.to_str(), "audio_page");
      assert_eq!(TauriCommandCaller::MeshPage.to_str(), "mesh_page");
      assert_eq!(TauriCommandCaller::SplatPage.to_str(), "splat_page");
    }

    #[test]
    fn from_str() {
      assert_eq!(TauriCommandCaller::from_str("image_page").unwrap(), TauriCommandCaller::ImagePage);
      assert_eq!(TauriCommandCaller::from_str("video_page").unwrap(), TauriCommandCaller::VideoPage);
      assert_eq!(TauriCommandCaller::from_str("audio_page").unwrap(), TauriCommandCaller::AudioPage);
      assert_eq!(TauriCommandCaller::from_str("mesh_page").unwrap(), TauriCommandCaller::MeshPage);
      assert_eq!(TauriCommandCaller::from_str("splat_page").unwrap(), TauriCommandCaller::SplatPage);
    }

    #[test]
    fn from_str_err() {
      let result = TauriCommandCaller::from_str("asdf");
      assert!(result.is_err());
      if let Err(EnumError::CouldNotConvertFromString(value)) = result {
        assert_eq!(value, "asdf");
      } else {
        panic!("Expected EnumError::CouldNotConvertFromString");
      }
    }

    #[test]
    fn all_variants() {
      let mut variants = TauriCommandCaller::all_variants();
      assert_eq!(variants.len(), 5);
      assert_eq!(variants.pop_first(), Some(TauriCommandCaller::ImagePage));
      assert_eq!(variants.pop_first(), Some(TauriCommandCaller::VideoPage));
      assert_eq!(variants.pop_first(), Some(TauriCommandCaller::AudioPage));
      assert_eq!(variants.pop_first(), Some(TauriCommandCaller::MeshPage));
      assert_eq!(variants.pop_first(), Some(TauriCommandCaller::SplatPage));
      assert_eq!(variants.pop_first(), None);
    }
  }

  mod mechanical_checks {
    use super::*;
    use strum::IntoEnumIterator;

    #[test]
    fn variant_length() {
      assert_eq!(TauriCommandCaller::all_variants().len(), TauriCommandCaller::iter().len());
    }

    #[test]
    fn round_trip() {
      for variant in TauriCommandCaller::all_variants() {
        // Test to_str(), from_str(), Display, and Debug.
        assert_eq!(variant, TauriCommandCaller::from_str(variant.to_str()).unwrap());
        assert_eq!(variant, TauriCommandCaller::from_str(&format!("{}", variant)).unwrap());
        assert_eq!(variant, TauriCommandCaller::from_str(&format!("{:?}", variant)).unwrap());
      }
    }

    //#[test]
    //fn serialized_length_ok_for_database() {
    //  const MAX_LENGTH : usize = 16;
    //  for variant in TauriCommandCaller::all_variants() {
    //    let serialized = variant.to_str();
    //    assert!(serialized.len() > 0, "variant {:?} is too short", variant);
    //    assert!(serialized.len() <= MAX_LENGTH, "variant {:?} is too long", variant);
    //  }
    //}
  }
}
