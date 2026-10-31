// NOTE: These are defined in Rust (as the source of truth) and duplicated in the frontend.
// In the future, we should use code gen (protobufs or similar) to keep the two sides in sync.

export enum TaskType {
  ImageGeneration = "image_generation",
  VideoGeneration = "video_generation",
  AudioGeneration = "audio_generation",
  MeshGeneration = "mesh_generation",
  SplatGeneration = "splat_generation",
}
