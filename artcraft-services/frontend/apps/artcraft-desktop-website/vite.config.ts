import { defineConfig } from 'vite';

// Static landing page for desktop.getartcraft.com. No framework: index.html
// plus one stylesheet, so Vite is only here for dev serving and hashed
// production assets.
export default defineConfig(() => ({
  root: __dirname,
  cacheDir: '../../node_modules/.vite/apps/artcraft-desktop-website',
  server: {
    port: 4260,
    host: 'localhost',
  },
  preview: {
    port: 4360,
    host: 'localhost',
  },
  build: {
    outDir: './dist',
    emptyOutDir: true,
    reportCompressedSize: true,
  },
}));
