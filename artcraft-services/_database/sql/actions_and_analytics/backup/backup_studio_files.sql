
-- STUDIO_maybe_media_subtype_not_null.csv
select token
from media_files
where maybe_media_subtype IS NOT NULL;

-- STUDIO_dimensional_media_class.csv
select token
from media_files
where media_class = 'dimensional';

-- STUDIO_maybe_animation_type_not_null.csv
select token
from media_files
where maybe_animation_type IS NOT NULL;

-- STUDIO_IMPORTANT_maybe_engine_category_not_null.csv
select token
from media_files
where maybe_engine_category IS NOT NULL;

-- STUDIO_IMPORTANT_scene_source_token_not_null.csv
select distinct maybe_scene_source_media_file_token
from media_files
where maybe_scene_source_media_file_token IS NOT NULL
group by maybe_scene_source_media_file_token;

-- STUDIO_media_file_origin_categories.csv
select token
from media_files
where origin_category IN ('processed', 'studio', 'story_engine');

-- STUDIO_media_file_studio_asset_product_category.csv
select token
from media_files
where origin_product_category IN ('mocap', 'studio', 'workflow');

-- VIDEO_media_file_video_product_category.csv
select token
from media_files
where origin_product_category IN ('face_mirror', 'face_fusion');

-- VIDEO_media_file_video_origin_model_type.csv
select token
from media_files
where maybe_origin_model_type IN ('comfy_ui', 'face_fusion', 'live_portrait');

-- STUDIO_media_file_mocap_model_type.csv
select token
from media_files
where maybe_origin_model_type IN ('mocap_net');

select token
from media_files
where media_type IN ('bvh', 'fbx', 'gif', 'glb', 'gltf', 'jpg', 'pmd', 'pmx', 'png', 'scene_json', 'scene_ron', 'vmd');

select token
from media_files
where media_type IN ('mp4', 'video');
