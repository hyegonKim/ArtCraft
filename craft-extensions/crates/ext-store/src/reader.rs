//! The read side apps use: what is installed and enabled for me, and what is my theme.
//!
//! Invariants: needs only the store folder, never the launcher; never writes; anything it cannot read
//! or that fails validation counts as absent (an app keeps its own look rather than showing an error
//! dialog at start-up), and a running app can poll `generation` cheaply to learn that something changed.

use crate::state::{Installed, STATE_FILE, State};
use craft_ext_core::id::{AppId, ExtensionId};
use craft_ext_core::manifest::{Kind, THEME_FILE};
use craft_ext_core::palette::{MAX_THEME_BYTES, Palette, Rgb, ThemeFile};
use std::collections::BTreeMap;
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug)]
pub struct Reader {
    root: PathBuf,
}

/// An installed extension together with where its files are.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Found {
    pub installed: Installed,
    pub dir: PathBuf,
}

/// The theme an app should be wearing.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ActiveTheme {
    pub id: ExtensionId,
    pub name: String,
    pub palette: Palette,
    /// This app's own overrides, by the app's own colour names.
    pub overrides: BTreeMap<String, Rgb>,
}

impl Reader {
    pub fn open(root: impl Into<PathBuf>) -> Reader {
        Reader { root: root.into() }
    }

    fn state(&self) -> Option<State> {
        State::read(&self.root.join(STATE_FILE)).ok()
    }

    /// Changes whenever anything is installed, removed, enabled or chosen. 0 when there is no store
    /// or it cannot be read.
    pub fn generation(&self) -> u64 {
        self.state().map_or(0, |s| s.generation)
    }

    fn dir_of(&self, i: &Installed) -> PathBuf {
        self.root.join("packages").join(i.id.as_str()).join(i.version.to_string())
    }

    /// Extensions of this kind that are installed and switched on for `app`, whose files are still there.
    pub fn installed_for(&self, app: &AppId, kind: Kind) -> Vec<Found> {
        let Some(state) = self.state() else { return Vec::new() };
        state
            .installed
            .iter()
            .filter(|i| i.kind == kind && i.apps.contains(app) && state.is_enabled(app, &i.id))
            .map(|i| Found { installed: i.clone(), dir: self.dir_of(i) })
            .filter(|f| f.dir.is_dir())
            .collect()
    }

    /// The theme `app` has chosen, read and checked again from disk. `None` means "use your own look".
    pub fn active_theme(&self, app: &AppId) -> Option<ActiveTheme> {
        let state = self.state()?;
        let id = state.active_theme.get(app)?;
        let installed = state.find(id)?;
        if installed.kind != Kind::Theme || !state.is_enabled(app, id) {
            return None;
        }
        let theme = read_theme(&self.dir_of(installed).join(THEME_FILE))?;
        Some(ActiveTheme {
            id: installed.id.clone(),
            name: installed.name.clone(),
            overrides: theme.overrides_for(app).cloned().unwrap_or_default(),
            palette: theme.palette,
        })
    }
}

