use super::*;
use materialcraft_core::{FunctionGraph, Graph, Package};
use std::sync::Arc;

/// Build a small library with one reusable function and no filesystem dependencies.
fn library(name: &str) -> Arc<NodeLibrary> {
    let mut function = Graph::new(name);
    function.function = Some(FunctionGraph::default());
    let mut package = Package::new("Custom library");
    package.graphs.push(function);
    let mut library = NodeLibrary::default();
    library.packages.insert("sbs://custom.sbs".into(), Arc::new(package));
    Arc::new(library)
}

/// Reload replaces owned sources and removes sources no longer installed.
#[test]
fn reload_updates_and_removes_library_owned_sources() {
    let mut session = Session::with_library(Package::new("Project"), library("Before"));

    replace_library(&mut session, library("After"));

    assert!(session.library_entries().iter().any(|entry| entry.id == "sbs://custom.sbs#After"));
    assert!(!session.library_entries().iter().any(|entry| entry.id == "sbs://custom.sbs#Before"));

    replace_library(&mut session, Arc::new(NodeLibrary::default()));

    assert!(!session.pkg.dependencies.contains_key("sbs://custom.sbs"));
    assert!(!session.library_entries().iter().any(|entry| entry.id.starts_with("sbs://custom.sbs#")));
    assert!(!session.modified);
}

/// Opening a package transfers dependency ownership even after initial session construction.
#[test]
fn reload_keeps_dependencies_supplied_by_later_opened_packages() {
    let mut session = Session::with_library(Package::new("First"), library("Original"));
    let custom = library("User_function");
    let mut package = Package::new("Second");
    package.dependencies = custom.packages.clone();
    session.open_package(package, None).unwrap();

    replace_library(&mut session, library("Replacement"));

    assert!(session.library_entries().iter().any(|entry| entry.id == "sbs://custom.sbs#User_function"));
    assert!(!session.library_entries().iter().any(|entry| entry.id == "sbs://custom.sbs#Replacement"));
}
