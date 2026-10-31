-- noinspection SqlDialectInspectionForFile
-- noinspection SqlNoDataSourceInspectionForFile
-- noinspection SqlResolveForFile

ALTER TABLE media_files
    ADD COLUMN origin_product_category VARCHAR(16) NOT NULL DEFAULT 'unknown'
    AFTER origin_category;

ALTER TABLE media_files
    ADD INDEX index_origin_product_category (origin_product_category);
