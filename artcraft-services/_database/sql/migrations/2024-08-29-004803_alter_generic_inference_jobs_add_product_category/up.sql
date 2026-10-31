-- noinspection SqlDialectInspectionForFile
-- noinspection SqlNoDataSourceInspectionForFile
-- noinspection SqlResolveForFile

ALTER TABLE generic_inference_jobs
    ADD COLUMN product_category VARCHAR(32) DEFAULT NULL
    AFTER job_type;

ALTER TABLE generic_inference_jobs
    ADD INDEX index_product_category (product_category);
