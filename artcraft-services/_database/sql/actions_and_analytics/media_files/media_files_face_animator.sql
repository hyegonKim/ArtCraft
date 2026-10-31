-- noinspection SqlDialectInspectionForFile
-- noinspection SqlNoDataSourceInspectionForFile
-- noinspection SqlResolveForFile

-- Select recent videos
select m.token,
       u.username
from media_files as m
left outer join users as u
on u.token = m.maybe_creator_user_token
where m.origin_product_category = 'face_animator'
order by m.id desc
limit 50;
