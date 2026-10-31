-- noinspection SqlDialectInspectionForFile
-- noinspection SqlNoDataSourceInspectionForFile
-- noinspection SqlResolveForFile

ALTER TABLE model_weights
    CHANGE description_markdown maybe_description_markdown TEXT DEFAULT NULL;

ALTER TABLE model_weights
    CHANGE description_rendered_html maybe_description_rendered_html TEXT DEFAULT NULL;