fn read_theme(path: &Path) -> Option<ThemeFile> {
    let mut bytes = Vec::new();
    File::open(path).ok()?.take(MAX_THEME_BYTES as u64 + 1).read_to_end(&mut bytes).ok()?;
    ThemeFile::parse(&bytes).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::Store;
    use crate::testutil::*;
    use craft_ext_core::tier::Tier;
    use std::fs;

    fn app(s: &str) -> AppId {
        AppId::new(s).unwrap()
    }

    fn setup() -> (tempfile::TempDir, Store, Reader) {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("ext");
        (dir, Store::at(&root), Reader::open(&root))
    }

    #[test]
    fn an_absent_store_reads_as_empty_without_any_error() {
        let (_d, _s, r) = setup();
        assert_eq!(r.generation(), 0);
        assert!(r.installed_for(&app("photocraft"), Kind::Theme).is_empty());
        assert!(r.active_theme(&app("photocraft")).is_none());
    }

    #[test]
    fn the_reader_sees_what_the_store_installs_without_the_launcher() {
        let (_d, s, r) = setup();
        let z = theme_package("org.example.ocean", "1.0.0", &["photocraft"], &dark_theme());
        let rec = s.install_zip(&z, Tier::FromFile).unwrap();
        assert_eq!(r.generation(), 1);
        let found = r.installed_for(&app("photocraft"), Kind::Theme);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].installed.id, rec.id);
        assert!(found[0].dir.join("theme.json").is_file());
        assert!(r.installed_for(&app("photocraft"), Kind::Plugin).is_empty(), "other kinds are not listed");
        assert!(r.installed_for(&app("filmcraft"), Kind::Theme).is_empty(), "other apps do not see it");
    }

    #[test]
    fn switching_off_hides_it_and_the_generation_moves_on_every_change() {
        let (_d, s, r) = setup();
        let rec = s.install_zip(&theme_package("org.example.ocean", "1.0.0", &["photocraft"], &dark_theme()), Tier::FromFile).unwrap();
        let pc = app("photocraft");
        let g1 = r.generation();
        s.set_enabled(&pc, &rec.id, false).unwrap();
        assert!(r.installed_for(&pc, Kind::Theme).is_empty());
        assert!(r.generation() > g1);
        let g2 = r.generation();
        s.set_enabled(&pc, &rec.id, true).unwrap();
        assert!(r.generation() > g2 && r.installed_for(&pc, Kind::Theme).len() == 1);
    }

    #[test]
    fn the_active_theme_carries_the_palette_and_only_this_apps_overrides() {
        let (_d, s, r) = setup();
        let z = theme_package("org.example.ocean", "1.0.0", &["photocraft", "launcher"], &dark_theme());
        let rec = s.install_zip(&z, Tier::FromFile).unwrap();
        let pc = app("photocraft");
        assert!(r.active_theme(&pc).is_none(), "nothing is chosen until the person chooses");
        s.set_active_theme(&pc, Some(&rec.id)).unwrap();
        let t = r.active_theme(&pc).unwrap();
        assert_eq!((t.id.as_str(), t.name.as_str()), ("org.example.ocean", "Sample theme"));
        assert!(t.palette.dark);
        assert_eq!(t.overrides.get("chrome"), Some(&Rgb(17, 17, 17)));
        assert!(r.active_theme(&app("launcher")).is_none(), "another app's choice is its own");
        s.set_enabled(&app("launcher"), &rec.id, true).unwrap();
        s.set_active_theme(&app("launcher"), Some(&rec.id)).unwrap();
        assert!(r.active_theme(&app("launcher")).unwrap().overrides.is_empty(), "the launcher has no overrides in this theme");
    }

    #[test]
    fn a_theme_changed_or_removed_on_disk_counts_as_absent() {
        let (_d, s, r) = setup();
        let pc = app("photocraft");
        let rec = s.install_zip(&theme_package("org.example.ocean", "1.0.0", &["photocraft"], &dark_theme()), Tier::FromFile).unwrap();
        s.set_active_theme(&pc, Some(&rec.id)).unwrap();
        let file = s.package_dir(&rec.id, rec.version).join("theme.json");
        fs::write(&file, low_contrast_theme()).unwrap();
        assert!(r.active_theme(&pc).is_none(), "a theme that fails the checks again is not worn");
        fs::write(&file, b"garbage").unwrap();
        assert!(r.active_theme(&pc).is_none());
        fs::write(&file, vec![b' '; MAX_THEME_BYTES + 10]).unwrap();
        assert!(r.active_theme(&pc).is_none());
        fs::remove_dir_all(s.package_dir(&rec.id, rec.version)).unwrap();
        assert!(r.active_theme(&pc).is_none());
        assert!(r.installed_for(&pc, Kind::Theme).is_empty(), "files that are gone are not listed");
    }

    #[test]
    fn a_theme_that_was_switched_off_is_not_worn_even_if_the_choice_lingers() {
        let (_d, s, r) = setup();
        let pc = app("photocraft");
        let rec = s.install_zip(&theme_package("org.example.ocean", "1.0.0", &["photocraft"], &dark_theme()), Tier::FromFile).unwrap();
        s.set_active_theme(&pc, Some(&rec.id)).unwrap();
        assert!(r.active_theme(&pc).is_some());
        let mut state = s.state().unwrap();
        state.enabled.clear();
        fs::write(s.root().join(STATE_FILE), serde_json::to_vec(&state).unwrap()).unwrap();
        assert!(r.active_theme(&pc).is_none());
    }

    #[test]
    fn a_damaged_state_reads_as_nothing_installed() {
        let (_d, s, r) = setup();
        s.install_zip(&theme_package("org.example.ocean", "1.0.0", &["photocraft"], &dark_theme()), Tier::FromFile).unwrap();
        fs::write(s.root().join(STATE_FILE), b"{ broken").unwrap();
        assert_eq!(r.generation(), 0);
        assert!(r.installed_for(&app("photocraft"), Kind::Theme).is_empty());
        assert!(r.active_theme(&app("photocraft")).is_none());
        assert_eq!(fs::read(s.root().join(STATE_FILE)).unwrap(), b"{ broken", "the reader never repairs or writes");
    }

    #[test]
    fn a_choice_pointing_at_something_that_is_not_a_theme_is_ignored() {
        let (_d, s, r) = setup();
        let rec = s.install_zip(&plugin_package("org.example.tone", "1.0.0"), Tier::FromFile).unwrap();
        // Even a perfectly good theme file sitting in the plug-in's folder must not make it a theme.
        fs::write(s.package_dir(&rec.id, rec.version).join("theme.json"), dark_theme()).unwrap();
        let mut state = s.state().unwrap();
        state.active_theme.insert(app("photocraft"), rec.id.clone());
        fs::write(s.root().join(STATE_FILE), serde_json::to_vec(&state).unwrap()).unwrap();
        assert!(r.active_theme(&app("photocraft")).is_none());
    }
}
