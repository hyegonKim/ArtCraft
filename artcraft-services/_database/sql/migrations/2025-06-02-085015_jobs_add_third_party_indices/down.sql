-- noinspection SqlDialectInspectionForFile
-- noinspection SqlNoDataSourceInspectionForFile
-- noinspection SqlResolveForFile

DROP INDEX `index_maybe_external_third_party_id`
    ON `generic_inference_jobs`;

DROP INDEX `index_maybe_external_third_party_and_id`
    ON `generic_inference_jobs`;
