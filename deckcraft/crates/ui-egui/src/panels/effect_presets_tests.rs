#![allow(clippy::unwrap_used, clippy::expect_used)]

use deckcraft_engine::Session;
use deckcraft_model::Effects;
use serde_json::json;

use super::effect_presets::shadow_preset_id;

#[test]
fn shadow_preset_id_tracks_built_in_presets() {
    assert_eq!(shadow_preset_id(&Effects::default()), Some("none"));
    let mut s = Session::with_new();
    let id = s.execute("shape.insert", &json!({"preset": "rect", "rect": [72.0, 36.0, 144.0, 108.0]})).unwrap()["id"].as_u64().unwrap();
    s.execute("edit.select", &json!({"ids": [id]})).unwrap();
    for preset in ["outer", "bottom", "center", "inner", "perspective", "none"] {
        s.execute("shape.effects", &json!({"shadow": preset})).unwrap();
        let fx = s.active().unwrap().selected_shapes()[0].effects.clone().unwrap_or_default();
        assert_eq!(shadow_preset_id(&fx), Some(preset));
    }
}
