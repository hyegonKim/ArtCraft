

INSERT INTO user_roles
SET 
  slug = 'user',
  name = 'User',
  -- Usage
  can_use_tts = TRUE,
  can_use_w2l = TRUE,
  can_delete_own_tts_results = TRUE,
  can_delete_own_w2l_results = TRUE,
  can_delete_own_account = TRUE,
  -- Contribution
  can_upload_tts_models = TRUE,
  can_upload_w2l_templates = TRUE,
  can_delete_own_tts_models = TRUE,
  can_delete_own_w2l_templates = TRUE,
  -- Moderation
  can_approve_w2l_templates = FALSE,
  can_edit_other_users_profiles = FALSE,
  can_edit_other_users_tts_models = FALSE,
  can_edit_other_users_w2l_templates = FALSE,
  can_delete_other_users_tts_models = FALSE,
  can_delete_other_users_tts_results = FALSE,
  can_delete_other_users_w2l_templates = FALSE,
  can_delete_other_users_w2l_results = FALSE,
  can_ban_users = FALSE,
  can_delete_users = FALSE;

INSERT INTO user_roles
SET 
  slug = 'mod',
  name = 'Moderator',
  -- Usage
  can_use_tts = TRUE,
  can_use_w2l = TRUE,
  can_delete_own_tts_results = TRUE,
  can_delete_own_w2l_results = TRUE,
  can_delete_own_account = TRUE,
  -- Contribution
  can_upload_tts_models = TRUE,
  can_upload_w2l_templates = TRUE,
  can_delete_own_tts_models = TRUE,
  can_delete_own_w2l_templates = TRUE,
  -- Moderation
  can_approve_w2l_templates = TRUE,
  can_edit_other_users_profiles = TRUE,
  can_edit_other_users_tts_models = TRUE,
  can_edit_other_users_w2l_templates = TRUE,
  can_delete_other_users_tts_models = TRUE,
  can_delete_other_users_tts_results = TRUE,
  can_delete_other_users_w2l_templates = TRUE,
  can_delete_other_users_w2l_results = TRUE,
  can_ban_users = TRUE,
  can_delete_users = TRUE;

INSERT INTO user_roles
SET 
  slug = 'admin',
  name = 'Admin',
  -- Usage
  can_use_tts = TRUE,
  can_use_w2l = TRUE,
  can_delete_own_tts_results = TRUE,
  can_delete_own_w2l_results = TRUE,
  can_delete_own_account = TRUE,
  -- Contribution
  can_upload_tts_models = TRUE,
  can_upload_w2l_templates = TRUE,
  can_delete_own_tts_models = TRUE,
  can_delete_own_w2l_templates = TRUE,
  -- Moderation
  can_approve_w2l_templates = TRUE,
  can_edit_other_users_profiles = TRUE,
  can_edit_other_users_tts_models = TRUE,
  can_edit_other_users_w2l_templates = TRUE,
  can_delete_other_users_tts_models = TRUE,
  can_delete_other_users_tts_results = TRUE,
  can_delete_other_users_w2l_templates = TRUE,
  can_delete_other_users_w2l_results = TRUE,
  can_ban_users = TRUE,
  can_delete_users = TRUE;

