# Media fixtures

`clip.mp4` is a generated one-second solid-blue H.264 video with no audio:

```sh
ffmpeg -f lavfi -i color=c=0x3366cc:s=64x64:d=1 -c:v libx264 -pix_fmt yuv420p -movflags +faststart clip.mp4
```

The tests construct their tiny PNG, sine-wave WAV, triangle GLB, and 27-point
Gaussian splat fixtures locally. All Storyteller API and media CDN requests are
intercepted in the browser; server-side social-card lookups use `api-server.mjs`
on `127.0.0.1:4203`, which has no upstream. No production API or database is used.
Run `npm run test:media` with Google Chrome installed and ports 4202 and 4203 free.
The suite starts its own Next server with the fixture API host, so it refuses to
reuse a running development server. Playwright uses installed Chrome.

`media-metadata.spec.ts` inspects raw HTTP response heads for Discord, Twitter,
Meta, Slack, an unknown crawler, curl, and a browser user agent. It executes no
page JavaScript. The fixtures cover image titles, video stills, 3D cover images,
unlisted links, audio without artwork, private media, API errors/timeouts, and
updates to an existing creation. The fixture API rejects forwarded session
credentials. Keep these tests independent of the browser's intercepted requests.

The route's `generateMetadata` performs the anonymous server-side lookup.
`htmlLimitedBots: /.*/` keeps metadata in the initial `<head>` for every user
agent. Keep the five-second API timeout and generic fallback so an API outage
does not leave crawlers waiting indefinitely. `cache: "no-store"` avoids stale
server metadata; social platforms can still cache cards independently.

Public audio/video links also advertise a standalone `/media/<token>/player`
document and `/media/<token>/oembed` discovery endpoint. The player uses native
HTML controls without scripts or the marketing layout. Tests play its fixture
video and audio in Chrome with page JavaScript disabled, check oEmbed sizing,
and verify that private media cannot be embedded. Twitter's optional native
stream URL is emitted only for MP4; audio and WebM use the iframe player.

These tests verify our responses and native player, not another platform's
acceptance of an embed. Validate a deployed URL in each platform. Discord's
audio-tag support is still an open request:
https://github.com/discord/discord-api-docs/discussions/3262
Discord-playable audio needs a generated video derivative (for example, H.264
cover art plus AAC audio in an MP4). Merely declaring an MP3 to be a video does
not create that derivative. Instagram feed/Reel publishing is a separate media
upload workflow; website metadata does not publish an Instagram post.
