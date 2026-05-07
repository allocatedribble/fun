import { svelte } from '@sveltejs/vite-plugin-svelte';
import { defineConfig } from 'vite';

declare const process: { env: Record<string, string | undefined> };

const rvelteDevIslandEnabled = process.env.VITE_FUN_RVELTE_DEV === '1';

export default defineConfig({
  plugins: [svelte()],
  publicDir: rvelteDevIslandEnabled ? 'public' : false,
  clearScreen: false,
  server: {
    host: '127.0.0.1',
    port: 1420,
    strictPort: true
  },
  build: {
    outDir: 'dist',
    emptyOutDir: true,
    manifest: true,
    assetsDir: 'assets',
    target: 'es2022'
  }
});
