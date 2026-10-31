import { fetchPublicMediaFile } from "@/lib/fetch-public-media";
import { mediaTitle } from "@/lib/media";
import { mediaEmbed, mediaShareImage, MEDIA_SHARE_ORIGIN } from "@/lib/media-sharing";

const HEADERS = {
  "Content-Type": "text/html; charset=utf-8",
  "Cache-Control": "private, no-store",
  "X-Content-Type-Options": "nosniff",
  "X-Robots-Tag": "noindex, nofollow",
  "Referrer-Policy": "no-referrer",
  "Content-Security-Policy": "default-src 'none'; style-src 'unsafe-inline'; img-src https:; media-src https:; base-uri 'none'; form-action 'none'",
};

// A standalone HTML document bypasses the site layout and needs no scripts.
// Platforms that support iframe Player Cards can use native media controls.
export async function GET(_request: Request, { params }: { params: Promise<{ token: string }> }) {
  const { token } = await params;
  const media = await fetchPublicMediaFile(token);
  const embed = media && mediaEmbed(media);
  if (!media || !embed) return new Response("Media unavailable", { status: 404, headers: HEADERS });
  const title = escapeHtml(mediaTitle(media));
  const poster = escapeHtml(mediaShareImage(media) ?? `${MEDIA_SHARE_ORIGIN}/images/og-image.png`);
  const source = `<source src="${escapeHtml(embed.url)}" type="${embed.contentType}">`;
  const player = embed.kind === "video"
    ? `<video controls playsinline preload="metadata" poster="${poster}" aria-label="${title}">${source}</video>`
    : `<div class="audio"><img src="${poster}" alt=""><div><p>${title}</p><audio controls preload="metadata" aria-label="${title}">${source}</audio></div></div>`;
  return new Response(`<!doctype html><html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>${title} — ArtCraft</title><style>
    :root{color-scheme:light dark}*{box-sizing:border-box}html,body{margin:0;width:100%;height:100%;font:14px system-ui;background:light-dark(#f2f1ee,#121316);color:light-dark(#222,#eee)}
    video{display:block;width:100%;height:100%;object-fit:contain;background:#000}.audio{display:flex;align-items:center;gap:16px;padding:16px;height:100%}.audio img{width:80px;height:80px;object-fit:cover}.audio div{min-width:0;flex:1}p{margin:0 0 12px;overflow:hidden;text-overflow:ellipsis;white-space:nowrap}audio{width:100%}@media(max-width:360px){.audio img{display:none}}
    </style></head><body>${player}</body></html>`, { headers: HEADERS });
}

function escapeHtml(value: string): string {
  return value.replaceAll("&", "&amp;").replaceAll('"', "&quot;").replaceAll("'", "&#39;").replaceAll("<", "&lt;").replaceAll(">", "&gt;");
}
