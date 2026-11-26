use std::collections::BTreeMap;

use materialcraft_core::{FunctionGraph, Value};

use super::{EvaluationContext, EvaluationState, FunctionScope, compile};

fn evaluate(operation: &str, name: &str, context: &EvaluationContext<'_>) -> Result<Value, String> {
    let mut graph = FunctionGraph::default();
    graph.output = Some(graph.add(operation, &[], Some(Value::Text(name.into()))));
    compile(&graph, 0)?.evaluate(context, &mut EvaluationState::default())
}

#[test]
fn system_variables_use_caller_scope_even_when_a_local_name_exists() {
    let variables = BTreeMap::from([
        ("$pos".into(), Value::Float2([0.9; 2])),
        ("$depth".into(), Value::Float(3.0)),
        ("$depthpow2".into(), Value::Float(0.125)),
        ("$number".into(), Value::Float(7.0)),
    ]);
    let mut context =
        EvaluationContext { scope: FunctionScope::Node, position: [0.25, 0.75], size: [256.0, 128.0], images: &[], tile: true, variables: &variables };

    for scope in [FunctionScope::Node, FunctionScope::PixelProcessor, FunctionScope::FxMap] {
        context.scope = scope;
        for (name, operation, expected, available) in [
            ("$pos", "get_float2", Value::Float2([0.25, 0.75]), scope != FunctionScope::Node),
            ("$depth", "get_float1", Value::Float(3.0), scope == FunctionScope::FxMap),
            ("$depthpow2", "get_float1", Value::Float(0.125), scope == FunctionScope::FxMap),
            ("$number", "get_float1", Value::Float(7.0), scope == FunctionScope::FxMap),
        ] {
            assert_eq!(evaluate("isdefined", name, &context).unwrap(), Value::Bool(available), "{scope:?}: {name}");
            if available {
                assert_eq!(evaluate(operation, name, &context).unwrap(), expected, "{scope:?}: {name}");
            } else {
                assert!(evaluate(operation, name, &context).unwrap_err().contains("undefined variable"));
            }
        }

        assert_eq!(evaluate("get_float2", "$size", &context).unwrap(), Value::Float2([256.0, 128.0]));
        assert_eq!(evaluate("get_float2", "$sizelog2", &context).unwrap(), Value::Float2([8.0, 7.0]));
    }
}

#[test]
fn system_values_are_typed_finite_and_read_only() {
    for (name, operation, value) in [
        ("$time", "get_float1", Value::Float(f32::NAN)),
        ("$physicalsize", "get_float3", Value::Float3([1.0, f32::INFINITY, 2.0])),
        ("$uvtile", "get_integer2", Value::Float2([0.0; 2])),
        ("$pixelratio", "get_integer1", Value::Int(2)),
        ("$tiling", "get_integer1", Value::Int(4)),
    ] {
        let variables = BTreeMap::from([(name.into(), value)]);
        let context = EvaluationContext { scope: FunctionScope::Node, position: [0.0; 2], size: [16.0; 2], images: &[], tile: true, variables: &variables };

        assert!(evaluate(operation, name, &context).is_err(), "{name}");
    }

    let variables = BTreeMap::new();
    let context = EvaluationContext { scope: FunctionScope::Node, position: [0.0; 2], size: [16.0; 2], images: &[], tile: true, variables: &variables };
    let mut graph = FunctionGraph::default();
    let value = graph.add_constant(Value::Float(9.0));
    graph.output = Some(graph.add("set", &[value], Some(Value::Text("$time".into()))));

    assert!(compile(&graph, 0).unwrap().evaluate(&context, &mut EvaluationState::default()).unwrap_err().contains("user variable name"));
    assert!(evaluate("get_integer1", "$time", &context).unwrap_err().contains("wrong type"));
}
