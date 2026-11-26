//! An isolated catalog fixture shared by the library unit and integration tests.

use materialcraft_core::{Graph, Package};
use materialcraft_engine::{NodeLibrary, Session};
use std::sync::{Arc, OnceLock};

/// Load only the embedded functions, independent of the installed library directory.
pub fn library() -> Arc<NodeLibrary> {
    static LIBRARY: OnceLock<Arc<NodeLibrary>> = OnceLock::new();
    LIBRARY
        .get_or_init(|| {
            let read = materialcraft_formats::sbs::read_sbs(include_bytes!("../../../../assets/sbs/functions.sbs"), "sbs://functions.sbs", &|_| None).unwrap();
            assert!(read.warnings.is_empty(), "{:?}", read.warnings);
            let mut library = NodeLibrary::default();
            library.packages.insert("sbs://functions.sbs".into(), Arc::new(read.pkg));
            Arc::new(library)
        })
        .clone()
}

/// Start an empty material graph with the isolated bundled library.
#[allow(dead_code)]
pub fn session() -> Session {
    let mut package = Package::new("Library tests");
    package.graphs.push(Graph::new("Material"));
    Session::with_library(package, library())
}
