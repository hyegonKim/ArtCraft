-- noinspection SqlDialectInspectionForFile
-- noinspection SqlNoDataSourceInspectionForFile
-- noinspection SqlResolveForFile

-- In order to rebuild any "zs_voice", we need to know which dataset samples were included.
-- Since users can change the dataset at any time after a voice was originally created, these
-- records are generated to retain a record of which samples fed into a given voice.
-- These records will probably not be used for any product features, just as a means to study
-- poor-performing voices, perform migrations, etc.
CREATE TABLE zs_voice_included_samples (
  -- Not used for anything except replication.
  id BIGINT(20) NOT NULL AUTO_INCREMENT,

  -- First member of composite effective "primary key"
  -- The "zs_voice" that was created.
  voice_token VARCHAR(32) NOT NULL,

  -- Second member of composite effective "primary key"
  -- The "zs_voice_dataset_sample" that was used.
  dataset_sample_token VARCHAR(32) NOT NULL,

  -- INDICES --
  PRIMARY KEY (id),
  UNIQUE KEY (voice_token, dataset_sample_token)

) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_bin;
