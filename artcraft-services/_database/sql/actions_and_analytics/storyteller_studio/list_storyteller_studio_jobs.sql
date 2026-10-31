-- noinspection SqlDialectInspectionForFile
-- noinspection SqlNoDataSourceInspectionForFile
-- noinspection SqlResolveForFile

select *
from generic_inference_jobs
where job_type = 'comfy_ui'
and status NOT IN (
  'pending',
  'started',
  'complete_success'
)
order by id desc
limit 5\G

-- List started jobs
select *
from generic_inference_jobs
where job_type = 'comfy_ui'
and status IN ('started')
order by id desc
limit 50\G

update generic_inference_jobs
set inference_category = 'convert_bvh_to_workflow'
where inference_category = 'deprecated_field'
limit 5;

[2024-07-16T22:53:07Z ERROR inference_job::job::job_loop::main_loop] Error querying jobs: error: ""