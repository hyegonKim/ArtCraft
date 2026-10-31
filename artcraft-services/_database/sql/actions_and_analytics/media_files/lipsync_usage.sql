
-- Total usage
select
  count(*) as use_count
from generic_inference_jobs as gij
where gij.product_category = 'vid_face_fusion'
  and gij.created_at >= (CURDATE() - INTERVAL 5 DAY)

-- Usage by user
select
  u.token,
  u.username,
  count(*) as use_count
from generic_inference_jobs as gij
left outer join users as u
on u.token = gij.maybe_creator_user_token
where gij.product_category = 'vid_face_fusion'
  and gij.created_at >= (CURDATE() - INTERVAL 2 DAY)
group by u.token, u.username
order by use_count desc


-- TTS vs Face Fusion side by side statistics
select
  x.user_token,
  x.username,
  x.face_fusion_count,
  count(*) as tts_count
from (
    select
      u.token as user_token,
      u.username,
      count(*) as face_fusion_count
    from generic_inference_jobs as gij
    left outer join users as u
    on u.token = gij.maybe_creator_user_token
    where gij.product_category = 'vid_face_fusion'
      and gij.created_at >= (CURDATE() - INTERVAL 1 DAY)
    group by u.token, u.username
) as x
left outer join generic_inference_jobs as gij
on x.user_token = gij.maybe_creator_user_token
where gij.product_category = 'tts_tacotron2'
  and gij.created_at >= (CURDATE() - INTERVAL 1 DAY)
group by x.user_token, x.username
order by face_fusion_count desc
