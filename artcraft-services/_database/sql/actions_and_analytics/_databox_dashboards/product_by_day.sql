SELECT
    DATE(gij.created_at) as created_date,
    product_category,
    count(*) as job_count
FROM generic_inference_jobs as gij
WHERE gij.created_at >= (CURDATE() - INTERVAL 30 DAY)
GROUP BY created_date, product_category;
