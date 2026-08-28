import { defineConfig } from 'vitest/config'
import { svelte } from '@sveltejs/vite-plugin-svelte'
import { fileURLToPath } from 'node:url'

export default defineConfig({
  plugins: [svelte()],
  resolve: {
    conditions: ['browser'],
    alias: {
      'lucide-svelte': fileURLToPath(new URL('./src/test/lucide-svelte.ts', import.meta.url)),
    },
  },
  test: {
    environment: 'jsdom',
    setupFiles: ['./src/test-setup.ts'],
  },
})
