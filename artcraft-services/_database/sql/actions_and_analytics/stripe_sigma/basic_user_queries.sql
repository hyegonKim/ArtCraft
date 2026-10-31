-- noinspection SqlDialectInspectionForFile
-- noinspection SqlNoDataSourceInspectionForFile
-- noinspection SqlResolveForFile

-- NB: These are for use in Stripe Sigma, not MySQL
-- NB: You must use single quotes instead of double quotes when comparing string values.

-- Histogram of customers by country
select address_country, count(*)
from customers
group by address_country

select customer_id, subscriptions.status
from subscriptions


select distinct customer_id
from subscriptions
where subscriptions.status = 'active'
or subscriptions.status = 'past_due'

select address_country, count(*) as country_count
from customers
where id IN (
    select distinct customer_id
    from subscriptions
    where subscriptions.status = 'active'
    or subscriptions.status = 'past_due'
)
group by address_country


-- Histogram
select address_country, count(*)
from customers
group by address_country
