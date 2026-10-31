
-- Media file cover images
select distinct maybe_cover_image_media_file_token
from media_files
where maybe_cover_image_media_file_token is not null
group by maybe_cover_image_media_file_token

-- Model weights cover images
select distinct maybe_cover_image_media_file_token
from model_weights
where maybe_cover_image_media_file_token is not null
group by maybe_cover_image_media_file_token
