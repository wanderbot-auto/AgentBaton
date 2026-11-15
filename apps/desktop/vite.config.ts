import { defineConfig } from 'vitest/config';
import react from '@vitejs/plugin-react';

export default defineConfig({
  plugins: [react()],
  clearScreen: false,
  server: { port: 1420, strictPort: true, host: '127.0.0.1' },
  // Keep all cases and their original deadlines while bounding concurrent
  // jsdom workers on the shared CI hosts. This is not a product runtime limit.
  test: { environment: 'jsdom', setupFiles: ['./src/test-setup.ts'], maxWorkers: 2 },
});
