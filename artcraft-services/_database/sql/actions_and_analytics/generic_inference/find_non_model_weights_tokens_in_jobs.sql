-- Find vcm_* tokens that are still in use
select
  job_token,
  model_token,
  user_token
from (
  select token as job_token,
  maybe_model_token as model_token,
  maybe_creator_user_token as user_token
  from generic_inference_jobs
  order by id
  desc limit 1000000
) as x
where model_token NOT LIKE "TM:%"
and model_token NOT LIKE "weight_%"
limit 50;
