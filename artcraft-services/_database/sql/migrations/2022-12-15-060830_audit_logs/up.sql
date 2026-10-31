-- noinspection SqlDialectInspectionForFile
-- noinspection SqlNoDataSourceInspectionForFile
-- noinspection SqlResolveForFile

CREATE TABLE audit_logs (

  -- Not used for anything except replication.
  id BIGINT(20) NOT NULL AUTO_INCREMENT,

  -- Effective "primary key" of the audit log item (in case we add a UI or notes)
  token VARCHAR(32) NOT NULL,

  -- The type of entity mutated
  entity_type VARCHAR(32) NOT NULL,

  -- The token of the entity mutated
  entity_token VARCHAR(32) NOT NULL,

  -- The type of action taken on the entity
  entity_action VARCHAR(32) NOT NULL,

  -- Typically only signed in users will be performing actions on known entities,
  -- but we're leaving this null just in case. This is the corresponding user that
  -- made the change.
  maybe_actor_user_token VARCHAR(32) DEFAULT NULL,

  -- Based on a cookie sent by the frontend.
  -- We'll save this even if the user is logged in.
  maybe_actor_anonymous_visitor_token VARCHAR(32) DEFAULT NULL,

  -- Whether the actor has some sort of moderator privilege.
  is_actor_moderator BOOLEAN NOT NULL DEFAULT FALSE,

  -- For abuse tracking.
  -- Wide enough for IPv4/6
  actor_ip_address VARCHAR(40) NOT NULL,

  created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
  updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,

  -- INDICES --
  PRIMARY KEY (id),
  UNIQUE KEY (token),
  KEY fk_entity_type (entity_type),
  KEY fk_entity_token (entity_token),
  KEY fk_maybe_actor_user_token (maybe_actor_user_token)

) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_bin;
