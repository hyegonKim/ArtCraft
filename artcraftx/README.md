<h1 align="center">ArtCraft-X</h1>

<p align="center">
  <img src="docs/images/artcraftx-screenshot.png" width="1100" alt="ArtCraft-X's image generation workspace, with model selection, reference images, and a prompt editor.">
</p>

<p align="center">A minimal desktop app for AI creation.</p>

<p align="center">
  Images · Video · Audio · 3D Meshes · Worlds
</p>

<p align="center">
  <a href="https://getartcraft.com/">ArtCraft Website</a> ·
  <a href="https://github.com/storytold/artcraft">ArtCraft Repository</a>
</p>

<p align="center">
  <a href="https://discord.gg/artcraft"><img alt="Discord members online" src="https://img.shields.io/discord/1359579021108842617?style=for-the-badge&amp;label=Discord&amp;color=5865F2&amp;logo=discord&amp;logoColor=white"></a>
  <a href="https://www.youtube.com/@OfficialArtCraftStudios"><img alt="YouTube" src="https://img.shields.io/badge/YouTube-FF0000?style=for-the-badge&amp;logo=youtube&amp;logoColor=white"></a>
  <a href="https://x.com/get_artcraft"><img alt="X" src="https://img.shields.io/badge/X-181717?style=for-the-badge&amp;logo=x&amp;logoColor=white"></a>
  <a href="https://www.linkedin.com/company/artcraft-ai"><img alt="LinkedIn" src="https://img.shields.io/badge/LinkedIn-0A66C2?style=for-the-badge"></a>
</p>

<p align="center">
  <strong>Early Experimental Release</strong><br>
  Builds are Forthcoming
</p>

## Development (macOS / Linux)

With Node.js 20+, npm, Rust, and the Tauri 2 CLI installed, run:

```bash
./script/unix_dev.sh
```

This starts the frontend and desktop app together. It installs frontend
dependencies when missing, then binds the first available loopback port starting
at **5183** before launching Tauri with that exact URL. Existing processes are
left running. To start the port search elsewhere:

```bash
ARTCRAFTX_DEV_PORT=5200 ./script/unix_dev.sh
```

JavaScript, TypeScript, and CSS edits reload through Vite, including shared
frontend libraries. Rust edits in the app or its workspace dependencies trigger
a rebuild and desktop app restart; in-memory app state resets on a Rust restart.
The Rust backend uses native Tauri IPC, so there is no second backend HTTP port
to allocate. HTTP and the HMR WebSocket share the selected frontend port.

Press **Ctrl-C** to stop the frontend, Rust watcher, and app. Use this combined
launcher instead of running the two `script/artcraftx/unix_*_dev.sh` scripts.
The launcher resolves paths relative to itself and can be called from any directory.
