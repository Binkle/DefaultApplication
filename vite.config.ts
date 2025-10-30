import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';

export default defineConfig({
  plugins: [react()],
  server: {
    host: process.env.TAURI_DEV_HOST || 'localhost',
    port: process.env.TAURI_DEV_PORT ? Number(process.env.TAURI_DEV_PORT) : 1420,
    strictPort: true,
    hmr: process.env.TAURI_DEV_HOST
      ? {
          host: process.env.TAURI_DEV_HOST,
          port: process.env.TAURI_DEV_PORT ? Number(process.env.TAURI_DEV_PORT) : 1420,
        }
      : undefined,
  },
  build: {
    target: ['es2021', 'chrome105', 'safari13'],
    outDir: 'dist',
  },
});
