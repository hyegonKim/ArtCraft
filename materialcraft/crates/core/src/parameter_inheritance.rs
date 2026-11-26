use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ParameterInheritance {
    Absolute,
    #[default]
    RelativeToInput,
    RelativeToParent,
}

impl ParameterInheritance {
    pub fn resolve<T: Copy>(self, absolute: T, input: Option<T>, parent: T) -> T {
        match self {
            Self::Absolute => absolute,
            Self::RelativeToInput => input.unwrap_or(parent),
            Self::RelativeToParent => parent,
        }
    }
}
