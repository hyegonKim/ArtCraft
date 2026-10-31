--- Analyze records
SELECT
  job_type,
  product_category,
  inference_category,
  maybe_model_type,
  created_at
FROM generic_inference_jobs
WHERE product_category IS NULL
AND created_at >= '2024-07-31'
ORDER BY ID DESC;

-- AND job_type NOT IN ('stable_diffusion', 'so_vits_svc', 'comfy_ui')

-- Update studio jobs (distinguished from VST jobs)
UPDATE generic_inference_jobs
SET product_category = 'studio'
WHERE job_type = 'comfy_ui'
AND product_category IN ('vst', 'studio', NULL)
AND token IN (
    SELECT
      job.token
    FROM media_files as m
    JOIN (
        SELECT
          token,
          job_type,
          product_category,
          inference_category,
          maybe_model_type,
          created_at,
          JSON_UNQUOTE(JSON_EXTRACT(maybe_inference_args, '$.args.Cu.in')) as input_token
        FROM generic_inference_jobs
        WHERE job_type = 'comfy_ui'
        AND product_category IN ('vst', 'studio', NULL)
        AND created_at >= '2024-05-01'
        ORDER BY ID DESC
    ) as job
    ON m.token = job.input_token
    WHERE m.created_at >= '2024-05-01'
    AND   m.maybe_scene_source_media_file_token IS NOT NULL
);

-- Update VST jobs (distinguished from studio jobs)
UPDATE generic_inference_jobs
SET product_category = 'vst'
WHERE job_type = 'comfy_ui'
AND product_category IN ('vst', 'studio', NULL)
AND token IN (
    SELECT
      job.token
    FROM media_files as m
    JOIN (
        SELECT
          token,
          job_type,
          product_category,
          inference_category,
          maybe_model_type,
          created_at,
          JSON_UNQUOTE(JSON_EXTRACT(maybe_inference_args, '$.args.Cu.in')) as input_token
        FROM generic_inference_jobs
        WHERE job_type = 'comfy_ui'
        AND product_category IN ('vst', 'studio', NULL)
        AND created_at >= '2024-03-01'
        ORDER BY ID DESC
    ) as job
    ON m.token = job.input_token
    WHERE m.created_at >= '2024-03-01'
    AND   m.maybe_scene_source_media_file_token IS NULL
);

-- Update final remaining VST jobs (distinguished from studio jobs)
UPDATE generic_inference_jobs
SET product_category = 'vst'
WHERE job_type = 'comfy_ui'
AND product_category IS NULL
AND token NOT IN (
    SELECT
      job.token
    FROM media_files as m
    JOIN (
        SELECT
          token,
          job_type,
          product_category,
          inference_category,
          maybe_model_type,
          created_at,
          JSON_UNQUOTE(JSON_EXTRACT(maybe_inference_args, '$.args.Cu.in')) as input_token
        FROM generic_inference_jobs
        WHERE job_type = 'comfy_ui'
        AND product_category IN ('vst', 'studio', NULL)
        AND created_at >= '2024-03-01'
    ) as job
    ON m.token = job.input_token
    WHERE m.created_at >= '2024-03-01'
);



-- Distinguish between VST and Studio
SELECT
  job_type,
  product_category,
  inference_category,
  maybe_model_type,
  created_at,
  maybe_inference_args,
  JSON_EXTRACT(maybe_inference_args, '$.args.Cu.in') as input_token
FROM generic_inference_jobs
WHERE job_type = 'comfy_ui'
AND product_category IS NULL
AND created_at >= '2024-07-31'
ORDER BY ID DESC
LIMIT 359;



--- Update tacotron2 jobs
UPDATE generic_inference_jobs
SET product_category = 'tts_tacotron2'
WHERE job_type = 'tacotron2'
AND inference_category = 'text_to_speech'
AND maybe_model_type = 'tacotron2'
AND product_category IS NULL
AND created_at >= '2024-07-30'
AND created_at <= '2024-08-30'
LIMIT 100000;


