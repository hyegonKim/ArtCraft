-- noinspection SqlDialectInspectionForFile
-- noinspection SqlNoDataSourceInspectionForFile
-- noinspection SqlResolveForFile

-- Staff
--   * coliphant - chris
--   * printrman - miles
--   * teddanson - victor's alt
-- SELECT token, username FROM users WHERE username IN (
UPDATE users SET user_role_slug = "admin" WHERE username IN (
  'bflat',
  'brandon',
  'coliphant',
  'crossproduct',
  'crossproduct1',
  'echelon',
  'endtimes',
  'fyscott',
  'mrvintage',
  'olivicmic',
  'printrman',
  'saltacc',
  'teddanson',
  'vegito1089',
  'wilwong',
  'yae_ph',
  'zzz_last_item'
);

