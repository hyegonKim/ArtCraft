
#[derive(Copy,Clone,Eq,PartialEq,Debug)]
pub enum OsPlatform {
  Linux,
  MacOs,
  Windows,
}

impl OsPlatform {
  pub fn get() -> OsPlatform {
    // Totally scientific.
    // https://doc.rust-lang.org/std/env/consts/constant.OS.html
    match std::env::consts::OS {
      "linux" => OsPlatform::Linux,
      "windows" => OsPlatform::Windows,
      "macos" | "apple" => OsPlatform::MacOs,
      _ => OsPlatform::Linux,
    }
  }
}
