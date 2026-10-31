-- noinspection SqlDialectInspectionForFile
-- noinspection SqlNoDataSourceInspectionForFile
-- noinspection SqlResolveForFile

-- Staff (1)
--  * teddanson - victor's alt
UPDATE users
SET
    can_access_studio = false,
    maybe_feature_flags = 'explore_media,studio,video_style_transfer'
WHERE username IN (
    'bflat',
    'brandon',
    'crossproduct',
    'crossproduct1',
    'echelon',
    'kasisnu',
    'olivicmic',
    'saltacc',
    'teddanson',
    'vegito1089',
    'wilwong',
    'zzz_last_item'
);

-- Staff (2)
--  * devdude123 - Joel
--  * el_cid_93 - for testing (who is this??)
--  * endtimes - @sugarbro (testing)
--  * printrman - miles
--  * tammieteller - Tammie (Pebblebed)
UPDATE users
SET
    can_access_studio = false,
    maybe_feature_flags = 'studio,video_style_transfer'
WHERE username IN (
    'candyfoxxx',
    'dannymcgee',
    'devdude123',
    'el_cid_93',
    'endtimes',
    'fyscott',
    'gateway',
    'heart_ribbon',
    'jags111',
    'justinjohn0306',
    'lorbach',
    'mrvintage',
    'printrman',
    'rewin123',
    'storyteller',
    'tropicalfun',
    'yae_ph',
    'zzz_last_item'
);

-- Early access (investors)
-- dreambig : hanashi
-- fanfiction : hanashi
-- fantasyworlds : hanashi
-- hollywoodstar : hanashi
-- postproduction : hanashi
-- show_dont_tell : hanashi
-- tellstories : hanashi
-- thedirector : hanashi

UPDATE users
SET
    can_access_studio = false,
    maybe_feature_flags = 'studio,video_style_transfer'
WHERE username IN (
    'claraqueiros',
    'dreambig',
    'fanfiction',
    'fantasyworlds',
    'hollywoodstar',
    'postproduction',
    'show_dont_tell',
    'tellstories',
    'thedirector',
    'tammieteller',
    'vagata',
    'zzz_last_item'
);

-- Early access (film)
-- Chechisauri0: Messaged Brandon on Discord; uses Premiere and Blender
-- dakrid: David, Brandon's filmmaker friend
-- omega7321: vegito / ishaan
-- thenadamgoes: Adam (/r/aivideo)
-- twirble: (/r/aivideo)
UPDATE users
SET
    can_access_studio = false,
    maybe_feature_flags = 'studio,video_style_transfer'
WHERE username IN (
    'FoxtailStudio',
    'chechisauri0',
    'dakrid',
    'omega7321',
    'thenadamgoes',
    'twirble',
    'zzz_last_item'
);

-- Early access (other)
UPDATE users
SET
    can_access_studio = false,
    maybe_feature_flags = 'studio,video_style_transfer'
WHERE username IN (
    'zzz_last_item'
);

-- Early access to VST only
UPDATE users
SET
    can_access_studio = false,
    maybe_feature_flags = 'video_style_transfer'
WHERE username IN (
    'fuxta',
    'kenjoplays',
    'ofccccccc',
    'sonicgt2',
    'stewiegroffin',
    'tanooki426',
    'wawoul',
    'waynut',
    'zzz_last_item'
);

