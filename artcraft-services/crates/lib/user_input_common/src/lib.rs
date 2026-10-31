#![forbid(dead_code)]
#![forbid(non_snake_case)]
#![forbid(private_bounds)]
#![forbid(private_interfaces)]
#![forbid(unused_imports)]
#![forbid(unused_must_use)] // Important
#![forbid(unused_mut)]
#![forbid(unused_variables)]
#![forbid(warnings)]

pub (crate) const BANNED_SLURS : &str = include_str!("../../../../includes/binary_includes/safety/banned_slurs.txt");

pub mod check_for_slurs;
pub mod latin_alphabet;
pub mod validate_user_provided_ip_address;
