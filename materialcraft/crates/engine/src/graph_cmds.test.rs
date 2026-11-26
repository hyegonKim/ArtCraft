#[cfg(test)]
mod delete_tests {
    use crate::{execute, sample_session};
    use serde_json::json;

    /// Deleting a graph that sits before the open one must not switch the user to another graph.
    #[test]
    fn deleting_an_earlier_graph_keeps_the_open_graph_open() {
        let mut s = sample_session();
        let run = |s: &mut crate::Session, id: &str, p: serde_json::Value| execute(s, id, &p).unwrap_or_else(|e| panic!("{id}: {e}"));
        run(&mut s, "file.new", json!({"template": "empty", "name": "Mine"}));
        for _ in 0..3 {
            run(&mut s, "graph.duplicate", json!({}));
        }
        let count = s.pkg.graphs.len();
        assert!(count >= 4, "need four graphs, got {count}");
        run(&mut s, "graph.open", json!({"graph": 2}));
        let open_name = s.pkg.graphs[s.graph].name.clone();
        run(&mut s, "graph.delete", json!({"graph": 0}));
        assert_eq!(s.pkg.graphs.len(), count - 1);
        assert_eq!(s.pkg.graphs[s.graph].name, open_name, "the open graph changed");
    }

    #[test]
    fn presets_export_and_import_json_with_undo() {
        let mut s = sample_session();
        let run = |s: &mut crate::Session, id: &str, p: serde_json::Value| execute(s, id, &p).unwrap_or_else(|e| panic!("{id}: {e}"));
        run(&mut s, "graph.exposed.add", json!({"id": "roughness", "default": 0.35}));
        run(&mut s, "graph.exposed.set", json!({"id": "roughness", "value": 0.75}));
        run(&mut s, "graph.preset.save", json!({"name": "Weathered"}));

        let exported = run(&mut s, "graph.preset.exportJson", json!({}));
        assert_eq!(exported["format"], "materialcraft-presets-v1");
        let payload = exported["json"].as_str().unwrap().to_string();

        run(&mut s, "graph.preset.delete", json!({"name": "Weathered"}));
        run(&mut s, "graph.exposed.set", json!({"id": "roughness", "value": 0.1}));
        assert!(s.graph().unwrap().presets.is_empty());

        let imported = run(&mut s, "graph.preset.importJson", json!({"json": payload, "apply": true}));
        assert_eq!(imported["imported"], 1);
        assert_eq!(s.graph().unwrap().presets.len(), 1);
        assert_eq!(s.graph().unwrap().presets[0].name, "Weathered");
        let applied = s.graph().unwrap().input_value("roughness").unwrap().as_f32().unwrap();
        assert!((applied - 0.75).abs() < 1e-5);

        run(&mut s, "edit.undo", json!({}));
        assert!(s.graph().unwrap().presets.is_empty());
    }
}
