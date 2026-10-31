-- noinspection SqlDialectInspectionForFile
-- noinspection SqlNoDataSourceInspectionForFile
-- noinspection SqlResolveForFile

ALTER TABLE generic_inference_jobs
    ADD COLUMN maybe_routing_tag VARCHAR(32) DEFAULT NULL
    AFTER is_debug_request;

ALTER TABLE generic_inference_jobs
    ADD INDEX index_maybe_routing_tag (maybe_routing_tag);
