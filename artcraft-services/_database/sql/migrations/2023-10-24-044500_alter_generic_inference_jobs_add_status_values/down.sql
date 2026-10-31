-- noinspection SqlDialectInspectionForFile
-- noinspection SqlNoDataSourceInspectionForFile
-- noinspection SqlResolveForFile

ALTER TABLE generic_inference_jobs
    MODIFY status ENUM(
        'pending',
        'started',
        'complete_success',
        'complete_failure',
        'attempt_failed',
        'dead'
  ) NOT NULL DEFAULT 'pending';
