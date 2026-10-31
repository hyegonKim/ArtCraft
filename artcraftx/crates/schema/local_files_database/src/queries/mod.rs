// ── local_files: the content-hash index ──
pub mod local_file_record;
pub mod get_local_file_by_path;
pub mod get_local_files_by_hash;
pub mod get_local_files_by_paths;
pub mod list_local_files_needing_thumbnails;
pub mod set_thumbnail_result;
pub mod upsert_local_file;

// ── asset_uploads: the per-account upload-dedup ledger ──
pub mod asset_upload_record;
pub mod delete_asset_upload;
pub mod get_asset_upload;
pub mod touch_asset_upload_reused;
pub mod upsert_asset_upload;
