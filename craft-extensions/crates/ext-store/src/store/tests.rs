use super::*;
use crate::testutil::*;
use craft_ext_core::version::Version;

fn id(s: &str) -> ExtensionId {
    ExtensionId::new(s).unwrap()
}

fn app(s: &str) -> AppId {
    AppId::new(s).unwrap()
}

fn store() -> (tempfile::TempDir, Store) {
    let dir = tempfile::tempdir().unwrap();
    let s = Store::at(dir.path().join("ext"));
    (dir, s)
}

fn theme(id_: &str, version: &str) -> Vec<u8> {
    theme_package(id_, version, &["photocraft", "launcher"], &dark_theme())
}

fn package_folders(s: &Store) -> Vec<String> {
    let mut out = Vec::new();
    let base = s.root().join("packages");
    if let Ok(ids) = fs::read_dir(&base) {
        for i in ids.flatten() {
            if let Ok(vs) = fs::read_dir(i.path()) {
                for v in vs.flatten() {
                    out.push(format!("{}/{}", i.file_name().to_string_lossy(), v.file_name().to_string_lossy()));
                }
            }
        }
    }
    out.sort();
    out
}

fn staging_is_empty(s: &Store) -> bool {
    fs::read_dir(s.root().join("staging")).map(|mut d| d.next().is_none()).unwrap_or(true)
}

#[test]
fn installing_a_theme_puts_it_in_place_and_enables_it_for_its_apps() {
    let (_d, s) = store();
    let rec = s.install_zip(&theme("org.example.ocean", "1.0.0"), Tier::FromFile).unwrap();
    assert_eq!((rec.id.as_str(), rec.version, rec.kind, rec.tier), ("org.example.ocean", Version::new(1, 0, 0), Kind::Theme, Tier::FromFile));
    assert_eq!(package_folders(&s), vec!["org.example.ocean/1.0.0"]);
    assert!(s.package_dir(&rec.id, rec.version).join("theme.json").is_file());
    let state = s.state().unwrap();
    assert_eq!(state.generation, 1);
    assert!(state.is_enabled(&app("photocraft"), &rec.id) && state.is_enabled(&app("launcher"), &rec.id));
    assert!(!state.is_enabled(&app("filmcraft"), &rec.id));
    assert!(state.active_theme.is_empty(), "installing never changes anyone's look");
    assert!(staging_is_empty(&s));
}

#[test]
fn a_newer_version_replaces_the_old_one_and_downgrades_are_refused() {
    let (_d, s) = store();
    s.install_zip(&theme("org.example.ocean", "1.0.0"), Tier::FromFile).unwrap();
    s.install_zip(&theme("org.example.ocean", "1.1.0"), Tier::FromFile).unwrap();
    assert_eq!(package_folders(&s), vec!["org.example.ocean/1.1.0"], "the old version's files are gone");
    assert_eq!(s.state().unwrap().installed.len(), 1);
    assert_eq!(s.state().unwrap().generation, 2);
    assert_eq!(s.install_zip(&theme("org.example.ocean", "1.0.5"), Tier::FromFile), Err(StoreError::Downgrade));
    assert_eq!(s.install_zip(&theme("org.example.ocean", "1.1.0"), Tier::FromFile), Err(StoreError::AlreadyInstalled));
    assert_eq!(s.state().unwrap().generation, 2, "refused installs change nothing");
    assert_eq!(package_folders(&s), vec!["org.example.ocean/1.1.0"]);
    assert!(staging_is_empty(&s));
}

#[test]
fn a_file_cannot_replace_a_first_party_extension_or_use_its_names() {
    let (_d, s) = store();
    s.install_zip(&theme("org.example.official", "1.0.0"), Tier::FirstParty).unwrap();
    assert_eq!(s.install_zip(&theme("org.example.official", "2.0.0"), Tier::FromFile), Err(StoreError::WouldReplaceFirstParty));
    assert_eq!(s.install_zip(&theme("org.storytold.fake", "1.0.0"), Tier::FromFile), Err(StoreError::Tier(TierError::ReservedNamespace)));
    assert!(s.install_zip(&theme("org.storytold.real", "1.0.0"), Tier::FirstParty).is_ok());
    assert_eq!(s.state().unwrap().installed.len(), 2);
    assert!(staging_is_empty(&s));
}

