-- noinspection SqlDialectInspectionForFile
-- noinspection SqlNoDataSourceInspectionForFile
-- noinspection SqlResolveForFile

ALTER TABLE model_weights
    CHANGE maybe_description_markdown description_markdown TEXT NOT NULL;

ALTER TABLE model_weights
    CHANGE maybe_description_rendered_html description_rendered_html TEXT NOT NULL;
