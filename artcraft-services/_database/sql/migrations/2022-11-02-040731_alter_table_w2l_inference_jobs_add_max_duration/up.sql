-- noinspection SqlDialectInspectionForFile
-- noinspection SqlNoDataSourceInspectionForFile
-- noinspection SqlResolveForFile

ALTER TABLE w2l_inference_jobs
    ADD COLUMN max_duration_seconds INTEGER NOT NULL DEFAULT 0
    AFTER creator_set_visibility;
