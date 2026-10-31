-- noinspection SqlDialectInspectionForFile
-- noinspection SqlNoDataSourceInspectionForFile
-- noinspection SqlResolveForFile

-- List GptSoVits model downloads for user (Vegito)
select *
from generic_inference_jobs
where maybe_creator_user_token = 'U:E00D2RD3ZNZ7P'
and job_type = 'gpt_sovits'
and maybe_download_url IS NOT NULL
order by id desc
limit 100\G
