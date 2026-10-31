-- noinspection SqlDialectInspectionForFile
-- noinspection SqlNoDataSourceInspectionForFile
-- noinspection SqlResolveForFile

ALTER TABLE model_weights
    MODIFY COLUMN file_size_bytes INT(10) NOT NULL DEFAULT 0;
