//! local_files_database
//!
//! A content-hash index of local files the app has seen, separate from the
//! tasks database on purpose: this is a rebuildable index (thumbnail
//! accounting today; an (account, checksum) upload-dedup ledger later),
//! with its own write patterns, lock domain, and lifecycle. The tasks
//! database is irreplaceable history; this one can be nuked and re-derived.
//!
//! Unlike the tasks database it keeps a STABLE filename with additive
//! migrations — no version-bumped filenames.

// Never allow these
#![forbid(private_bounds)]
#![forbid(private_interfaces)]
#![forbid(unused_must_use)] // NB: It's unsafe to not close/check some things

// Okay to toggle
#![forbid(unreachable_patterns)]
#![forbid(unused_imports)]
#![forbid(unused_mut)]
#![forbid(unused_variables)]

// Always allow
#![allow(dead_code)]

pub mod connection;
pub mod error;
pub mod queries;
pub mod thumbnail_state;
