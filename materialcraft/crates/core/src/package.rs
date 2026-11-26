use crate::Resource;
use crate::{CoreError, Graph, Result};
use serde::{Deserialize, Serialize};

pub const PACKAGE_EXT: &str = "sbs";

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Package {
    pub name: String,
    pub graphs: Vec<Graph>,
    /// Explicit SBS group paths, including folders without graphs
    #[serde(default)]
    pub folders: Vec<String>,
    /// Descriptions of SBS groups, keyed by their full folder path.
    #[serde(default, skip_serializing_if = "std::collections::BTreeMap::is_empty")]
    pub folder_descriptions: std::collections::BTreeMap<String, String>,
    #[serde(default)]
    pub resources: Vec<Resource>,
    /// Resolved source packages shared by cooks; SBS stores their file references instead
    #[serde(skip)]
    pub dependencies: std::collections::BTreeMap<String, std::sync::Arc<Package>>,
    /// Format version of `package.json`
    #[serde(default = "version")]
    pub version: u32,
}

fn version() -> u32 {
    1
}

impl Package {
    pub fn new(name: &str) -> Package {
        Package {
            name: name.into(),
            graphs: Vec::new(),
            folders: Vec::new(),
            folder_descriptions: Default::default(),
            resources: Vec::new(),
            version: 1,
            dependencies: Default::default(),
        }
    }

    pub fn folder_paths(&self) -> std::collections::BTreeSet<String> {
        let mut folders = std::collections::BTreeSet::from(["Resources".to_string()]);
        for path in self
            .folders
            .iter()
            .map(String::as_str)
            .chain(self.graphs.iter().map(|graph| graph.folder.as_str()))
            .chain(self.resources.iter().map(|resource| resource.folder.as_str()))
        {
            let mut parent = path;
            while !parent.is_empty() {
                folders.insert(parent.to_string());
                parent = parent.rsplit_once('/').map_or("", |(parent, _)| parent);
            }
        }
        folders
    }

    pub fn graph(&self, i: usize) -> Result<&Graph> {
        self.graphs.get(i).ok_or(CoreError::NoGraph(i))
    }

    pub fn graph_mut(&mut self, i: usize) -> Result<&mut Graph> {
        self.graphs.get_mut(i).ok_or(CoreError::NoGraph(i))
    }

    pub fn graph_by_name(&self, name: &str) -> Option<(usize, &Graph)> {
        self.graphs.iter().enumerate().find(|(_, g)| g.sbs_path() == name).or_else(|| {
            let mut matches = self.graphs.iter().enumerate().filter(|(_, g)| g.name == name);
            let first = matches.next()?;
            matches.next().is_none().then_some(first)
        })
    }

    pub fn resource(&self, name: &str) -> Option<&Resource> {
        self.resources.iter().find(|r| r.name == name)
    }

    pub fn instance_package(&self, node: &crate::Node) -> Option<&Package> {
        match node.instance.as_ref().map(|instance| instance.package.as_str()) {
            Some(file) if !file.is_empty() => self.dependencies.get(file).map(AsRef::as_ref),
            _ => Some(self),
        }
    }

    pub fn instance_graph(&self, node: &crate::Node) -> Option<(usize, &Graph)> {
        let name = node.instance.as_ref().map(|instance| instance.graph.as_str()).or_else(|| match node.params.get("graph") {
            Some(crate::Value::Text(name)) => Some(name.as_str()),
            _ => None,
        })?;
        self.instance_package(node)?.graph_by_name(name)
    }
}
