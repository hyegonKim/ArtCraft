use super::xml::{child_element, child_value, children_named};
use super::{
    function_graph,
    sbs_value::{constant_xml, sbs_constant, to_natural_value},
    sbs_writer::SbsWriter,
    warnings::Warnings,
};
use materialcraft_core::{BaseParameter, BaseParameters, ParameterInheritance};
use roxmltree::Node as XmlNode;
use std::fmt::Write as _;

pub(super) fn read_base_params(params: Option<XmlNode>, warnings: &mut Warnings) -> BaseParameters {
    let mut base = BaseParameters::default();
    for element in params.into_iter().flat_map(|parameters| children_named(parameters, "parameter")) {
        let Some(parameter) = child_value(element, "name").and_then(BaseParameter::named) else { continue };
        let inheritance = match child_value(element, "relativeTo").unwrap_or("1") {
            "0" => ParameterInheritance::Absolute,
            "2" => ParameterInheritance::RelativeToParent,
            _ => ParameterInheritance::RelativeToInput,
        };
        let Some(value) = child_element(element, "paramValue") else { continue };

        if let Some(dynamic) = child_element(value, "dynamicValue") {
            match function_graph::read(dynamic) {
                Ok(function) => {
                    base.functions.insert(parameter, function);
                }
                Err(error) => warnings.add(format!("{} function: {error}", parameter.label())),
            }
        } else if let Some(value) = sbs_constant(value)
            && let Err(error) = parameter.set_value(&mut base, &to_natural_value(&value))
        {
            warnings.add(error);
        }

        parameter.set_inheritance(&mut base, inheritance);
    }

    base
}

pub(super) fn base_params_xml(base: &BaseParameters, writer: &mut SbsWriter) -> Result<String, String> {
    let mut output = String::new();
    let defaults = BaseParameters::default();
    for parameter in BaseParameter::ALL {
        let function = base.functions.get(&parameter);
        let value = parameter.value(base);
        let inheritance = parameter.inheritance(base);
        if function.is_none() && value == parameter.value(&defaults) && inheritance == parameter.inheritance(&defaults) {
            continue;
        }

        let relative_to = match inheritance {
            ParameterInheritance::Absolute => 0,
            ParameterInheritance::RelativeToInput => 1,
            ParameterInheritance::RelativeToParent => 2,
        };
        let body = match function {
            Some(function) => function_graph::write(function, writer)?,
            None => constant_xml(&value).ok_or("invalid base parameter value")?,
        };
        let _ =
            write!(output, "<parameter><name v=\"{}\"/><relativeTo v=\"{relative_to}\"/><paramValue>{body}</paramValue></parameter>", parameter.identifier());
    }

    Ok(output)
}
