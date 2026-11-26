use serde::{Deserialize, Serialize};

use crate::{BaseParameter, FunctionGraph, OutputFormat, ParameterInheritance, SizeMode};
use std::collections::BTreeMap;

/// Parameters every node has ("Base parameters")
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct BaseParameters {
    pub size_mode: SizeMode,
    /// log2 offset per axis (relative modes) or log2 size (absolute)
    pub size: [i32; 2],
    pub format: OutputFormat,
    pub format_mode: ParameterInheritance,
    /// Added to the inherited seed, or used directly in absolute mode
    pub seed: i32,
    pub seed_mode: ParameterInheritance,
    pub pixel_size: [f32; 2],
    pub pixel_size_mode: ParameterInheritance,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub functions: BTreeMap<BaseParameter, FunctionGraph>,
    /// Whether sampling wraps at the edges (horizontal, vertical)
    pub tiling: [bool; 2],
    pub tiling_mode: ParameterInheritance,
    /// SBS pixel ratio: zero stretches the image; one keeps pixels square.
    pub pixel_ratio: i32,
    pub pixel_ratio_mode: ParameterInheritance,
}

impl Default for BaseParameters {
    fn default() -> Self {
        BaseParameters {
            size_mode: SizeMode::RelativeToInput,
            size: [0, 0],
            format: OutputFormat::Inherit,
            format_mode: ParameterInheritance::Absolute,
            seed: 0,
            seed_mode: ParameterInheritance::RelativeToInput,
            pixel_size: [1.0; 2],
            pixel_size_mode: ParameterInheritance::RelativeToInput,
            functions: BTreeMap::new(),
            tiling: [true, true],
            tiling_mode: ParameterInheritance::default(),
            pixel_ratio: 0,
            pixel_ratio_mode: ParameterInheritance::default(),
        }
    }
}
