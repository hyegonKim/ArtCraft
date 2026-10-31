import { API_HOST } from "./api";
import { mediaFilePath, safeMediaUrl, type SharedMedia } from "./media";

// Social cards use only the public API response. Never forward a visitor's
// cookies or signed session into server metadata; private media stays client-side.
export async function fetchPublicMediaFile(token: string): Promise<SharedMedia | null> {
  try {
    const response = await fetch(`${API_HOST}${mediaFilePath(token)}`, {
      headers: { Accept: "application/json" },
      credentials: "omit",
      cache: "no-store",
      signal: AbortSignal.timeout(5000),
    });
    if (!response.ok) return null;
    const payload = await response.json() as { success?: boolean; media_file?: SharedMedia };
    const media = payload.media_file;
    if (!payload.success || media?.token !== token || !safeMediaUrl(media.media_links?.cdn_url)) return null;
    // Hidden means unlisted but shareable by URL. Private files must never
    // appear in social cards, even if an API response includes their fields.
    return media.creator_set_visibility === "public" || media.creator_set_visibility === "hidden" ? media : null;
  } catch {
    return null;
  }
}
