//! Filter › Plug-ins: one menu item per installed WebAssembly filter plug-in (`plugin.*` engine
//! commands), with a generated dialog and live preview for plug-ins that take parameters.

use serde_json::{Map, Value, json};

use crate::PhotocraftApp;
use crate::menus::MenuItem;

/// Menu ids of installed plug-ins: `plugin.filter.<plug-in id>`.
const PREFIX: &str = "plugin.filter.";

/// Inserts the installed plug-ins into Filter › Plug-ins, above "Install Plug-in…".
pub fn insert_menu_items(app: &PhotocraftApp, items: &mut Vec<MenuItem>) {
    let plugins = photocraft_plugins::registry::list();
    let Some(at) = items.iter().position(|i| i.id == "plugin.install") else { return };
    // `MenuItem::path` is the untranslated English path from the command's engine spec: the menu
    // renderer groups rows by it and translates only at display time. Translating it here left the
    // plug-in rows under a path no menu node matched, so under any non-English language they were
    // never drawn (#2540). Borrow the path of "Install Plug-in…", the item we sit above.
    let Some(path) = items.get(at).map(|i| i.path.clone()) else { return };
    let enabled = app.session.is_enabled("plugin.run");
    let mut extra: Vec<MenuItem> = plugins
        .iter()
        .map(|p| {
            let m = p.manifest();
            let badge = if p.is_native_8bf() && !m.name.contains("[8BF]") { " [8BF]" } else { "" };
            let dots = if m.params.is_empty() || m.name.ends_with('…') { "" } else { "…" };
            MenuItem {
                id: format!("{PREFIX}{}", m.id),
                label: format!("{}{badge}{dots}", m.name),
                path: path.clone(),
                shortcut: None,
                enabled,
                checked: None,
                color: None,
            }
        })
        .collect();
    if !extra.is_empty() {
        extra.push(MenuItem { id: "---".into(), label: "---".into(), path: path.clone(), shortcut: None, enabled: false, checked: None, color: None });
    }
    for (k, it) in extra.into_iter().enumerate() {
        items.insert(at + k, it);
    }
}

/// Enabled state of a plug-in menu item (`None` for other ids).
pub fn is_enabled(app: &PhotocraftApp, id: &str) -> Option<bool> {
    let pid = id.strip_prefix(PREFIX)?;
    Some(photocraft_plugins::registry::get(pid).is_some() && app.session.is_enabled("plugin.run"))
}

