use std::fmt::format;
use server_environment::ServerEnvironment;
use crate::http_server::requests::get_request_domain_branding::DomainBranding;


#[derive(Clone)]
pub enum PasswordResetLocation {
  FakeYouProduction,
  FakeYouDevelopment,
  ArtCraftProduction,
  ArtCraftDevelopment,
  CustomUrl(String)
}

pub fn get_password_reset_url(reset_token: &str, location: PasswordResetLocation) -> String {
  let url = match location {
    PasswordResetLocation::ArtCraftProduction => "https://getartcraft.com/forgot-password/verify",
    PasswordResetLocation::FakeYouProduction => "https://fakeyou.com/password-reset/verify",
    PasswordResetLocation::ArtCraftDevelopment |
    PasswordResetLocation::FakeYouDevelopment => {
      "http://localhost:7000/password-reset/verify"
    },
    PasswordResetLocation::CustomUrl(ref url) => url,
  };
  format!("{url}?token={reset_token}")
}