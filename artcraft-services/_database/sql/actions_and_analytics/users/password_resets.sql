-- noinspection SqlDialectInspectionForFile
-- noinspection SqlNoDataSourceInspectionForFile
-- noinspection SqlResolveForFile

-- A look into the most recent password resets.
-- User account age, password change version, etc.
SELECT
    e.destination_email_address AS email,
    u.username,
    u.created_at,
    u.updated_at,
    u.password_version,
    u.version
FROM email_sender_jobs AS e
JOIN users AS u
    on u.token = e.maybe_destination_user_token
order by e.id desc
limit 100;
