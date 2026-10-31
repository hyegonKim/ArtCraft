-- noinspection SqlDialectInspectionForFile
-- noinspection SqlNoDataSourceInspectionForFile
-- noinspection SqlResolveForFile

-- Get pending job count
select count(*)
from generic_inference_jobs
where status = 'pending';

-- Get pending job count by job type
select
  job_type,
  count(*)
from generic_inference_jobs
where status = 'pending'
group by job_type;

-- Kill *ALL* pending / outstanding jobs of any type
update generic_inference_jobs
set status = 'dead'
where status IN ('pending', 'started', 'attempt_failed');

-- Kill all routed jobs
update generic_inference_jobs
set status = 'dead'
where maybe_routing_tag IS NOT NULL;

-- Kill all Tacotron TTS jobs
update generic_inference_jobs
set status = 'dead'
where status IN ('pending', 'started', 'attempt_failed')
  and maybe_model_type IN ('tacotron2');

-- Kill all Sad Talker Face Animation jobs
update generic_inference_jobs
set status = 'dead'
where status IN ('pending', 'started', 'attempt_failed')
  and maybe_model_type IN ('sad_talker');

-- Kill all jobs of several types
update generic_inference_jobs
set status = 'dead'
where status IN ('pending', 'started', 'attempt_failed')
  and maybe_model_type IN ('sad_talker', 'so_vits_svc');
