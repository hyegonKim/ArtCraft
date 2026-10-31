//! hashing
//!
//! The purpose of this library is to collect hashing-related functions and recipes that are
//! reusable.
//!

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
#![allow(non_snake_case)]

// TODO(bt,2023-10-05): Should this library contain any "user libraries"
//  (like password hashing and gravatar), or focus on purely hashing functions?

// NB: Because we have our own bcrypt module
extern crate bcrypt as bcrypt_lib;

pub mod bcrypt;

// NB(bt,2023-10-05): md5 is broken and should not be used for anything seriously.
// We're using it in this case to support email gravatar hashing, which honestly,
// should be carefully reconsidered since all of these hashes are broken and exposed
// to the public.
pub mod md5;

pub mod sha256;
