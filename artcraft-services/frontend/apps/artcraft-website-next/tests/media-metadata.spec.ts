import { expect, test, type APIRequestContext } from "@playwright/test";
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { wav } from "./fixtures/audio";

const CDN = "https://media-fixture.invalid";
const SITE = "https://getartcraft.com";
const CRAWLERS = [
  ["Discord", "Mozilla/5.0 (compatible; Discordbot/2.0; +https://discordapp.com)"],
  ["Twitter", "Twitterbot/1.0"],
  ["Meta", "facebookexternalhit/1.1 (+http://www.facebook.com/externalhit_uatext.php)"],
  ["Slack", "Slackbot-LinkExpanding 1.0 (+https://api.slack.com/robots)"],
  ["unknown crawler", "ArtCraftPreviewTest/1.0"],
  ["curl", "curl/8.7.1"],
  ["browser", "Mozilla/5.0 Chrome/140.0.0.0 Safari/537.36"],
] as const;

// Raw HTTP only: no browser, hydration, JavaScript, or DOM repair can make
// these assertions pass. Tags must be inside the server response's <head>.
for (const [name, userAgent] of CRAWLERS) {
  test(`${name} gets complete media social tags in the raw HTML head`, async ({ request }) => {
    const head = await requestHead(request, "m_fixture_image", userAgent);
    expect(head).toContain("<title>Night sky &amp; stars — ArtCraft</title>");
    expect(head).toContain('property="og:title" content="Night sky &amp; stars — ArtCraft"');
    expect(head).toContain('property="og:description" content="Made with ArtCraft by Fixture Artist."');
    expect(head).toContain('property="og:type" content="website"');
    expect(head).toContain('property="og:site_name" content="ArtCraft"');
    expect(head).toContain(`property="og:url" content="${SITE}/media/m_fixture_image"`);
    expect(head).toContain(`property="og:image" content="${CDN}/asset.png?fixture=1"`);
    expect(head).toContain('property="og:image:alt" content="Night sky &amp; stars"');
    expect(head).toContain('name="twitter:card" content="summary_large_image"');
    expect(head).toContain('name="twitter:title" content="Night sky &amp; stars — ArtCraft"');
    expect(head).toContain('name="twitter:description" content="Made with ArtCraft by Fixture Artist."');
    expect(head).toContain(`name="twitter:image" content="${CDN}/asset.png?fixture=1"`);
    expect(head).toContain(`rel="canonical" href="${SITE}/media/m_fixture_image"`);
    expect(head).toContain('name="robots" content="noindex, follow"');
  });
}

test("video, mesh, splat, and unlisted cards use image previews", async ({ request }) => {
  for (const [token, preview] of [
    ["m_fixture_video", "still-1200.jpg"],
    ["m_fixture_mesh", "cover-1200.jpg"],
    ["m_fixture_splat", "splat-cover.jpg"],
    ["m_fixture_hidden", "unlisted-1200.png"],
  ]) {
    const head = await requestHead(request, token);
    expect(head).toContain(`property="og:image" content="${CDN}/${preview}"`);
    expect(head).toContain(`name="twitter:image" content="${CDN}/${preview}"`);
  }
});

test("audio without cover art uses the site image and its own title", async ({ request }) => {
  const head = await requestHead(request, "m_fixture_audio");
  expect(head).toContain('property="og:title" content="A song — ArtCraft"');
  expect(head).toContain(`property="og:image" content="${SITE}/images/og-image.png"`);
  expect(head).toContain(`name="twitter:image" content="${SITE}/images/og-image.png"`);
  expect(head).not.toContain(`property="og:image" content="${CDN}/asset.wav"`);
});

test("video and audio advertise direct media and iframe players", async ({ request }) => {
  for (const [, userAgent] of CRAWLERS) {
    const video = await requestHead(request, "m_fixture_video", userAgent);
    expect(video).toContain('property="og:type" content="video.other"');
    expect(video).toContain(`property="og:video" content="${CDN}/asset.mp4"`);
    expect(video).toContain(`property="og:video:secure_url" content="${CDN}/asset.mp4"`);
    expect(video).toContain('property="og:video:type" content="video/mp4"');
    expect(video).toContain('name="twitter:card" content="player"');
    expect(video).toContain(`name="twitter:player" content="${SITE}/media/m_fixture_video/player"`);
    expect(video).toContain(`name="twitter:player:stream" content="${CDN}/asset.mp4"`);
    expect(video).toContain('name="twitter:player:stream:content_type" content="video/mp4"');
    expect(video).toContain('name="twitter:player:width" content="640"');
    expect(video).toContain('name="twitter:player:height" content="360"');
    expect(video).toContain('type="application/json+oembed"');
    const audio = await requestHead(request, "m_fixture_audio", userAgent);
    expect(audio).toContain(`property="og:audio" content="${CDN}/asset.wav"`);
    expect(audio).toContain('property="og:audio:type" content="audio/wav"');
    expect(audio).toContain('name="twitter:card" content="player"');
    expect(audio).toContain(`name="twitter:player" content="${SITE}/media/m_fixture_audio/player"`);
    expect(audio).not.toContain('name="twitter:player:stream"');
    expect(audio).not.toContain('property="og:video"');
  }
});

