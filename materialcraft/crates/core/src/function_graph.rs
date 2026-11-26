use serde::{Deserialize, Serialize};

use crate::Value;

use crate::FunctionNode;

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct FunctionGraph {
    #[serde(default, skip_serializing_if = "crate::CanvasAnnotations::is_empty")]
    pub annotations: crate::CanvasAnnotations,
    pub nodes: Vec<FunctionNode>,
    #[serde(default)]
    pub output: Option<u32>,
}

impl FunctionGraph {
    pub fn node(&self, id: u32) -> Option<&FunctionNode> {
        self.nodes.iter().find(|node| node.id == id)
    }

    pub fn next_id(&self) -> u32 {
        let maximum = self.nodes.iter().map(|node| node.id).max().unwrap_or(0);
        if let Some(next) = maximum.checked_add(1) {
            return next;
        }

        (1..=u32::MAX).find(|id| self.node(*id).is_none()).unwrap_or(0)
    }

    pub fn add(&mut self, operation: &str, inputs: &[u32], value: Option<Value>) -> u32 {
        let id = self.next_id();
        let horizontal_position = self.nodes.len() as f32 * 140.0;
        let node = FunctionNode {
            id,
            missing: None,
            missing_data: String::new(),
            instance: None,
            source: None,
            type_code: 0,
            op: operation.into(),
            inputs: inputs.iter().copied().map(Some).collect(),
            value,
            pos: [horizontal_position, 0.0],
        };

        self.nodes.push(node);
        id
    }

    pub fn constant(value: Value) -> FunctionGraph {
        let mut graph = FunctionGraph::default();
        graph.output = Some(graph.add_constant(value));
        graph
    }

    pub fn input(identifier: &str, value_type: crate::ValueType) -> FunctionGraph {
        use crate::ValueType;

        let operation = match value_type {
            ValueType::Boolean => "get_bool",
            ValueType::Integer => "get_integer1",
            ValueType::Integer2 => "get_integer2",
            ValueType::Integer3 => "get_integer3",
            ValueType::Integer4 => "get_integer4",
            ValueType::Float => "get_float1",
            ValueType::Float2 => "get_float2",
            ValueType::Float3 => "get_float3",
            ValueType::Float4 => "get_float4",
            ValueType::String => "get_string",
        };
        let mut graph = FunctionGraph::default();
        graph.output = Some(graph.add(operation, &[], Some(Value::Text(identifier.into()))));
        graph
    }

    pub fn input_identifier(&self) -> Option<&str> {
        let output = self.node(self.output?)?;
        if self.nodes.len() != 1 || !output.op.starts_with("get_") {
            return None;
        }

        match &output.value {
            Some(Value::Text(identifier)) => Some(identifier),
            _ => None,
        }
    }

    pub fn add_constant(&mut self, value: Value) -> u32 {
        let operation = match &value {
            Value::Float(_) => "const_float1",
            Value::Float2(_) => "const_float2",
            Value::Float3(_) => "const_float3",
            Value::Float4(_) | Value::Color(_) => "const_float4",
            Value::Int(_) | Value::Enum(_) => "const_int1",
            Value::Int2(_) => "const_int2",
            Value::Int3(_) => "const_int3",
            Value::Int4(_) => "const_int4",
            Value::Text(_) => "const_string",
            Value::Bool(_) => "const_bool",
            _ => "const_float1",
        };

        let value = match value {
            Value::Color(color) => Value::Float4(color),
            Value::Enum(index) => Value::Int(index.min(i32::MAX as u32) as i32),
            value => value,
        };

        self.add(operation, &[], Some(value))
    }
}