/// Runs a plug-in menu item: straight away when it has no parameters (or params were given),
/// else through its dialog. "Install Plug-in…" without params opens its path dialog.
pub fn menu(app: &mut PhotocraftApp, id: &str, params: &Value) -> Option<Result<Value, String>> {
    let given = params.as_object().is_some_and(|o| !o.is_empty());
    if id == "plugin.install" && !given {
        return Some(Ok(json!({"dialog": crate::filter_dialog::open(app, "plugin.install")})));
    }
    let pid = id.strip_prefix(PREFIX)?;
    let Some(plugin) = photocraft_plugins::registry::get(pid) else { return Some(Err(format!("no plug-in {pid:?} is installed"))) };
    let m = plugin.manifest();
    if m.params.is_empty() || given {
        let mut p = params.as_object().cloned().unwrap_or_default();
        p.insert("id".into(), json!(pid));
        return Some(app.run("plugin.run", Value::Object(p)));
    }
    let mut fixed = Map::new();
    fixed.insert("id".into(), json!(pid));
    let d = crate::filter_dialog::open_with_spec(app, "plugin.run", &m.name, &m.params_notation(), fixed);
    Some(Ok(json!({"dialog": d})))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn installed_plugins_appear_under_filter_plugins_and_run() {
        let mut app = PhotocraftApp::new(photocraft_engine::Session::new(), crate::Services::default());
        let ctx = egui::Context::default();
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../plugins/tests/fixtures/invert.wasm");
        app.run("plugin.install", json!({"path": path})).unwrap();
        let items = crate::menus::menu_items(&app);
        let pi: Vec<&MenuItem> = items.iter().filter(|i| i.path == ["Filter", "Plug-ins"]).collect();
        let me = pi.iter().find(|i| i.id == "plugin.filter.org.photocraft.example.invert").expect("plug-in listed");
        assert_eq!(me.label, "Invert (WebAssembly)");
        assert!(!me.enabled, "no document yet");
        assert!(pi.iter().any(|i| i.id == "plugin.install"), "Install Plug-in… is in the same submenu");
        app.run("file.new", json!({"width": 16, "height": 16})).unwrap();
        assert!(crate::menus::is_enabled(&app, "plugin.filter.org.photocraft.example.invert"));
        let px = |app: &PhotocraftApp| app.session.active().unwrap().doc.layers[0].surface().unwrap().pixel(1, 1);
        let before = px(&app);
        crate::menus::invoke(&mut app, &ctx, "plugin.filter.org.photocraft.example.invert", json!({})).unwrap();
        assert!((px(&app)[0] - (1.0 - before[0])).abs() < 1e-6);
        assert!(crate::menus::invoke(&mut app, &ctx, "plugin.filter.not.installed", json!({})).is_err());
        // Install Plug-in… without params opens its dialog instead of failing.
        let r = crate::menus::invoke(&mut app, &ctx, "plugin.install", json!({})).unwrap();
        assert!(r.get("dialog").is_some());
    }

    /// Plug-in rows carry the untranslated English menu path like every other item, so they still
    /// land under Filter › Plug-ins when the UI language translates those menu names (#2540).
    #[test]
    fn plugin_menu_path_stays_untranslated_in_other_languages() {
        if !crate::i18n::needs_catalogs() {
            return;
        }
        let ja = crate::i18n::Lang::from_code("ja").expect("registered language");
        let _lang = crate::i18n::language_scope(ja);
        let mut app = PhotocraftApp::new(photocraft_engine::Session::new(), crate::Services::default());
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../plugins/tests/fixtures/invert.wasm");
        app.run("plugin.install", json!({"path": path})).unwrap();
        let items = crate::menus::menu_items(&app);
        let me = items.iter().find(|i| i.id == "plugin.filter.org.photocraft.example.invert").expect("plug-in listed");
        assert_eq!(me.path, ["Filter", "Plug-ins"], "plug-in row sits where the menu renderer looks for it");
        let install = items.iter().find(|i| i.id == "plugin.install").expect("Install Plug-in… listed");
        assert_eq!(me.path, install.path, "same submenu as Install Plug-in…");
    }

    #[test]
    fn native_8bf_plugins_show_8bf_badge_in_filter_menu() {
        let mut mo = vec![0u8; 0x1000];
        mo[0..4].copy_from_slice(&0xFEED_FACFu32.to_le_bytes());
        mo[4..8].copy_from_slice(&0x0100_000Cu32.to_le_bytes());
        mo[12..16].copy_from_slice(&0x08u32.to_le_bytes());
        mo[16..20].copy_from_slice(&2u32.to_le_bytes());
        mo[20..24].copy_from_slice(&176u32.to_le_bytes());
        mo[32..36].copy_from_slice(&0x19u32.to_le_bytes());
        mo[36..40].copy_from_slice(&152u32.to_le_bytes());
        mo[40..46].copy_from_slice(b"__TEXT");
        mo[56..64].copy_from_slice(&0x0000_0001_0000_0000u64.to_le_bytes());
        mo[64..72].copy_from_slice(&0x1000u64.to_le_bytes());
        mo[80..88].copy_from_slice(&0x0800u64.to_le_bytes());
        mo[96..100].copy_from_slice(&1u32.to_le_bytes());
        mo[104..110].copy_from_slice(b"__text");
        mo[120..126].copy_from_slice(b"__TEXT");
        mo[136..144].copy_from_slice(&0x0000_0001_0000_0400u64.to_le_bytes());
        mo[144..152].copy_from_slice(&8u64.to_le_bytes());
        mo[152..156].copy_from_slice(&0x0400u32.to_le_bytes());
        mo[184..188].copy_from_slice(&0x02u32.to_le_bytes());
        mo[188..192].copy_from_slice(&24u32.to_le_bytes());
        mo[192..196].copy_from_slice(&0x0800u32.to_le_bytes());
        mo[196..200].copy_from_slice(&1u32.to_le_bytes());
        mo[200..204].copy_from_slice(&0x0820u32.to_le_bytes());
        mo[204..208].copy_from_slice(&0x0100u32.to_le_bytes());
        mo[0x0400..0x0404].copy_from_slice(&0x7900_007Fu32.to_le_bytes()); // strh wzr, [x3]
        mo[0x0404..0x0408].copy_from_slice(&0xD65F_03C0u32.to_le_bytes()); // ret
        mo[0x0821..0x082C].copy_from_slice(b"_PluginMain");
        mo[0x0800..0x0804].copy_from_slice(&1u32.to_le_bytes());
        mo[0x0804] = 0x0F;
        mo[0x0805] = 1;
        mo[0x0808..0x0810].copy_from_slice(&0x0000_0001_0000_0400u64.to_le_bytes());

        let plugin = photocraft_plugins::Plugin::load_with_hint(&mo, Some("SolarizeMenu.8bf"), photocraft_plugins::Limits::default()).unwrap();
        let pid = plugin.id().to_string();
        photocraft_plugins::registry::install(plugin);

        let app = PhotocraftApp::new(photocraft_engine::Session::new(), crate::Services::default());
        let items = crate::menus::menu_items(&app);
        let me = items.iter().find(|i| i.id == format!("plugin.filter.{pid}")).expect("native .8bf listed");
        assert_eq!(me.label, "SolarizeMenu [8BF]");
        photocraft_plugins::registry::remove(&pid);
    }
}