test("oEmbed returns a sized iframe and rejects unrelated URLs and unsupported formats", async ({ request }) => {
  for (const [token, type] of [["m_fixture_video", "video"], ["m_fixture_audio", "rich"]]) {
    const response = await request.get(`/media/${token}/oembed`, { params: { url: `${SITE}/media/${token}`, format: "json", maxwidth: 320, maxheight: 90 } });
    expect(response.status()).toBe(200);
    const data = await response.json();
    expect(data).toMatchObject({ version: "1.0", type, provider_name: "ArtCraft", provider_url: SITE });
    expect(data.width).toBeLessThanOrEqual(320);
    expect(data.height).toBeLessThanOrEqual(90);
    expect(data.html).toContain(`src="${SITE}/media/${token}/player"`);
    expect(data.html).toContain(`width="${data.width}" height="${data.height}"`);
  }
  const unrelated = await request.get("/media/m_fixture_video/oembed", { params: { url: "https://unrelated.invalid/" } });
  expect(unrelated.status()).toBe(404);
  const xml = await request.get("/media/m_fixture_video/oembed?format=xml");
  expect(xml.status()).toBe(501);
});

test("private and non-playable media cannot be embedded", async ({ request }) => {
  for (const token of ["m_fixture_private", "m_missing", "m_fixture_image", "m_fixture_mesh"]) {
    for (const route of ["player", "oembed"]) {
      const response = await request.get(`/media/${token}/${route}`, { params: { url: `${SITE}/media/${token}` } });
      expect(response.status()).toBe(404);
      const body = await response.text();
      expect(body).not.toContain(CDN);
      expect(body).not.toContain("Secret creation");
    }
  }
});

test("standalone video and audio players load and play with JavaScript disabled", async ({ browser }) => {
  const context = await browser.newContext({ javaScriptEnabled: false });
  try {
    const page = await context.newPage();
    const png = readFileSync(join(__dirname, "../public/images/og-image.png"));
    await page.route(`${CDN}/**`, (route) => route.fulfill({ body: png, contentType: "image/png" }));
    await page.route(`${SITE}/images/og-image.png`, (route) => route.fulfill({ body: png, contentType: "image/png" }));
    await page.route(`${CDN}/asset.mp4`, (route) => route.fulfill({ body: readFileSync(join(__dirname, "fixtures/clip.mp4")), contentType: "video/mp4" }));
    await page.route(`${CDN}/asset.wav`, (route) => route.fulfill({ body: wav(), contentType: "audio/wav" }));
    for (const kind of ["video", "audio"]) {
      const response = await page.goto(`http://localhost:4202/media/m_fixture_${kind}/player`);
      expect(response?.headers()["content-security-policy"]).toContain("default-src 'none'");
      expect(await response!.text()).not.toContain("<script");
      const player = page.locator(kind);
      await expect(player).toHaveAttribute("controls", "");
      await expect.poll(() => player.evaluate((element: HTMLMediaElement) => element.readyState)).toBeGreaterThanOrEqual(1);
      // Invoke the native media API; the document itself contains no JavaScript.
      await player.evaluate((element: HTMLMediaElement) => { element.muted = true; return element.play(); });
      await expect.poll(() => player.evaluate((element: HTMLMediaElement) => element.currentTime)).toBeGreaterThan(0);
    }
  } finally {
    await context.close();
  }
});

test("private files, missing files, and API failures get a generic card", async ({ request }) => {
  for (const token of ["m_fixture_private", "m_missing", "m_fixture_unauthorized", "m_fixture_forbidden", "m_fixture_failure", "m_fixture_malformed", "m_fixture_timeout"]) {
    const head = await requestHead(request, token);
    expect(head).toContain('property="og:title" content="Shared media — ArtCraft"');
    expect(head).toContain(`property="og:url" content="${SITE}/media/${token}"`);
    expect(head).toContain(`property="og:image" content="${SITE}/images/og-image.png"`);
    expect(head).toContain(`name="twitter:image" content="${SITE}/images/og-image.png"`);
    expect(head).not.toContain(CDN);
    expect(head).not.toContain("Secret creation");
    expect(head).not.toContain("secret_artist");
  }
});

test("metadata is looked up again when a public creation changes", async ({ request }) => {
  const first = await requestHead(request, "m_fixture_changing");
  const second = await requestHead(request, "m_fixture_changing");
  const revision = (head: string) => Number(head.match(/property="og:title" content="Revision (\d+)/)?.[1]);
  expect(revision(first)).toBeGreaterThan(0);
  expect(revision(second)).toBeGreaterThan(revision(first));
});

async function requestHead(request: APIRequestContext, token: string, userAgent = "Discordbot/2.0") {
  const response = await request.get(`/media/${token}`, {
    // The fixture API rejects these headers if the website forwards them.
    headers: { "User-Agent": userAgent, Cookie: "session=private-fixture", session: "private-fixture" },
  });
  expect(response.status()).toBe(200);
  expect(response.headers()["content-type"]).toContain("text/html");
  const html = await response.text();
  const head = html.match(/<head\b[^>]*>([\s\S]*?)<\/head>/i)?.[1];
  expect(head).toBeDefined();
  return head!;
}
