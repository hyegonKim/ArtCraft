import { createServer } from "node:http";

// Next's server-side metadata requests cannot be intercepted by page.route.
// This local-only fixture has no upstream and never connects to a database.
const CDN = "https://media-fixture.invalid";
const publicMedia = {
  m_fixture_image: {
    token: "m_fixture_image", media_class: "image",
    creator_set_visibility: "public", maybe_title: "Night sky & stars",
    maybe_creator_user: { username: "fixture_artist", display_name: "Fixture Artist" },
    media_links: { cdn_url: `${CDN}/asset.png?fixture=1` },
  },
  m_fixture_video: {
    token: "m_fixture_video", media_class: "video",
    creator_set_visibility: "public", maybe_title: "A short film",
    media_links: { cdn_url: `${CDN}/asset.mp4`, maybe_video_previews: {
      still: `${CDN}/still.jpg`, still_thumbnail_template: `${CDN}/still-{WIDTH}.jpg`,
    } },
  },
  m_fixture_audio: {
    token: "m_fixture_audio", media_class: "audio",
    creator_set_visibility: "public", maybe_title: "A song",
    media_links: { cdn_url: `${CDN}/asset.wav` },
  },
  m_fixture_mesh: {
    token: "m_fixture_mesh", media_class: "mesh", creator_set_visibility: "public",
    media_links: { cdn_url: `${CDN}/asset.glb` },
    cover_image: { maybe_links: { cdn_url: `${CDN}/cover.jpg`, thumbnail_template: `${CDN}/cover-{WIDTH}.jpg` } },
  },
  m_fixture_splat: {
    token: "m_fixture_splat", media_class: "splat", creator_set_visibility: "public",
    media_links: { cdn_url: `${CDN}/asset.spz` },
    cover_image: { maybe_links: { cdn_url: `${CDN}/splat-cover.jpg` } },
  },
  m_fixture_private: {
    token: "m_fixture_private", media_class: "image", creator_set_visibility: "private",
    maybe_title: "Secret creation", maybe_creator_user: { username: "secret_artist" },
    media_links: { cdn_url: `${CDN}/secret-image.png` },
  },
  m_fixture_hidden: {
    token: "m_fixture_hidden", media_class: "image", creator_set_visibility: "hidden",
    media_links: { cdn_url: `${CDN}/unlisted.png`, maybe_thumbnail_template: `${CDN}/unlisted-{WIDTH}.png` },
  },
};
let revision = 0;

createServer((request, response) => {
  response.setHeader("Content-Type", "application/json");
  if (request.url === "/health") {
    response.end(JSON.stringify({ success: true }));
    return;
  }
  // A regression forwarding visitor credentials must not produce public cards.
  if (request.headers.cookie || request.headers.authorization || request.headers.session) {
    response.writeHead(403).end(JSON.stringify({ success: false }));
    return;
  }
  const token = request.url?.match(/^\/v1\/media_files\/file\/([^/?]+)$/)?.[1];
  if (token === "m_fixture_changing") {
    response.end(JSON.stringify({ success: true, media_file: {
      ...publicMedia.m_fixture_image, token, maybe_title: `Revision ${++revision}`,
    } }));
    return;
  }
  if (token === "m_fixture_malformed") {
    response.end("not json");
    return;
  }
  if (token === "m_fixture_timeout") {
    setTimeout(() => response.end(JSON.stringify({ success: false })), 6000);
    return;
  }
  const errorStatus = { m_fixture_unauthorized: 401, m_fixture_forbidden: 403, m_fixture_failure: 500 }[token];
  if (errorStatus) {
    response.writeHead(errorStatus).end(JSON.stringify({ success: false }));
    return;
  }
  const media = publicMedia[token];
  response.writeHead(media ? 200 : 404).end(JSON.stringify(media
    ? { success: true, media_file: media }
    : { success: false }));
}).listen(4203, "127.0.0.1");
