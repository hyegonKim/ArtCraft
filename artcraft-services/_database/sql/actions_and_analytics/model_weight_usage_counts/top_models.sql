| Field       | Type             | Null | Key | Default | Extra          |
+-------------+------------------+------+-----+---------+----------------+
| id          | bigint(20)       | NO   | PRI | NULL    | auto_increment |
| token       | varchar(32)      | NO   | MUL | NULL    |                |
| on_date     | date             | NO   | MUL | NULL    |                |
| usage_count | int(10) unsigned | NO   | MUL | 0       |                |
+-------------+------------------+------+-----+---------+----------------+

--- Top models after date
select
  w.token,
  substring(w.title, 1, 100) as title,
  sum(usage_count) as usage_count
from model_weight_usage_counts as uc
join model_weights as w
  on uc.token = w.token
where on_date >= '2024-09-01'
group by w.token
order by usage_count desc
limit 150;


