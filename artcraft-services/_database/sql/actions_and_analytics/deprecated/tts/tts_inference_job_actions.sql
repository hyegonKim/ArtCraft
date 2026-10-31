-- noinspection SqlDialectInspectionForFile
-- noinspection SqlNoDataSourceInspectionForFile
-- noinspection SqlResolveForFile

-- Kill pending jobs
update tts_inference_jobs set status = 'dead' where status = 'pending';

-- Kill all low priority pending, waiting, and in-progress jobs
update tts_inference_jobs
set status = 'dead'
where status IN ('pending', 'started', 'attempt_failed')
  and priority_level IN (0, 1);

-- Kill all pending jobs
update tts_inference_jobs
set status = 'dead'
where status = 'pending';

-- Kill all waiting and in-progress jobs
update tts_inference_jobs
set status = 'dead'
where status IN ('pending', 'started', 'attempt_failed');


