-- noinspection SqlDialectInspectionForFile
-- noinspection SqlNoDataSourceInspectionForFile
-- noinspection SqlResolveForFile

CREATE TABLE unsubscribe_reason (
    -- Generic primary key
    id BIGINT(20) NOT NULL AUTO_INCREMENT,
    
    -- User unsubscribing
    user_token VARCHAR(32) NOT NULL,

    -- User's feedback/reason for unsubscribing
    feedback_reason MEDIUMTEXT,

    -- tracking
    ip_address VARCHAR(40) NOT NULL,

    -- Timestamp the feedback was submitted at
    unsubscribed_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,

    PRIMARY KEY (id)

) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_bin;
