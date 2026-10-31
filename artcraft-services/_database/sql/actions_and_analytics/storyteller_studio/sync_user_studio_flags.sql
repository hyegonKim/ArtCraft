-- noinspection SqlDialectInspectionForFile
-- noinspection SqlNoDataSourceInspectionForFile
-- noinspection SqlResolveForFile

-- Sync the top-level boolean with the authoritative feature flags field.
-- Performs a full table scan and string search, but is relatively quick.
-- NB: Can't use users in the subquery:
-- ERROR 1093 (HY000): You can't specify target table 'users' for update in FROM clause
UPDATE users u INNER JOIN users uf
ON u.token = uf.token
SET u.can_access_studio = true
WHERE uf.maybe_feature_flags  like '%studio%';
