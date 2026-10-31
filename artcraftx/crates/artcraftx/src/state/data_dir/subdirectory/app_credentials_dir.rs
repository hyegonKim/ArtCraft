use crate::credentials::auth_credential::AuthCredential;
use core_types::enums::generation_source::GenerationSource;
use crate::error::artcraftx_credential_error::ArtcraftXCredentialError;
use crate::error::artcraftx_error::ArtcraftXError;
use crate::state::data_dir::subdirectory::trait_data_subdir::DataSubdir;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use core_types::identifiers::credential_id::CredentialId;

/// The directory holding per-service credential TOML files
/// (by default `~/Artcraft/artcraftx/credentials`).
///
/// Files can be named anything (`artcraft_user1.toml`, `fal_api_key.toml`,
/// `higgsfield.toml`, ...) — users may hand-write their own. See
/// [`crate::credentials`] for the file format.
#[derive(Clone)]
pub struct AppCredentialsDir {
  path: PathBuf,
}

impl DataSubdir for AppCredentialsDir {
  const DIRECTORY_NAME: &'static str = "credentials";

  fn new_from<P: AsRef<Path>> (dir: P) -> Self {
    Self {
      path: dir.as_ref().to_path_buf(),
    }
  }

  fn path(&self) -> &Path {
    &self.path
  }
}

impl AppCredentialsDir {
  /// List and load every `*.toml` credential file in the directory.
  ///
  /// Files that fail to parse or validate are skipped with a warning so a
  /// single malformed (possibly hand-written) file can't take down every
  /// other credential. Only a directory listing failure is an error.
  pub fn load_credentials(&self) -> Result<Vec<AuthCredential>, ArtcraftXError> {
    let entries = std::fs::read_dir(&self.path)
        .map_err(|source| ArtcraftXCredentialError::DirectoryReadError {
          path: self.path.clone(),
          source,
        })?;

    let mut paths: Vec<PathBuf> = entries
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .filter(|path| path.is_file() && has_toml_extension(path))
        .collect();
    paths.sort();

    let mut credentials = Vec::new();
    let mut seen_ids: HashSet<CredentialId> = HashSet::new();
    for path in paths {
      match AuthCredential::load_from_file(&path) {
        Ok(mut credential) => {
          // Ids must be unique; a duplicate (eg. a copied file) gets a
          // fresh id, persisted so it stays stable.
          if seen_ids.contains(&credential.id) {
            credential.id = generate_id_not_in(&seen_ids);
            if let Err(err) = self.save_credential(&credential) {
              log::warn!(
                "Could not persist deduplicated credential id for {:?}: {}",
                credential.source_path, err,
              );
            }
          }
          seen_ids.insert(credential.id.clone());
          credentials.push(credential);
        },
        Err(err) => {
          log::warn!("Skipping bad credential file: {}", err);
        },
      }
    }

    Ok(credentials)
  }

  /// Rewrite a credential's TOML file in place (refreshed cookies,
  /// success/failure timestamps, etc.)
  pub fn save_credential(&self, credential: &AuthCredential) -> Result<(), ArtcraftXError> {
    credential.save().map_err(ArtcraftXError::from)
  }

  /// Path for a new managed credential file with the given file stem,
  /// e.g. `file_path_for("fal_api_key")` -> `.../credentials/fal_api_key.toml`.
  pub fn file_path_for(&self, file_stem: &str) -> PathBuf {
    self.path.join(format!("{}.toml", file_stem))
  }

  /// Next unused managed path for a service, following the naming scheme
  /// `{service}.toml`, then `{service}_2.toml`, `{service}_3.toml`, ...
  /// (Users are free to rename the files afterwards.)
  pub fn next_available_credential_path(&self, service: GenerationSource) -> PathBuf {
    let stem = service.to_str();
    let first = self.file_path_for(stem);
    if !first.exists() {
      return first;
    }
    let mut number: u32 = 2;
    loop {
      let candidate = self.file_path_for(&format!("{}_{}", stem, number));
      if !candidate.exists() {
        return candidate;
      }
      number += 1;
    }
  }

