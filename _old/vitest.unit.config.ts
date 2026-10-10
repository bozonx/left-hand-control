import { fileURLToPath } from 'node:url'

import { defineConfig } from 'vitest/config'

const rootDir = fileURLToPath(new URL('.', import.meta.url))

export default defineConfig({
  resolve: {
    alias: {
      '~': rootDir,
      '@': rootDir,
    },
  },
  test: {
    name: 'unit',
    environment: 'node',
    include: ['tests/unit/**/*.test.ts'],
    setupFiles: ['tests/setup/unit.ts'],
    reporters: process.env.CI ? ['default', 'junit'] : ['default'],
    outputFile: process.env.CI ? { junit: './test-results/unit/junit.xml' } : undefined,
    coverage: {
      provider: 'v8',
      include: ['utils/**', 'composables/**'],
      exclude: ['**/*.test.ts', 'tests/**'],
      reporter: process.env.CI ? ['text', 'lcov'] : ['text'],
      reportsDirectory: './coverage/unit',
    },
  },
})
