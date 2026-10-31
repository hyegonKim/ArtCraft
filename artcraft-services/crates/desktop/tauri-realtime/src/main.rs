// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::fs::File;

fn main() {
  //// NB: This is a test to see the behavior of the application with dynamic loading modules.
  //// We should be able to execute code before loading CUDA and other dependencies.
  //let r = File::create("program_started.txt");
  //match r {
  //  Err(err) => println!("Failed to create program_started.txt: {}", err),
  //  Ok(_) => {}
  //}
  app_lib::run();
}
