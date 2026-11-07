import { readFileSync } from 'node:fs';
import { createRequire } from 'node:module';
import type { Plugin } from 'vite';

const require = createRequire(import.meta.url);
const files = ['ammo.wasm.js', 'ammo.wasm.wasm'];

// Ammo's classic script locates its WASM sibling relative to its own URL.
// Serve and emit the installed Three.js pair together, without a remote CDN.
export function ammoAssetsPlugin(): Plugin {
  const assets = files.map((name) => ({
    path: `/jsm/libs/${name}`,
    source: readFileSync(require.resolve(`three/examples/jsm/libs/${name}`)),
    type: name.endsWith('.wasm') ? 'application/wasm' : 'application/javascript',
  }));
  return {
    name: 'artcraft-ammo-assets',
    configureServer(server) {
      server.middlewares.use((req, res, next) => {
        const asset = assets.find((item) => item.path === req.url?.split('?')[0]);
        if (!asset) return next();
        res.setHeader('Content-Type', asset.type);
        res.end(asset.source);
      });
    },
    generateBundle() {
      for (const asset of assets) {
        this.emitFile({ type: 'asset', fileName: asset.path.slice(1), source: asset.source });
      }
    },
  };
}