  /// Resolve a credential id (a bare file name like `fal_api_2.toml`) to a
  /// path inside this directory, rejecting anything that could escape it.
  pub fn resolve_credential_file(&self, file_name: &str) -> Result<PathBuf, ArtcraftXError> {
    let is_bare_file_name = !file_name.is_empty()
        && !file_name.contains('/')
        && !file_name.contains('\\')
        && !file_name.contains("..");
    let is_toml = Path::new(file_name)
        .extension()
        .map(|ext| ext.eq_ignore_ascii_case("toml"))
        .unwrap_or(false);
    if !is_bare_file_name || !is_toml {
      return Err(ArtcraftXCredentialError::InvalidFileName {
        file_name: file_name.to_string(),
      }.into());
    }
    Ok(self.path.join(file_name))
  }

  /// Load a single credential by id (file name).
  pub fn load_credential(&self, file_name: &str) -> Result<AuthCredential, ArtcraftXError> {
    let path = self.resolve_credential_file(file_name)?;
    AuthCredential::load_from_file(path).map_err(ArtcraftXError::from)
  }

  /// Find a credential by its id, the stable primary identifier.
  pub fn find_credential_by_id(
    &self,
    id: &str,
  ) -> Result<Option<AuthCredential>, ArtcraftXError> {
    Ok(self.load_credentials()?
        .into_iter()
        .find(|credential| credential.id.as_str() == id))
  }

  /// Generate an id no existing credential uses. Collisions are
  /// astronomically unlikely; the check guards against them anyway.
  pub fn generate_unique_credential_id(&self) -> CredentialId {
    let existing: HashSet<CredentialId> = self.load_credentials()
        .map(|credentials| credentials.into_iter().map(|c| c.id).collect())
        .unwrap_or_default();
    generate_id_not_in(&existing)
  }

  /// Delete a credential file by id (file name).
  pub fn delete_credential_file(&self, file_name: &str) -> Result<(), ArtcraftXError> {
    let path = self.resolve_credential_file(file_name)?;
    std::fs::remove_file(&path)
        .map_err(|source| ArtcraftXCredentialError::FileDeleteError { path, source }.into())
  }
}

fn generate_id_not_in(taken: &HashSet<CredentialId>) -> CredentialId {
  loop {
    let id = CredentialId::generate();
    if !taken.contains(&id) {
      return id;
    }
  }
}

