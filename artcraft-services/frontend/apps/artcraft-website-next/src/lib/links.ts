// Cross-app link configuration, ported from the Vite site's config/links.ts.
// The marketing site links out to the webapp for all product features.

// Canonical origin for metadata, sitemaps and share links.
export const SITE_URL = "https://getartcraft.com";

/** Absolute URL for a site path; already-absolute URLs (media CDN) pass through. */
export function siteUrl(path: string): string {
  if (/^https?:\/\//.test(path)) return path;
  return `${SITE_URL}${path.startsWith("/") ? path : `/${path}`}`;
}

export const WEBAPP_URL =
  process.env.NEXT_PUBLIC_WEBAPP_URL ?? "https://app.getartcraft.com/";

// Marketing media lives in this app's public/ directory so production and
// deploy previews each serve their own assets. Override only for a media CDN.
export const MEDIA_BASE = process.env.NEXT_PUBLIC_MEDIA_BASE ?? "";

export function webappUrl(path: string): string {
  return `${WEBAPP_URL}${path.replace(/^\//, "")}`;
}

export function mediaUrl(path: string): string {
  return `${MEDIA_BASE}${path.startsWith("/") ? path : `/${path}`}`;
}

// Showcase clips are served straight from the R2 CDN, never proxied (proxying
// bills their bandwidth to Netlify). The CDN sends Access-Control-Allow-Origin,
// which the hero wall's WebGL video textures need.
const CDN_MEDIA_BASE = "https://cdn-w.storyteller.ai";

export function cdnMediaUrl(path: string): string {
  return `${CDN_MEDIA_BASE}${path.startsWith("/") ? path : `/${path}`}`;
}

export const SOCIAL_LINKS = {
  DISCORD: "https://discord.gg/artcraft",
  YOUTUBE: "https://www.youtube.com/@OfficialArtCraftStudios",
  TIKTOK: "https://www.tiktok.com/@artcraft.studios",
  GITHUB: "https://github.com/storytold/artcraft",
  INSTAGRAM: "https://www.instagram.com/get_artcraft",
  LINKEDIN: "https://www.linkedin.com/company/artcraft-ai",
  REDDIT: "https://www.reddit.com/r/ArtCraftAI/",
} as const;

export const SUPPORT_EMAIL = "hello@storyteller.ai";
