//! Desktop font-store downloads (`font.download`): opt-in per-user installation of missing Google
//! Fonts / Noto families via `craft_font_store`.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

use craft_font_catalog::FontEnv;
use craft_font_store::{Caller, Cancel, FontStore, Index, Network, PINNED_INDEX};
use serde_json::{Value, json};

use super::Download;
use crate::cmd::CommandSpec;
use crate::params::req_string;
use crate::{EngineError, Result, Session, bad, spec};

fn store_override() -> &'static Mutex<Option<PathBuf>> {
    static DIR: OnceLock<Mutex<Option<PathBuf>>> = OnceLock::new();
    DIR.get_or_init(|| Mutex::new(None))
}

fn jobs() -> &'static Mutex<BTreeMap<String, (String, Download)>> {
    static JOBS: OnceLock<Mutex<BTreeMap<String, (String, Download)>>> = OnceLock::new();
    JOBS.get_or_init(|| Mutex::new(BTreeMap::new()))
}

fn bundled_index() -> &'static Option<Index> {
    static IDX: OnceLock<Option<Index>> = OnceLock::new();
    IDX.get_or_init(|| Index::bundled().ok())
}

/// Point the font store at `dir` (used by tests so user font preferences are untouched).
pub fn use_store_dir(dir: PathBuf) {
    *store_override().lock().unwrap_or_else(std::sync::PoisonError::into_inner) = Some(dir);
    jobs().lock().unwrap_or_else(std::sync::PoisonError::into_inner).clear();
}

fn open_store(caller: Caller) -> Option<FontStore> {
    if let Some(dir) = store_override().lock().unwrap_or_else(std::sync::PoisonError::into_inner).clone() {
        return Some(FontStore::new(dir, caller));
    }
    let env = FontEnv::from_process();
    match caller {
        Caller::User => FontStore::for_user(&env),
        Caller::Agent => FontStore::for_agent(&env),
    }
}

fn current_index(store: Option<&FontStore>) -> Option<Index> {
    if let Some(st) = store {
        return Some(st.cached_indexes(&PINNED_INDEX).unwrap_or_else(|| st.offline_indexes()));
    }
    bundled_index().clone()
}

/// Display name of the font store source that offers `family` (`Google Fonts`, `Noto`), if any.
pub fn source(family: &str) -> Option<String> {
    let store = open_store(Caller::User);
    let idx = current_index(store.as_ref())?;
    let fam = idx.family(family.trim())?;
    idx.source_of(fam).map(|s| s.name.clone())
}

/// Active and recently finished font downloads.
pub fn downloads() -> Vec<(String, Download)> {
    jobs()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .values()
        .cloned()
        .collect()
}

/// Whether the user has allowed downloading fonts.
pub fn downloads_allowed() -> bool {
    crate::file_access::network_refused().is_none()
        && open_store(Caller::User).is_some_and(|st| st.network() == Network::On)
}

/// Turn font downloads on (from the user's explicit consent in the Missing fonts window).
pub fn allow_downloads() -> std::result::Result<(), String> {
    if let Some(why) = crate::file_access::network_refused() {
        return Err(why);
    }
    let st = open_store(Caller::User).ok_or_else(|| "no user font folder is available on this system".to_string())?;
    st.enable_network().map_err(|e| e.to_string())
}

fn set_job(family: &str, state: Download) {
    let key = family.trim().to_lowercase();
    jobs()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .insert(key, (family.trim().to_string(), state));
}

fn font_download_cmd(s: &mut Session, p: &Value) -> Result<Value> {
    let cmd = "font.download";
    let family = req_string(cmd, p, "family")?;
    let family = family.trim().to_string();
    if family.is_empty() {
        return Err(bad(cmd, "`family` must not be empty"));
    }
    let key = family.to_lowercase();

    // If a background download already completed, clear its job entry and refit open text layers.
    let finished = {
        let mut guard = jobs().lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        if matches!(guard.get(&key), Some((_, Download::Installed))) {
            guard.remove(&key);
            true
        } else {
            false
        }
    };
    if finished {
        let refitted = super::refit_open(s, None, Some(std::slice::from_ref(&family)));
        return Ok(json!({"family": family, "status": "installed", "refitted": refitted}));
    }

    if s.agent.is_some() {
        return Err(EngineError::Disabled(cmd.into(), "agents cannot start network font downloads".into()));
    }
    if let Some(why) = crate::file_access::network_refused() {
        return Err(EngineError::Disabled(cmd.into(), why));
    }
    let store = open_store(Caller::User)
        .ok_or_else(|| EngineError::Disabled(cmd.into(), "no user font folder is available".into()))?;
    if store.network() != Network::On {
        return Err(EngineError::Disabled(
            cmd.into(),
            "downloading fonts is turned off: allow it in the Missing fonts window first".into(),
        ));
    }
    let index = store.cached_indexes(&PINNED_INDEX).unwrap_or_else(|| store.offline_indexes());
    let fam = index
        .family(&family)
        .cloned()
        .ok_or_else(|| bad(cmd, format!("the font store doesn't have `{family}`")))?;

    let already_running = jobs()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .get(&key)
        .is_some_and(|(_, d)| matches!(d, Download::Running { .. }));
    if already_running {
        return Ok(json!({"family": fam.family, "status": "downloading"}));
    }

    let canonical = fam.family.clone();
    set_job(&canonical, Download::Running { done: 0, total: 0 });
    let job_name = canonical.clone();
    std::thread::spawn(move || {
        let cancel = Cancel::new();
        let mut on_progress = |prog: craft_font_store::Progress| {
            set_job(&job_name, Download::Running { done: prog.done, total: prog.total });
        };
        match store.install_family(&index, &fam, &cancel, &mut on_progress) {
            Ok(_) => {
                let _ = figcraft_text::rescan_system_fonts();
                set_job(&job_name, Download::Installed);
            }
            Err(err) => {
                set_job(&job_name, Download::Failed(err.to_string()));
            }
        }
    });

    Ok(json!({"family": canonical, "status": "downloading"}))
}

pub fn specs() -> Vec<CommandSpec> {
    vec![spec!(
        "font.download",
        "Download font",
        "{family}  installs a missing font family from the font store once the user has allowed font downloads, or refits open text once its install finishes → {family, status: downloading|installed, refitted?}",
        false,
        font_download_cmd
    )]
}
