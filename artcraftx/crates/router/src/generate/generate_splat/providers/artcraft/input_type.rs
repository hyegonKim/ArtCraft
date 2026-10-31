/// How a splat request is driven, for pricing. Marble prices differ by
/// input kind (World Labs' published table), and Artcraft's flat prices
/// follow the same split.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InputType {
  Text,
  ImageNonPanorama,
  ImagePanorama,
  MultiImage,
  Video,
}
