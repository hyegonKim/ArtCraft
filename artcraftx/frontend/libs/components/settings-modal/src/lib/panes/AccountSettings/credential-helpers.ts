import { invoke } from "@tauri-apps/api/core";
import { getServicesBasePath } from "@storyteller/model-list";

// Mirrors the Rust `CredentialPayload` (crates/artcraftx/src/commands/credentials).
export interface CredentialPayload {
  /**
   * Stable identity (`credential_{entropy}`) stored inside the credential
   * file. Hidden from users, but the effective primary identifier — pass it
   * back to edit/delete and use it to reference the credential anywhere.
   */
  id: string;
  /** File name within the credentials directory, e.g. `fal_api_2.toml`.
   *  Informational; changes if the user renames the file. */
  file_name: string;
  service: string;
  kind: "cookies" | "api_key";
  name: string | null;
  api_key: string | null;
  api_key_preview: string | null;
  username: string | null;
  email: string | null;
  updated_at: string | null;
  failed_at: string | null;
  succeeded_at: string | null;
  /**
   * Set while the provider has rejected this session and the user must log
   * in again; the backend makes no API calls with the credential until then.
   * Cleared by a successful re-login.
   */
  relogin_required_since: string | null;
}

export interface ServiceMeta {
  value: string;
  label: string;
  logo: string;
  /**
   * For website-login services: the `LoginWebsite` value passed to
   * `open_web_login_command` (mirrors the Rust enum in
   * crates/artcraftx/src/credentials/login_website.rs).
   */
  loginWebsite?: string;
  /**
   * For direct API logins (ArtCraft): show our own username + password form
   * instead of opening a login webview. `value` is the service passed to
   * `artcraft_login_command`.
   */
  passwordLogin?: boolean;
}

/** API-key services offered in the "Add" dropdown. */
export const API_KEY_SERVICES: ServiceMeta[] = [
  { value: "artcraft_api", label: "ArtCraft", logo: "artcraft.svg" },
  { value: "fal_api", label: "FAL", logo: "fal.svg" },
  { value: "openai_api", label: "OpenAI", logo: "openai.svg" },
  { value: "replicate_api", label: "Replicate", logo: "replicate.svg" },
  { value: "xai_api", label: "xAI", logo: "grok.svg" },
];

/**
 * Website-login services offered as logo buttons. ArtCraft uses our own
 * username/password form (`passwordLogin`); the rest open a login webview.
 * "ArtCraft Local Dev" stays last — it's a developer option.
 */
export const WEBSITE_LOGIN_SERVICES: ServiceMeta[] = [
  { value: "artcraft", label: "ArtCraft", logo: "artcraft.svg", passwordLogin: true },
  { value: "runway_cookies", label: "Runway", logo: "runway.svg", loginWebsite: "runway" },
  { value: "higgsfield_cookies", label: "Higgsfield", logo: "higgsfield.svg", loginWebsite: "higgsfield" },
  { value: "openart_cookies", label: "OpenArt", logo: "openart.svg", loginWebsite: "openart" },
  { value: "magnific_cookies", label: "Magnific", logo: "magnific.svg", loginWebsite: "magnific" },
  { value: "midjourney_cookies", label: "Midjourney", logo: "midjourney.svg", loginWebsite: "midjourney" },
  { value: "grok_cookies", label: "Grok", logo: "grok.svg", loginWebsite: "xai" },
  { value: "artcraft_local", label: "ArtCraft Local Dev", logo: "artcraft.svg", passwordLogin: true },
];

// Every service the backend knows about, so hand-written credential files
// for services we don't offer in the "Add" flow still render sensibly.
const ALL_SERVICES: ServiceMeta[] = [
  ...API_KEY_SERVICES,
  ...WEBSITE_LOGIN_SERVICES,
  { value: "artcraft_cookies", label: "ArtCraft", logo: "artcraft.svg" },
  { value: "grok_cookies", label: "Grok", logo: "grok.svg" },
  { value: "worldlabs_cookies", label: "World Labs", logo: "worldlabs.svg" },
];

export const getServiceMeta = (service: string): ServiceMeta => {
  const found = ALL_SERVICES.find((meta) => meta.value === service);
  if (found) return found;
  return { value: service, label: service, logo: "generic.svg" };
};

export const getServiceLogoPath = (service: string): string =>
  `${getServicesBasePath()}/${getServiceMeta(service).logo}`;

// ── Tauri command wrappers ──────────────────────────────────────────────────

export const listCredentials = async (): Promise<CredentialPayload[]> => {
  const response = await invoke<{ credentials: CredentialPayload[] }>(
    "list_credentials_command"
  );
  return response.credentials;
};

export const addApiCredential = async (args: {
  service: string;
  apiKey: string;
  name?: string;
}): Promise<CredentialPayload> => {
  const response = await invoke<{ credential: CredentialPayload }>(
    "add_api_credential_command",
    { service: args.service, apiKey: args.apiKey, name: args.name ?? null }
  );
  return response.credential;
};

export const editApiCredential = async (args: {
  credentialId: string;
  apiKey?: string;
  name?: string;
}): Promise<CredentialPayload> => {
  const response = await invoke<{ credential: CredentialPayload }>(
    "edit_api_credential_command",
    {
      credentialId: args.credentialId,
      apiKey: args.apiKey ?? null,
      name: args.name ?? null,
    }
  );
  return response.credential;
};

export const deleteCredential = async (
  credentialId: string
): Promise<void> => {
  await invoke("delete_credentials_command", { credentialId });
};

/**
 * Open a web-login window for a site. The `website` value must be a
 * `LoginWebsite` variant (see `WEBSITE_LOGIN_SERVICES[].loginWebsite`). The
 * backend opens a fresh webview and, once the user signs in, saves the
 * captured cookies as a credential and emits `refresh_account_state_event`.
 *
 * Pass `credentialId` to refresh that credential in place (a re-login after
 * its session expired) instead of adding a second account for the service.
 */
export const openWebLogin = async (website: string, credentialId?: string): Promise<void> => {
  await invoke("open_web_login_command", { website, credentialId });
};

/**
 * Ask the backend to refresh the Grok statsig material if it's missing or
 * stale. Fire-and-forget: the backend spawns the work and returns immediately,
 * so this never blocks the UI. Safe to call whenever the credentials page opens.
 */
export const refreshGrokStatsig = async (): Promise<void> => {
  try {
    await invoke("refresh_grok_statsig_command");
  } catch (error) {
    console.warn("Failed to trigger Grok statsig refresh:", error);
  }
};

// Mirrors the Rust `ArtcraftLoginCommandError` payload.
export interface ArtcraftLoginError {
  error_type:
    | "invalid_credentials"
    | "account_needs_password"
    | "server_error"
    | "connection_error"
    | "bad_request";
  message: string;
}

/**
 * Log into an ArtCraft account with username/email + password. `service` is
 * `"artcraft"` (production) or `"artcraft_local"` (local dev server). Each
 * successful login stores a NEW credential file. Rejects with an
 * `ArtcraftLoginError` payload on failure.
 */
export const artcraftLogin = async (args: {
  service: string;
  usernameOrEmail: string;
  password: string;
}): Promise<CredentialPayload> => {
  const response = await invoke<{ credential: CredentialPayload }>(
    "artcraft_login_command",
    {
      service: args.service,
      usernameOrEmail: args.usernameOrEmail,
      password: args.password,
    }
  );
  return response.credential;
};