#[cfg(not(feature = "third-party"))]
#[test]
fn third_party_is_refused_before_anything_is_unpacked() {
    let (_d, s) = store();
    assert_eq!(s.install_zip(&theme("org.example.ocean", "1.0.0"), Tier::ReviewedThirdParty), Err(StoreError::Tier(TierError::ThirdPartyClosed)));
    assert!(!s.root().exists(), "not even the store folder was created");
}

#[test]
fn a_theme_nobody_can_read_is_refused_and_leaves_nothing_behind() {
    let (_d, s) = store();
    let bad = theme_package("org.example.dim", "1.0.0", &["photocraft"], &low_contrast_theme());
    assert!(matches!(s.install_zip(&bad, Tier::FromFile), Err(StoreError::BadTheme(_))));
    assert!(package_folders(&s).is_empty() && staging_is_empty(&s));
    assert_eq!(s.state().unwrap(), State::default());
}

#[test]
fn a_hostile_archive_is_refused_and_leaves_nothing_behind() {
    let (_d, s) = store();
    let z = zip_of(&[("craft-extension.json", b"{}"), ("../escape.txt", b"x")]);
    assert!(matches!(s.install_zip(&z, Tier::FromFile), Err(StoreError::Unpack(_))));
    assert!(matches!(s.install_zip(b"not a zip", Tier::FromFile), Err(StoreError::Unpack(UnpackError::NotAnArchive))));
    assert!(package_folders(&s).is_empty() && staging_is_empty(&s));
    assert_eq!(s.state().unwrap().generation, 0);
}

#[test]
fn a_plugin_installs_with_its_interface_and_is_enabled_for_its_app() {
    let (_d, s) = store();
    let rec = s.install_zip(&plugin_package("org.example.tone", "1.0.0"), Tier::FromFile).unwrap();
    assert_eq!(rec.kind, Kind::Plugin);
    assert!(s.package_dir(&rec.id, rec.version).join("plugin.wasm").is_file());
    assert!(s.state().unwrap().is_enabled(&app("photocraft"), &rec.id));
}

#[test]
fn an_existing_package_folder_is_never_overwritten() {
    let (_d, s) = store();
    let dir = s.package_dir(&id("org.example.ocean"), Version::new(1, 0, 0));
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("precious"), b"keep").unwrap();
    assert_eq!(s.install_zip(&theme("org.example.ocean", "1.0.0"), Tier::FromFile), Err(StoreError::Io));
    assert_eq!(fs::read(dir.join("precious")).unwrap(), b"keep");
    assert!(staging_is_empty(&s));
    assert_eq!(s.state().unwrap().generation, 0);
}

#[test]
fn an_existing_empty_package_folder_is_not_replaced_either() {
    let (_d, s) = store();
    let dir = s.package_dir(&id("org.example.ocean"), Version::new(1, 0, 0));
    fs::create_dir_all(&dir).unwrap();
    assert_eq!(s.install_zip(&theme("org.example.ocean", "1.0.0"), Tier::FromFile), Err(StoreError::Io));
    assert!(fs::read_dir(&dir).unwrap().next().is_none());
    assert_eq!(s.state().unwrap().generation, 0);
}

#[test]
fn uninstall_removes_files_enablement_and_theme_choice_everywhere() {
    let (_d, s) = store();
    let rec = s.install_zip(&theme("org.example.ocean", "1.0.0"), Tier::FromFile).unwrap();
    s.set_active_theme(&app("photocraft"), Some(&rec.id)).unwrap();
    s.uninstall(&rec.id).unwrap();
    assert!(package_folders(&s).is_empty());
    assert!(!s.root().join("packages").join("org.example.ocean").exists(), "the empty id folder goes too");
    let state = s.state().unwrap();
    assert!(state.installed.is_empty() && state.active_theme.is_empty());
    assert!(!state.is_enabled(&app("photocraft"), &rec.id));
    assert_eq!(state.generation, 3);
    assert_eq!(s.uninstall(&rec.id), Err(StoreError::NotInstalled));
    assert_eq!(s.state().unwrap().generation, 3, "a refused change does not bump the generation");
}

