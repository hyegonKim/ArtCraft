use crate::email::get_password_reset_url::{get_password_reset_url, PasswordResetLocation};
use crate::http_server::requests::get_request_domain_branding::DomainBranding;
use errors::AnyhowResult;
use resend_rs::types::CreateEmailBaseOptions;
use resend_rs::Resend;
use server_environment::ServerEnvironment;

pub struct SendPasswordResetEmailArgs<'a> {
  pub email_address_destination: &'a str,
  pub verification_token: &'a str,
  pub resend_api_key: &'a str,
  pub server_environment: ServerEnvironment,
  pub domain_branding: DomainBranding,
}

pub async fn send_password_reset_email(
  args: SendPasswordResetEmailArgs<'_>
) -> AnyhowResult<()> {
  let resend = Resend::new(&args.resend_api_key);

  let to = [args.email_address_destination];

  let subject = "ArtCraft Password Reset";

  let from_address = match args.domain_branding {
    DomainBranding::ArtCraftDotAi |
    DomainBranding::GetArtCraft => "ArtCraft <noreply@getartcraft.com>",
    DomainBranding::FakeYou => "FakeYou <noreply@fakeyou.com>",
    DomainBranding::Storyteller => "FakeYou <noreply@fakeyou.com>",
  };

  let platform = match args.domain_branding {
    DomainBranding::ArtCraftDotAi |
    DomainBranding::GetArtCraft => "ArtCraft",
    DomainBranding::FakeYou => "FakeYou",
    DomainBranding::Storyteller => "Storyteller.ai",
  };

  let team_name = match args.domain_branding {
    DomainBranding::ArtCraftDotAi |
    DomainBranding::GetArtCraft => "ArtCraft Team",
    DomainBranding::FakeYou => "FakeYou Team",
    DomainBranding::Storyteller => "Storyteller.ai Team",
  };

  let url = match (args.domain_branding, args.server_environment) {
    // Legacy FakeYou
    (DomainBranding::FakeYou, ServerEnvironment::Development) => PasswordResetLocation::FakeYouDevelopment,
    (DomainBranding::FakeYou, ServerEnvironment::Production) => PasswordResetLocation::FakeYouProduction,
    // Everything else "development" is ArtCraft Development
    (_, ServerEnvironment::Development) => PasswordResetLocation::ArtCraftDevelopment,
    // Everything else "production" is ArtCraft Production
    (_, ServerEnvironment::Production) => PasswordResetLocation::ArtCraftProduction,
  };

  let link = get_password_reset_url(args.verification_token, url);
  let code = args.verification_token;

  let html_message = format!(r#"
      We received a request to reset your password on {platform}.
      If this wasn't you, you can safely ignore this email.
      <br />
      <br />
      <a href="{link}">Click here to reset your password!</a>
      <br />
      <br />
      If you can't click the link, here's the secret reset code: {code}
      <br />
      <br />
      Thank You,
      <br />
      <br />
      {team_name}
    "#);

  let email = CreateEmailBaseOptions::new(from_address, to, subject)
      .with_html(&html_message);

  let _email = resend.emails.send(email).await?;

  Ok(())
}