--- Update GptSoVits (#1)
UPDATE generic_inference_jobs
SET product_category = 'tts_gpt_so_vits'
WHERE job_type = 'gpt_sovits'
AND inference_category = 'text_to_speech'
AND maybe_model_type IS NULL
AND product_category IS NULL
AND created_at >= '2024-08-14'
AND created_at <= '2024-08-30'
LIMIT 100000;


--- Update GptSoVits (#2: we changed the inference_category)
UPDATE generic_inference_jobs
SET product_category = 'tts_gpt_so_vits'
WHERE job_type = 'gpt_sovits'
AND inference_category = 'deprecated_field'
AND maybe_model_type IS NULL
AND product_category IS NULL
AND created_at >= '2024-07-15'
AND created_at <= '2024-08-30'
LIMIT 100000;


-- Update GptSoVits Download Jobs
UPDATE generic_inference_jobs
SET product_category = 'download_gpt_so_vits'
WHERE job_type = 'gpt_sovits'
AND maybe_download_url IS NOT NULL
LIMIT 10000;


--- TODO: Not working
--- Update RVCv2 VC jobs
UPDATE generic_inference_jobs
SET product_category = 'vc_rvc2'
WHERE job_type = 'rvc_v2'
AND inference_category = 'voice_conversion'
AND maybe_model_type = 'rvc_v2'
AND product_category IS NULL
AND created_at >= '2024-07-31'
AND created_at <= '2024-08-30'
LIMIT 100000;


--- Update SVC VC jobs
UPDATE generic_inference_jobs
SET product_category = 'vc_svc'
WHERE job_type = 'so_vits_svc'
AND product_category IS NULL
AND created_at >= '2024-07-25'
AND created_at <= '2024-08-30'
LIMIT 10000;


--- Update StyleTTS2 jobs
UPDATE generic_inference_jobs
SET product_category = 'tts_style_tts2'
WHERE job_type = 'styletts2'
AND inference_category = 'text_to_speech'
AND maybe_model_type = 'styletts2'
AND product_category IS NULL
AND created_at >= '2024-08-15'
AND created_at <= '2024-08-30'
LIMIT 1000;


--- Update Live Portrait jobs (#1)
UPDATE generic_inference_jobs
SET product_category = 'live_portrait'
WHERE job_type = 'live_portrait'
AND inference_category = 'live_portrait'
AND maybe_model_type IS NULL
AND product_category IS NULL
AND created_at >= '2024-07-25'
LIMIT 10000;

--- Update Live Portrait jobs (#2)
UPDATE generic_inference_jobs
SET product_category = 'live_portrait'
WHERE job_type = 'live_portrait'
AND inference_category = 'deprecated_field'
AND maybe_model_type IS NULL
AND product_category IS NULL
AND created_at >= '2024-07-25'
LIMIT 10000;

--- Update SadTalker jobs
UPDATE generic_inference_jobs
SET product_category = 'lipsync_sad_talker'
WHERE maybe_model_type = 'sad_talker'
AND inference_category = 'lipsync_animation'
AND product_category IS NULL
AND created_at >= '2024-07-25'
LIMIT 100;

--- Update "stable diffusion" jobs (how are people still enqueueing these?)
UPDATE generic_inference_jobs
SET product_category = 'stable_diffusion'
WHERE job_type = 'stable_diffusion'
AND inference_category = 'image_generation'
AND maybe_model_type = 'stable_diffusion'
AND product_category IS NULL
AND created_at >= '2024-07-25'
AND created_at <= '2024-09-30'
LIMIT 100;

-- Change Product Category: vst -> vid_style_transfer
UPDATE generic_inference_jobs
SET product_category = 'vid_style_transfer'
WHERE product_category = 'vst'
AND created_at >= '2024-07-30'
AND created_at <= '2024-09-30'
LIMIT 10;

-- Change Product Category: studio -> vid_studio
UPDATE generic_inference_jobs
SET product_category = 'vid_studio'
WHERE product_category = 'studio'
AND created_at >= '2024-07-30'
AND created_at <= '2024-09-30'
LIMIT 10;

-- Change Product Category: vid_face_fusion -> vid_lipsync_face_fusion
UPDATE generic_inference_jobs
SET product_category = 'vid_lipsync_face_fusion'
WHERE product_category = 'vid_face_fusion'
AND created_at >= '2024-07-30'
AND created_at <= '2024-09-30'
LIMIT 10;

-- Change Product Category: lipsync_sad_talker -> vid_lipsync_sad_talker
UPDATE generic_inference_jobs
SET product_category = 'vid_lipsync_sad_talker'
WHERE product_category = 'lipsync_sad_talker'
AND created_at >= '2024-07-30'
AND created_at <= '2024-09-30'
LIMIT 10;

-- Change Product Category: live_portrait -> vid_live_portrait
UPDATE generic_inference_jobs
SET product_category = 'vid_live_portrait'
WHERE product_category = 'live_portrait'
AND created_at >= '2024-07-30'
AND created_at <= '2024-09-30'
LIMIT 10;

-- Change Product Category: live_portrait_webcam -> vid_live_portrait_webcam
UPDATE generic_inference_jobs
SET product_category = 'vid_live_portrait_webcam'
WHERE product_category = 'live_portrait_webcam'
AND created_at >= '2024-07-30'
AND created_at <= '2024-09-30'
LIMIT 10;