#[test]
fn enabling_and_choosing_a_theme_follow_the_rules() {
    let (_d, s) = store();
    let th = s.install_zip(&theme_package("org.example.ocean", "1.0.0", &["photocraft"], &dark_theme()), Tier::FromFile).unwrap();
    let pl = s.install_zip(&plugin_package("org.example.tone", "1.0.0"), Tier::FromFile).unwrap();
    let pc = app("photocraft");
    assert_eq!(s.set_active_theme(&pc, Some(&pl.id)), Err(StoreError::NotATheme));
    assert_eq!(s.set_active_theme(&app("filmcraft"), Some(&th.id)), Err(StoreError::NotForThisApp));
    assert_eq!(s.set_enabled(&app("filmcraft"), &th.id, true), Err(StoreError::NotForThisApp));
    assert_eq!(s.set_active_theme(&pc, Some(&id("org.example.nope"))), Err(StoreError::NotInstalled));
    s.set_enabled(&pc, &th.id, false).unwrap();
    assert_eq!(s.set_active_theme(&pc, Some(&th.id)), Err(StoreError::NotEnabled));
    s.set_enabled(&pc, &th.id, true).unwrap();
    s.set_active_theme(&pc, Some(&th.id)).unwrap();
    assert_eq!(s.state().unwrap().active_theme.get(&pc), Some(&th.id));
    s.set_enabled(&pc, &th.id, false).unwrap();
    assert!(s.state().unwrap().active_theme.is_empty(), "switching a theme off stops the app using it");
    s.set_enabled(&pc, &th.id, true).unwrap();
    s.set_active_theme(&pc, Some(&th.id)).unwrap();
    s.set_active_theme(&pc, None).unwrap();
    assert!(s.state().unwrap().active_theme.is_empty());
}

#[test]
fn many_installs_at_once_lose_nothing() {
    let (_d, s) = store();
    let n = 8;
    let barrier = std::sync::Arc::new(std::sync::Barrier::new(n));
    let handles: Vec<_> = (0..n)
        .map(|i| {
            let s = s.clone();
            let barrier = barrier.clone();
            std::thread::spawn(move || {
                let z = theme(&format!("org.example.t{i}"), "1.0.0");
                barrier.wait();
                s.install_zip(&z, Tier::FromFile)
            })
        })
        .collect();
    for h in handles {
        h.join().unwrap().unwrap();
    }
    let state = s.state().unwrap();
    assert_eq!(state.installed.len(), n, "every install is recorded");
    assert_eq!(state.generation, n as u64, "every change bumped the generation exactly once");
    assert_eq!(package_folders(&s).len(), n);
    assert!(staging_is_empty(&s));
}

#[test]
fn a_damaged_or_newer_state_file_blocks_changes_and_is_left_untouched() {
    let (_d, s) = store();
    fs::create_dir_all(s.root()).unwrap();
    let path = s.root().join(STATE_FILE);
    fs::write(&path, b"{ broken").unwrap();
    assert_eq!(s.install_zip(&theme("org.example.ocean", "1.0.0"), Tier::FromFile), Err(StoreError::State(StateError::Corrupt)));
    assert_eq!(fs::read(&path).unwrap(), b"{ broken");
    assert!(package_folders(&s).is_empty() && staging_is_empty(&s));
    fs::write(&path, br#"{"schema":2,"generation":1}"#).unwrap();
    assert_eq!(s.uninstall(&id("org.example.ocean")), Err(StoreError::State(StateError::Newer(2))));
}

#[test]
fn install_file_reads_a_file_and_refuses_a_giant_one() {
    let (d, s) = store();
    let path = d.path().join("ocean.craftext");
    fs::write(&path, theme("org.example.ocean", "1.0.0")).unwrap();
    assert!(s.install_file(&path, Tier::FromFile).is_ok());
    let missing = d.path().join("missing.craftext");
    assert_eq!(s.install_file(&missing, Tier::FromFile), Err(StoreError::Io));
    let mut small = Store::at(d.path().join("ext2"));
    small.caps.max_archive = 10;
    assert_eq!(small.install_file(&path, Tier::FromFile), Err(StoreError::Unpack(UnpackError::TooBig)));
}

#[test]
fn every_message_is_plain_language() {
    let all = [
        StoreError::Busy,
        StoreError::Downgrade,
        StoreError::AlreadyInstalled,
        StoreError::WouldReplaceFirstParty,
        StoreError::NotInstalled,
        StoreError::NotForThisApp,
        StoreError::NotATheme,
        StoreError::NotEnabled,
        StoreError::Io,
    ];
    for e in all {
        let text = e.to_string().to_lowercase();
        for word in ["tier", "lock", "mutex", "state", "generation", "manifest", "staging", "io", "wasm"] {
            assert!(!text.split(|c: char| !c.is_alphanumeric()).any(|w| w == word), "{e:?}: {text}");
        }
    }
}
