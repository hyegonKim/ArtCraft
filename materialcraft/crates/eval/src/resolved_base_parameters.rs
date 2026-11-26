use materialcraft_core::{BaseParameter, BaseParameters, Graph, OutputFormat, ParameterInheritance, Value};
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, Default, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResolvedBaseParameters {
    pub size: [u32; 2],
    pub seed: u32,
    pub format: OutputFormat,
    pub tiling: [bool; 2],
    pub pixel_ratio: i32,
    pub pixel_size: [f32; 2],
}

impl ResolvedBaseParameters {
    pub(crate) fn graph(graph: &Graph, options: &crate::Options<'_>) -> Self {
        let parent = options.parent_base;
        Self {
            size: parent.map_or_else(|| graph.size_px(), |base| base.size),
            seed: parent.map_or((graph.seed as u32).wrapping_mul(0x9e37_79b9), |base| base.seed),
            format: parent.map_or(graph.format, |base| base.format),
            tiling: graph.tiling_mode.resolve(graph.tiling, options.parent_tiling, options.parent_tiling.unwrap_or(graph.tiling)),
            pixel_ratio: graph.pixel_ratio_mode.resolve(graph.pixel_ratio, options.parent_pixel_ratio, options.parent_pixel_ratio.unwrap_or(graph.pixel_ratio)),
            pixel_size: parent.map_or([1.0; 2], |base| base.pixel_size),
        }
    }

    pub(crate) fn resolve(base: &BaseParameters, input: Option<Self>, parent: Self) -> Self {
        let size_base = BaseParameter::OutputSize.inheritance(base).resolve([1; 2], input.map(|base| base.size), parent.size);
        let size = std::array::from_fn(|axis| (f64::from(size_base[axis]) * 2f64.powi(base.size[axis].clamp(-12, 13))).round().clamp(16.0, 8192.0) as u32);
        let inherited_seed = base.seed_mode.resolve(0, input.map(|base| base.seed), parent.seed);
        let inherited_pixel_size = base.pixel_size_mode.resolve([1.0; 2], input.map(|base| base.pixel_size), parent.pixel_size);
        Self {
            size,
            seed: inherited_seed.wrapping_add(base.seed as u32),
            format: BaseParameter::Format.inheritance(base).resolve(base.format, input.map(|base| base.format), parent.format),
            tiling: base.tiling_mode.resolve(base.tiling, input.map(|base| base.tiling), parent.tiling),
            pixel_ratio: base.pixel_ratio_mode.resolve(base.pixel_ratio, input.map(|base| base.pixel_ratio), parent.pixel_ratio),
            pixel_size: std::array::from_fn(|axis| (inherited_pixel_size[axis] * base.pixel_size[axis]).clamp(f32::MIN_POSITIVE, 8192.0)),
        }
    }

    pub(crate) fn variables(self, variables: &mut BTreeMap<String, Value>) {
        variables.insert("$tiling".into(), Value::Int(i32::from(self.tiling[0]) | (i32::from(self.tiling[1]) << 1)));
        variables.insert("$pixelratio".into(), Value::Int(self.pixel_ratio));
    }
}

pub(crate) fn evaluate(
    base: &BaseParameters,
    input: Option<ResolvedBaseParameters>,
    parent: ResolvedBaseParameters,
    variables: &BTreeMap<String, Value>,
) -> Result<(ResolvedBaseParameters, BTreeMap<BaseParameter, Value>), String> {
    if base.functions.is_empty() {
        return Ok((ResolvedBaseParameters::resolve(base, input, parent), BTreeMap::new()));
    }

    // Evaluation mutates literals while borrowing the attached function graphs
    let mut evaluated = BaseParameters {
        size_mode: base.size_mode,
        size: base.size,
        format: base.format,
        format_mode: base.format_mode,
        seed: base.seed,
        seed_mode: base.seed_mode,
        pixel_size: base.pixel_size,
        pixel_size_mode: base.pixel_size_mode,
        functions: BTreeMap::new(),
        tiling: base.tiling,
        tiling_mode: base.tiling_mode,
        pixel_ratio: base.pixel_ratio,
        pixel_ratio_mode: base.pixel_ratio_mode,
    };
    let mut values = BTreeMap::new();
    let mut variables = variables.clone();
    let mut resolved = ResolvedBaseParameters::resolve(&evaluated, input, parent);

    // Size and seed establish the context used by the remaining parameter functions
    for parameter in [
        BaseParameter::RandomSeed,
        BaseParameter::OutputSize,
        BaseParameter::Format,
        BaseParameter::PixelSize,
        BaseParameter::PixelRatio,
        BaseParameter::Tiling,
    ] {
        if let Some(function) = base.functions.get(&parameter) {
            resolved.variables(&mut variables);
            let size = if parameter == BaseParameter::OutputSize && parameter.inheritance(base) != ParameterInheritance::Absolute {
                parameter.inheritance(base).resolve(parent.size, input.map(|base| base.size), parent.size)
            } else {
                resolved.size
            };
            let value = crate::parameter_function::evaluate(function, parameter.value_type(), &variables, resolved.seed, size)
                .map_err(|error| format!("function of `{}`: {error}", parameter.identifier()))?;
            parameter.set_value(&mut evaluated, &value)?;
            parameter.set_inheritance(&mut evaluated, parameter.inheritance(base));
            resolved = ResolvedBaseParameters::resolve(&evaluated, input, parent);
        }

        values.insert(parameter, parameter.value(&evaluated));
    }

    Ok((resolved, values))
}
