-- noinspection SqlDialectInspectionForFile
-- noinspection SqlNoDataSourceInspectionForFile
-- noinspection SqlResolveForFile

-- NB: These are for use in Stripe Sigma, not MySQL
-- NB: You must use single quotes instead of double quotes when comparing string values.

-- Status values: past_due, incomplete, active, canceled, incomplete_expired (perhaps not exhaustive)
select distinct status
from subscriptions

select count(*)
from subscriptions
where status IN ('active');

-- Metadata is in the 'subscriptions_metadata' table, and we store 'username', 'email', and 'user_token'
select
    m.value,
    m.key,
    s.id,
    s.created,
    s.cancel_at,
    s.canceled_at
from subscriptions as s
join subscriptions_metadata as m
on m.subscription_id = s.id
where m.key = 'username'


-- Full mapping of Stripe subscribers to FakeYou users
select
    s.id as subscription_id,
    user_tokens.user_token,
    usernames.username,
    emails.email,
    s.status as subscription_status,
    s.created,
    s.cancel_at,
    s.canceled_at
from subscriptions as s
join (
    select
        m.subscription_id,
        m.value as username
    from subscriptions_metadata as m
    where m.key = 'username'
) as usernames
on s.id = usernames.subscription_id
join (
    select m.subscription_id,
           m.value as email
    from subscriptions_metadata as m
    where m.key = 'email'
) as emails
on s.id = emails.subscription_id
join (
    select m.subscription_id,
           m.value as user_token
    from subscriptions_metadata as m
    where m.key = 'user_token'
) as user_tokens
on s.id = user_tokens.subscription_id
order by s.id asc

-- Without emails
select
    s.id as subscription_id,
    user_tokens.user_token,
    usernames.username,
    s.status as subscription_status,
    s.created,
    s.cancel_at,
    s.canceled_at
from subscriptions as s
join (
    select
        m.subscription_id,
        m.value as username
    from subscriptions_metadata as m
    where m.key = 'username'
) as usernames
on s.id = usernames.subscription_id
join (
    select m.subscription_id,
           m.value as user_token
    from subscriptions_metadata as m
    where m.key = 'user_token'
) as user_tokens
on s.id = user_tokens.subscription_id
order by s.id asc
