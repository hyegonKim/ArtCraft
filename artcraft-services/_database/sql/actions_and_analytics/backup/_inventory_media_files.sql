-- TODO: TOP maybe_creator_user_token

-- TODO: maybe_style_transfer_source_media_file_token (original videos)

-- TODO: Later: media_type = (mp4, video)

-- origin_product_category
select distinct origin_category,
count(*)
from media_files
group by origin_category;

-- +-----------------+----------+
-- | origin_category | count(*) |
-- +-----------------+----------+
-- | inference       | 40933502 |
-- | processed       |      365 |
-- | story_engine    |      688 |
-- | studio          |      148 |
-- | upload          |   796516 |
-- +-----------------+----------+
-- 5 rows in set (54.69 sec) -- TODO [QUERY DONE]: Get origin_category = `story_engine`, `studio`, and `processed` (???)


-- origin_product_category
select distinct origin_product_category,
count(*)
from media_files
group by origin_product_category;

-- +-------------------------+----------+
-- | origin_product_category | count(*) |
-- +-------------------------+----------+
-- | face_animator           |   145770 |
-- | face_fusion             |     1524 |
-- | face_mirror             |    12743 |
-- | image_gen               |   450196 |
-- | mocap                   |      436 |
-- | studio                  |      148 |
-- | tts                     | 33757597 |
-- | unknown                 |    94182 |
-- | voice_conversion        |  6664633 |
-- | workflow                |    46011 |
-- | zs_voice                |   558212 |
-- +-------------------------+----------+
-- 11 rows in set (45.88 sec)
-- TODO [QUERIES DONE(2)]: Get origin_product_category = `mocap`, `studio`, `workflow`, `face_mirror`, `face_fusion`


-- maybe_origin_model_type (+token)
select distinct maybe_origin_model_type,
count(*)
from media_files
group by maybe_origin_model_type;

-- +-------------------------+----------+
-- | maybe_origin_model_type | count(*) |
-- +-------------------------+----------+
-- | NULL                    |   347531 |
-- | comfy_ui                |    46013 |
-- | face_fusion             |     1524 |
-- | gpt_sovits              |   304046 |
-- | live_portrait           |    12743 |
-- | mocap_net               |       71 |
-- | rvc_v2                  |  6107295 |
-- | sad_talker              |   156308 |
-- | so_vits_svc             |   557377 |
-- | stable_diffusion_1_5    |   450196 |
-- | styletts2               |   263575 |
-- | tacotron2               | 33453882 |
-- | vall_e_x                |    31188 |
-- +-------------------------+----------+
-- 13 rows in set (51.98 sec)
-- TODO [DONEx2]: maybe_origin_model_type = comfy_ui, mocap_net, face_fusion, live_portrait


-- media_type
select distinct media_type,
count(*)
from media_files
group by media_type;

-- +------------+----------+
-- | media_type | count(*) |
-- +------------+----------+
-- | audio      | 40676557 |
-- | bvh        |      107 |
-- | fbx        |      325 |
-- | gif        |        7 |
-- | glb        |     3100 |
-- | gltf       |       45 |
-- | image      |   481274 |
-- | jpg        |       91 |
-- | mp4        |    13280 |
-- | pmd        |       11 |
-- | pmx        |      239 |
-- | png        |      208 |
-- | scene_json |     4562 |
-- | scene_ron  |      688 |
-- | video      |   247391 |
-- | vmd        |      168 |
-- | wav        |   303996 |
-- +------------+----------+
-- 17 rows in set (43.68 sec)
-- TODO [DONE WITH QUERY]: media_type = bvh, fbx, gif, glb, gltf, jpg, png, pmx, pmd, scene_json, scene_ron, vmd
-- TODO: Later: media_type = (mp4, video)


-- media_class
select distinct media_class,
count(*)
from media_files
group by media_class;

-- +-------------+----------+
-- | media_class | count(*) |
-- +-------------+----------+
-- | audio       | 40786861 |
-- | dimensional |     7846 |
-- | image       |    30446 |
-- | unknown     |   653032 |
-- | video       |   254270 |
-- +-------------+----------+
-- 5 rows in set (46.78 sec)
-- TODO [QUERY DONE]: media_class = dimensional


-- maybe_media_subtype
select distinct maybe_media_subtype,
count(*)
from media_files
group by maybe_media_subtype;

-- +---------------------+----------+
-- | maybe_media_subtype | count(*) |
-- +---------------------+----------+
-- | NULL                | 41730497 |
-- | mixamo              |      950 |
-- | storyteller_scene   |      766 |
-- | scene_import        |      385 |
-- | mocap_net           |        2 |
-- | animation_only      |       15 |
-- | object              |        1 |
-- +---------------------+----------+
-- 7 rows in set (25.48 sec)
-- TODO [QUERY DONE]: maybe_media_subtype IS NOT NULL


-- maybe_animation_type
select distinct maybe_animation_type,
count(*)
from media_files
group by maybe_animation_type;

-- +----------------------+----------+
-- | maybe_animation_type | count(*) |
-- +----------------------+----------+
-- | NULL                 | 41732552 |
-- | mixamo               |      360 |
-- | mocap_net_ar_kit     |        1 |
-- | ar_kit               |       41 |
-- | move_ai              |       46 |
-- | move_ai_ar_kit       |        7 |
-- | rigify               |        7 |
-- | mixamo_ar_kit        |      114 |
-- | miku_miku_dance      |      290 |
-- | mocap_net            |        2 |
-- +----------------------+----------+
-- 10 rows in set (27.46 sec)
-- TODO [QUERY DONE]: maybe_animation_type IS NOT NULL


-- maybe_engine_category
select distinct maybe_engine_category,
count(*)
from media_files
group by maybe_engine_category;

-- +-----------------------+----------+
-- | maybe_engine_category | count(*) |
-- +-----------------------+----------+
-- | NULL                  | 41729660 |
-- | animation             |      344 |
-- | character             |      707 |
-- | creature              |       71 |
-- | expression            |       45 |
-- | image_plane           |      306 |
-- | location              |      105 |
-- | object                |      694 |
-- | scene                 |     4493 |
-- | set_dressing          |       44 |
-- | skybox                |        1 |
-- | video_plane           |       77 |
-- +-----------------------+----------+
-- 12 rows in set (36.41 sec)
-- TODO [QUERY DONE]: maybe_engine_category IS NOT NULL
