import { ListingModelBase } from "./ListingCommon.js";

// `models::configs::MeshModelConfig`
export interface MeshModelListing extends ListingModelBase {
  text_prompt_supported: boolean;
  image_input_supported: boolean;
  sketch_input_supported: boolean;
  multi_view_supported: boolean;
  mesh_input_supported: boolean;
  mesh_output_types: string[];
  polygon_types: string[];
  face_count_supported: boolean;
  pbr_supported: boolean;
  texture_toggle_supported: boolean;
  texture_quality_supported: boolean;
  geometry_quality_supported: boolean;
}
