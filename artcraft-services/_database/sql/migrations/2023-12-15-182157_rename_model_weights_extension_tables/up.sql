-- noinspection SqlDialectInspectionForFile
-- noinspection SqlNoDataSourceInspectionForFile
-- noinspection SqlResolveForFile

-- I'm doing this to visually align these tables and sort them together, because there will likely be other model_weights_* tables.
RENAME TABLE model_weights_image_generation_details TO model_weights_extension_image_generation_details;
RENAME TABLE model_weights_tts_details TO model_weights_extension_tts_details;
RENAME TABLE model_weights_vocoder_details TO model_weights_extension_vocoder_details;
RENAME TABLE model_weights_voice_conversion_details TO model_weights_extension_voice_conversion_details;
