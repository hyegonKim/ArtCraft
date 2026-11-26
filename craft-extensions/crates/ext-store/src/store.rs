//! The write side of the shared store: install, remove, enable, choose a theme.
//!
//! Layout under the store root:
//!   packages/<id>/<version>/...   unpacked, never edited afterwards
//!   staging/<unique>/             where a package is unpacked and checked before it counts
//!   state.json                    what is installed and enabled (see `state`)
//!   .lock                         held while the state file changes
//!
//! Invariants: an install is all or nothing. The package is unpacked and checked outside the lock, the
//! lock covers only the move into place and the state write, the state file is replaced atomically with
//! a bumped generation, and a failure leaves the previous state and no stray package folder. A file
//! can never replace a first-party extension, and a downgrade is refused.

use crate::state::{Installed, STATE_FILE, State, StateError};
use crate::unpack::{Caps, UnpackError, unpack_into};
use craft_ext_core::id::{AppId, ExtensionId};
use craft_ext_core::manifest::{Kind, THEME_FILE};
use craft_ext_core::palette::{ThemeError, ThemeFile};
use craft_ext_core::tier::{Policy, Tier, TierError};
use std::fmt;
use std::fs::{self, File, TryLockError};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const LOCK_WAIT: Duration = Duration::from_secs(5);

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StoreError {
    Unpack(UnpackError),
    Tier(TierError),
    BadTheme(ThemeError),
    State(StateError),
    Busy,
    Downgrade,
    AlreadyInstalled,
    WouldReplaceFirstParty,
    NotInstalled,
    NotForThisApp,
    NotATheme,
    NotEnabled,
    Io,
}

impl fmt::Display for StoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            StoreError::Unpack(e) => write!(f, "{e}"),
            StoreError::Tier(e) => write!(f, "{e}"),
            StoreError::BadTheme(e) => write!(f, "{e}"),
            StoreError::State(e) => write!(f, "{e}"),
            StoreError::Busy => f.write_str("another ArtCraft app is changing extensions right now; try again in a moment"),
            StoreError::Downgrade => f.write_str("a newer version of this extension is already installed"),
            StoreError::AlreadyInstalled => f.write_str("this version of the extension is already installed"),
            StoreError::WouldReplaceFirstParty => f.write_str("this file would replace an extension made by ArtCraft, which is not allowed"),
            StoreError::NotInstalled => f.write_str("that extension is not installed"),
            StoreError::NotForThisApp => f.write_str("that extension is not made for this app"),
            StoreError::NotATheme => f.write_str("only a theme can be chosen as the look of an app"),
            StoreError::NotEnabled => f.write_str("turn the theme on for this app before choosing it"),
            StoreError::Io => f.write_str("the extension files could not be saved"),
        }
    }
}

impl std::error::Error for StoreError {}

/// Holds the cross-process lock for as long as it lives.
struct Lock(File);

impl Drop for Lock {
    fn drop(&mut self) {
        let _ = self.0.unlock();
    }
}

#[derive(Clone, Debug)]
pub struct Store {
    root: PathBuf,
    policy: Policy,
    caps: Caps,
}

fn unique_name() -> String {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let nanos = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
    format!("{}-{}-{}", std::process::id(), nanos, COUNTER.fetch_add(1, Ordering::Relaxed))
}

