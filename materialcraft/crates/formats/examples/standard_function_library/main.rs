#![forbid(unsafe_code)]
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

mod convert;
mod layout;

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests;

use std::{io::Read, path::Path};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut arguments = std::env::args().skip(1);
    let source = arguments.next().ok_or("usage: standard_function_library <source.sbs> [destination.sbs]")?;
    let destination = arguments.next().unwrap_or_else(|| "assets/sbs/functions.sbs".into());
    if arguments.next().is_some() {
        return Err("usage: standard_function_library <source.sbs> [destination.sbs]".into());
    }

    // Bound allocation before parsing potentially untrusted XML
    const MAX_BYTES: u64 = 256 << 20;
    let mut bytes = Vec::new();
    std::fs::File::open(&source)?.take(MAX_BYTES + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > MAX_BYTES {
        return Err("source package exceeds 256 MiB".into());
    }

    let package = convert::package(&bytes, &source)?;
    let nodes: usize = package.graphs.iter().flat_map(|graph| graph.functions()).map(|function| function.nodes.len()).sum();
    let saved = materialcraft_formats::sbs::write_sbs(&package)?;
    materialcraft_formats::write_atomic(Path::new(&destination), &saved)?;

    eprintln!("Saved {} function graphs and {nodes} nodes to {destination}", package.graphs.len());
    eprintln!("Descriptions and layout were rewritten; source graph rights are unchanged.");
    Ok(())
}