fn has_toml_extension(path: &Path) -> bool {
  path.extension()
      .map(|ext| ext.eq_ignore_ascii_case("toml"))
      .unwrap_or(false)
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::credentials::api_key_credential::ApiKeyCredential;
  use crate::credentials::auth_credential::CredentialSecret;

  fn write_file(dir: &Path, name: &str, contents: &str) {
    std::fs::write(dir.join(name), contents).unwrap();
  }

  #[test]
  fn loads_all_valid_toml_files() {
    let dir = tempfile::tempdir().unwrap();
    write_file(
      dir.path(),
      "artcraft_user1.toml",
      "service = \"artcraft_cookies\"\n[[cookie.cookies]]\nname = \"a\"\nvalue = \"b\"\ndomain = \"storyteller.ai\"\n",
    );
    write_file(
      dir.path(),
      "fal_api_key.toml",
      "service = \"fal_api\"\n[api_key]\napi_key = \"fal-key-123\"\n",
    );
    // Non-toml and malformed files must not break the load.
    write_file(dir.path(), "notes.txt", "not a credential");
    write_file(dir.path(), "broken.toml", "service = \"fal_api\"");

    let creds_dir = AppCredentialsDir::new_from(dir.path());
    let credentials = creds_dir.load_credentials().unwrap();

    assert_eq!(credentials.len(), 2);
    assert_eq!(credentials[0].service, GenerationSource::ArtcraftCookies);
    assert_eq!(credentials[1].service, GenerationSource::FalApi);
  }

  #[test]
  fn save_credential_writes_to_source_path() {
    let dir = tempfile::tempdir().unwrap();
    let creds_dir = AppCredentialsDir::new_from(dir.path());

    let credential = AuthCredential {
      id: CredentialId::generate(),
      service: GenerationSource::FalApi,
      name: None,
      secret: CredentialSecret::ApiKey(ApiKeyCredential::new("fal-key-123")),
      user_info: None,
      source_path: creds_dir.file_path_for("fal_api_key"),
    };
    creds_dir.save_credential(&credential).unwrap();

    let credentials = creds_dir.load_credentials().unwrap();
    assert_eq!(credentials.len(), 1);
    assert_eq!(credentials[0].api_key().unwrap().api_key, "fal-key-123");
  }

  #[test]
  fn next_available_path_numbers_duplicates() {
    let dir = tempfile::tempdir().unwrap();
    let creds_dir = AppCredentialsDir::new_from(dir.path());
    let service = GenerationSource::FalApi;

    let first = creds_dir.next_available_credential_path(service);
    assert_eq!(first.file_name().unwrap(), "fal_api.toml");
    std::fs::write(&first, "").unwrap();

    let second = creds_dir.next_available_credential_path(service);
    assert_eq!(second.file_name().unwrap(), "fal_api_2.toml");
    std::fs::write(&second, "").unwrap();

    let third = creds_dir.next_available_credential_path(service);
    assert_eq!(third.file_name().unwrap(), "fal_api_3.toml");
  }

  #[test]
  fn missing_id_is_backfilled_and_stable_across_loads() {
    let dir = tempfile::tempdir().unwrap();
    write_file(
      dir.path(),
      "fal_api.toml",
      "service = \"fal_api\"\n[api_key]\napi_key = \"fal-key-123\"\n",
    );

    let creds_dir = AppCredentialsDir::new_from(dir.path());
    let first_load = creds_dir.load_credentials().unwrap();
    assert_eq!(first_load.len(), 1);
    let id = first_load[0].id.clone();
    assert!(id.as_str().starts_with("credential_"));

    // The generated id was persisted, so a reload sees the same one.
    let second_load = creds_dir.load_credentials().unwrap();
    assert_eq!(second_load[0].id, id);
  }

  #[test]
  fn duplicate_ids_are_deduplicated_on_load() {
    let dir = tempfile::tempdir().unwrap();
    let duplicated = "id = \"credential_copied\"\nservice = \"fal_api\"\n[api_key]\napi_key = \"x\"\n";
    write_file(dir.path(), "fal_api.toml", duplicated);
    write_file(dir.path(), "fal_api_2.toml", duplicated);

    let creds_dir = AppCredentialsDir::new_from(dir.path());
    let credentials = creds_dir.load_credentials().unwrap();

    assert_eq!(credentials.len(), 2);
    assert_ne!(credentials[0].id, credentials[1].id);

    // The reassigned id was persisted, so a reload is stable.
    let reloaded = creds_dir.load_credentials().unwrap();
    let mut ids: Vec<_> = credentials.iter().map(|c| c.id.clone()).collect();
    let mut reloaded_ids: Vec<_> = reloaded.iter().map(|c| c.id.clone()).collect();
    ids.sort();
    reloaded_ids.sort();
    assert_eq!(ids, reloaded_ids);
  }

  #[test]
  fn find_credential_by_id_matches() {
    let dir = tempfile::tempdir().unwrap();
    write_file(
      dir.path(),
      "fal_api.toml",
      "id = \"credential_findme\"\nservice = \"fal_api\"\n[api_key]\napi_key = \"x\"\n",
    );

    let creds_dir = AppCredentialsDir::new_from(dir.path());
    let found = creds_dir.find_credential_by_id("credential_findme").unwrap();
    assert_eq!(found.unwrap().file_name(), "fal_api.toml");

    let missing = creds_dir.find_credential_by_id("credential_nope").unwrap();
    assert!(missing.is_none());
  }

  #[test]
  fn resolve_rejects_path_traversal() {
    let dir = tempfile::tempdir().unwrap();
    let creds_dir = AppCredentialsDir::new_from(dir.path());
    assert!(creds_dir.resolve_credential_file("../evil.toml").is_err());
    assert!(creds_dir.resolve_credential_file("a/b.toml").is_err());
    assert!(creds_dir.resolve_credential_file("not_toml.txt").is_err());
    assert!(creds_dir.resolve_credential_file("").is_err());
    assert!(creds_dir.resolve_credential_file("fal_api.toml").is_ok());
  }

  #[test]
  fn delete_removes_the_right_file() {
    let dir = tempfile::tempdir().unwrap();
    write_file(
      dir.path(),
      "fal_api.toml",
      "service = \"fal_api\"\n[api_key]\napi_key = \"key-one\"\n",
    );
    write_file(
      dir.path(),
      "fal_api_2.toml",
      "service = \"fal_api\"\n[api_key]\napi_key = \"key-two\"\n",
    );

    let creds_dir = AppCredentialsDir::new_from(dir.path());
    creds_dir.delete_credential_file("fal_api.toml").unwrap();

    let remaining = creds_dir.load_credentials().unwrap();
    assert_eq!(remaining.len(), 1);
    assert_eq!(remaining[0].api_key().unwrap().api_key, "key-two");
  }
}
