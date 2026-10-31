-- noinspection SqlDialectInspectionForFile
-- noinspection SqlNoDataSourceInspectionForFile
-- noinspection SqlResolveForFile

ALTER TABLE generic_inference_jobs
    ADD UNIQUE INDEX index_maybe_external_third_party_and_id (maybe_external_third_party, maybe_external_third_party_id),
    ALGORITHM=INPLACE, LOCK=NONE;

ALTER TABLE generic_inference_jobs
    ADD INDEX index_maybe_external_third_party_id (maybe_external_third_party_id),
    ALGORITHM=INPLACE, LOCK=NONE;
