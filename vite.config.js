import { defineConfig } from 'vite';
import { svelte } from '@sveltejs/vite-plugin-svelte';

export default defineConfig({
  plugins: [svelte()],
  clearScreen: false,
  server: { host: '127.0.0.1', port: 1420, strictPort: true, watch: { ignored: ['**/src-tauri/**'] } },
  build: {
    target: 'chrome120',
    sourcemap: false,
    // two pages: the app itself and the startup cube's own little window
    rollupOptions: { input: { main: 'index.html', splash: 'splash.html' } },
  },
});
