use materialcraft_formats::sbs::{read_sbs, write_sbs};
use serde_json::json;

#[test]
fn native_graph_attributes_load_and_survive_sbs_save() {
    for kind in ["graph", "function"] {
        let body = if kind == "function" { "<type v=\"256\"/><paramValue><dynamicValue><paramNodes/></dynamicValue></paramValue>" } else { "" };
        let xml = format!(
            r#"<package><content><{kind}><identifier v="Stable_identifier"/><attributes>
            <label v="Readable &amp; non-unique label"/><description v="First line&#10;&lt;p&gt;Second line&lt;/p&gt;"/>
            <category v="Tools/Math"/><author v="Example author"/><authorURL v="https://example.org/?a=1&amp;b=2"/>
            <tags v="math utility"/><group v="Utilities"/><userdata v="pipeline=example"/>
            </attributes>{body}</{kind}></content></package>"#
        );
        let imported = read_sbs(xml.as_bytes(), "attributes.sbs", &|_| None).unwrap().pkg;
        let expected = json!({"label": "Readable & non-unique label", "description": "First line\n<p>Second line</p>",
            "category": "Tools/Math", "author": "Example author", "authorUrl": "https://example.org/?a=1&b=2",
            "tags": "math utility", "group": "Utilities", "userData": "pipeline=example"});

        assert_eq!(serde_json::to_value(&imported.graphs[0].attributes).unwrap(), expected, "{kind} import");
        let bytes = write_sbs(&imported).unwrap();
        let saved = read_sbs(&bytes, "saved.sbs", &|_| None).unwrap().pkg;

        assert_eq!(saved.graphs[0].name, "Stable_identifier");
        assert_eq!(serde_json::to_value(&saved.graphs[0].attributes).unwrap(), expected, "{kind} save");
        let xml = String::from_utf8(bytes).unwrap();
        assert!(xml.contains("<authorURL v="));
        assert!(xml.contains("<userdata v="));
    }
}
