import { fetchPublicMediaFile } from "@/lib/fetch-public-media";
import { mediaTitle } from "@/lib/media";
import { mediaEmbed, MEDIA_SHARE_ORIGIN } from "@/lib/media-sharing";

const HEADERS = { "Cache-Control": "private, no-store", "X-Robots-Tag": "noindex, nofollow" };

export async function GET(request: Request, { params }: { params: Promise<{ token: string }> }) {
  const { token } = await params;
  const query = new URL(request.url).searchParams;
  if (query.has("format") && query.get("format") !== "json") {
    return Response.json({ error: "Only JSON is supported" }, { status: 501, headers: HEADERS });
  }
  if (query.get("url") !== `${MEDIA_SHARE_ORIGIN}/media/${encodeURIComponent(token)}`) {
    return Response.json({ error: "Media unavailable" }, { status: 404, headers: HEADERS });
  }
  const media = await fetchPublicMediaFile(token);
  const embed = media && mediaEmbed(media);
  if (!media || !embed) return Response.json({ error: "Media unavailable" }, { status: 404, headers: HEADERS });
  const playerUrl = `${MEDIA_SHARE_ORIGIN}/media/${encodeURIComponent(token)}/player`;
  const scale = Math.min(1, maximum(query.get("maxwidth")) / embed.width, maximum(query.get("maxheight")) / embed.height);
  const width = Math.max(1, Math.floor(embed.width * scale));
  const height = Math.max(1, Math.floor(embed.height * scale));
  return Response.json({
    version: "1.0",
    type: embed.kind === "video" ? "video" : "rich",
    provider_name: "ArtCraft",
    provider_url: MEDIA_SHARE_ORIGIN,
    title: mediaTitle(media),
    author_name: media.maybe_creator_user?.display_name || media.maybe_creator_user?.username,
    width,
    height,
    html: `<iframe src="${playerUrl}" width="${width}" height="${height}" title="ArtCraft media player" frameborder="0" allow="fullscreen" allowfullscreen></iframe>`,
  }, { headers: HEADERS });
}

function maximum(value: string | null): number {
  const parsed = Number(value);
  return Number.isSafeInteger(parsed) && parsed > 0 ? parsed : Infinity;
}
