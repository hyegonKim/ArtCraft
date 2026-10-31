/// Define an enum that is a closed set of known string values on the wire,
/// plus an `Other(String)` variant so unknown values coming *back* from the
/// API still parse instead of failing the whole response.
///
/// Generates `as_str`, `from_str_lossy`, `Display`, and serde impls that use
/// the wire string.
macro_rules! string_enum {
  (
    $(#[$meta:meta])*
    $name:ident {
      $(
        $(#[$variant_meta:meta])*
        $variant:ident => $wire:literal
      ),+ $(,)?
    }
  ) => {
    $(#[$meta])*
    #[derive(Clone, Debug, PartialEq, Eq, Hash)]
    pub enum $name {
      $(
        $(#[$variant_meta])*
        $variant,
      )+
      /// A value this client doesn't know about yet (only produced when
      /// parsing responses; carries the raw wire string).
      Other(String),
    }

    impl $name {
      /// The exact string sent to / received from the API.
      pub fn as_str(&self) -> &str {
        match self {
          $( Self::$variant => $wire, )+
          Self::Other(raw) => raw.as_str(),
        }
      }

      /// Parse a wire string; unknown values become [`Self::Other`].
      pub fn from_str_lossy(raw: &str) -> Self {
        match raw {
          $( $wire => Self::$variant, )+
          other => Self::Other(other.to_string()),
        }
      }

      /// Every known (non-`Other`) variant, in declaration order.
      pub fn known_variants() -> &'static [Self] {
        &[ $( Self::$variant, )+ ]
      }
    }

    impl std::fmt::Display for $name {
      fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
      }
    }

    impl serde::Serialize for $name {
      fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.as_str())
      }
    }

    impl<'de> serde::Deserialize<'de> for $name {
      fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let raw = String::deserialize(deserializer)?;
        Ok(Self::from_str_lossy(&raw))
      }
    }
  };
}

pub(crate) use string_enum;
