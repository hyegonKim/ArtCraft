-- noinspection SqlDialectInspectionForFile
-- noinspection SqlNoDataSourceInspectionForFile
-- noinspection SqlResolveForFile

ALTER TABLE generic_inference_jobs
  CHANGE maybe_routing_tag maybe_routing_tag VARCHAR(64) DEFAULT NULL;