impl Store {
    pub fn at(root: impl Into<PathBuf>) -> Store {
        Store { root: root.into(), policy: Policy::standard(), caps: Caps::default() }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn package_dir(&self, id: &ExtensionId, version: craft_ext_core::version::Version) -> PathBuf {
        self.root.join("packages").join(id.as_str()).join(version.to_string())
    }

    fn state_path(&self) -> PathBuf {
        self.root.join(STATE_FILE)
    }

    pub fn state(&self) -> Result<State, StoreError> {
        State::read(&self.state_path()).map_err(StoreError::State)
    }

    fn lock(&self) -> Result<Lock, StoreError> {
        fs::create_dir_all(&self.root).map_err(|_| StoreError::Io)?;
        let file = fs::OpenOptions::new().create(true).truncate(false).write(true).open(self.root.join(".lock")).map_err(|_| StoreError::Io)?;
        let started = Instant::now();
        loop {
            match file.try_lock() {
                Ok(()) => return Ok(Lock(file)),
                Err(TryLockError::WouldBlock) if started.elapsed() < LOCK_WAIT => std::thread::sleep(Duration::from_millis(20)),
                Err(TryLockError::WouldBlock) => return Err(StoreError::Busy),
                Err(TryLockError::Error(_)) => return Err(StoreError::Io),
            }
        }
    }

    /// Runs `change` on the current state under the lock and saves the result with the generation bumped.
    fn update<T>(&self, change: impl FnOnce(&mut State) -> Result<T, StoreError>) -> Result<T, StoreError> {
        let _lock = self.lock()?;
        let mut state = self.state()?;
        let out = change(&mut state)?;
        state.generation = state.generation.saturating_add(1);
        self.save(&state)?;
        Ok(out)
    }

    fn save(&self, state: &State) -> Result<(), StoreError> {
        let bytes = serde_json::to_vec_pretty(state).map_err(|_| StoreError::Io)?;
        craft_atomic::atomic_write(&self.state_path(), &bytes).map_err(|_| StoreError::Io)
    }

    /// Installs a package from an archive held in memory. `tier` is where the caller got it from.
    pub fn install_zip(&self, bytes: &[u8], tier: Tier) -> Result<Installed, StoreError> {
        self.policy.admit(tier).map_err(StoreError::Tier)?;
        let staging = self.root.join("staging").join(unique_name());
        let outcome = self.install_staged(bytes, tier, &staging);
        // On success the folder was moved away and this finds nothing; on failure it removes the debris.
        let _ = fs::remove_dir_all(&staging);
        outcome
    }

    /// Installs a package from a file, refusing to read one past the size limit.
    pub fn install_file(&self, path: &Path, tier: Tier) -> Result<Installed, StoreError> {
        let file = File::open(path).map_err(|_| StoreError::Io)?;
        let mut bytes = Vec::new();
        file.take(self.caps.max_archive + 1).read_to_end(&mut bytes).map_err(|_| StoreError::Io)?;
        if bytes.len() as u64 > self.caps.max_archive {
            return Err(StoreError::Unpack(UnpackError::TooBig));
        }
        self.install_zip(&bytes, tier)
    }

    fn install_staged(&self, bytes: &[u8], tier: Tier, staging: &Path) -> Result<Installed, StoreError> {
        let manifest = unpack_into(bytes, staging, self.caps).map_err(StoreError::Unpack)?;
        self.policy.check_id(tier, &manifest.id).map_err(StoreError::Tier)?;
        if manifest.kind == Kind::Theme {
            let text = fs::read(staging.join(THEME_FILE)).map_err(|_| StoreError::Io)?;
            ThemeFile::parse(&text).map_err(StoreError::BadTheme)?;
        }
        let record = Installed {
            id: manifest.id.clone(),
            version: manifest.version,
            kind: manifest.kind,
            name: manifest.name.clone(),
            tier,
            apps: manifest.compat.keys().cloned().collect(),
        };
        let destination = self.package_dir(&record.id, record.version);
        let mut old_dir = None;
        self.update(|state| {
            if let Some(existing) = state.find(&record.id) {
                if existing.tier == Tier::FirstParty && tier != Tier::FirstParty {
                    return Err(StoreError::WouldReplaceFirstParty);
                }
                if record.version < existing.version {
                    return Err(StoreError::Downgrade);
                }
                if record.version == existing.version {
                    return Err(StoreError::AlreadyInstalled);
                }
                old_dir = Some(self.package_dir(&existing.id, existing.version));
            }
            if let Some(parent) = destination.parent() {
                fs::create_dir_all(parent).map_err(|_| StoreError::Io)?;
            }
            if destination.exists() {
                return Err(StoreError::Io);
            }
            fs::rename(staging, &destination).map_err(|_| StoreError::Io)?;
            state.installed.retain(|i| i.id != record.id);
            state.installed.push(record.clone());
            for app in &record.apps {
                state.enabled.entry(app.clone()).or_default().insert(record.id.clone());
            }
            Ok(())
        })
        .inspect_err(|_| {
            // The package was moved into place but the state could not be saved: put things back.
            if !staging.exists() && destination.exists() && self.state().is_ok_and(|s| s.find(&record.id).is_none_or(|i| i.version != record.version))
            {
                let _ = fs::remove_dir_all(&destination);
            }
        })?;
        if let Some(dir) = old_dir {
            let _ = fs::remove_dir_all(dir);
        }
        Ok(record)
    }

    /// Removes an extension everywhere: its files, its enablement and any app's choice of it as theme.
    pub fn uninstall(&self, id: &ExtensionId) -> Result<(), StoreError> {
        let dir = self.update(|state| {
            let existing = state.find(id).ok_or(StoreError::NotInstalled)?.clone();
            state.installed.retain(|i| &i.id != id);
            state.enabled.values_mut().for_each(|set| {
                set.remove(id);
            });
            state.active_theme.retain(|_, chosen| chosen != id);
            Ok(self.package_dir(&existing.id, existing.version))
        })?;
        let _ = fs::remove_dir_all(dir);
        // Only succeeds when no other version folder is left.
        let _ = fs::remove_dir(self.root.join("packages").join(id.as_str()));
        Ok(())
    }

    /// Turns an installed extension on or off for one app. Turning a theme off also stops the app using it.
    pub fn set_enabled(&self, app: &AppId, id: &ExtensionId, on: bool) -> Result<(), StoreError> {
        self.update(|state| {
            let existing = state.find(id).ok_or(StoreError::NotInstalled)?;
            if !existing.apps.contains(app) {
                return Err(StoreError::NotForThisApp);
            }
            let set = state.enabled.entry(app.clone()).or_default();
            if on {
                set.insert(id.clone());
            } else {
                set.remove(id);
                if state.active_theme.get(app) == Some(id) {
                    state.active_theme.remove(app);
                }
            }
            Ok(())
        })
    }

    /// Chooses which installed theme an app uses, or none to return to its own.
    pub fn set_active_theme(&self, app: &AppId, id: Option<&ExtensionId>) -> Result<(), StoreError> {
        self.update(|state| {
            match id {
                None => {
                    state.active_theme.remove(app);
                }
                Some(id) => {
                    let existing = state.find(id).ok_or(StoreError::NotInstalled)?;
                    if existing.kind != Kind::Theme {
                        return Err(StoreError::NotATheme);
                    }
                    if !existing.apps.contains(app) {
                        return Err(StoreError::NotForThisApp);
                    }
                    if !state.is_enabled(app, id) {
                        return Err(StoreError::NotEnabled);
                    }
                    state.active_theme.insert(app.clone(), id.clone());
                }
            }
            Ok(())
        })
    }
}

#[cfg(test)]
mod tests;
