import { defineVitestConfig } from '@nuxt/test-utils/config'

export default defineVitestConfig({
  test: {
    name: 'composables',
    environment: 'nuxt',
    hookTimeout: 30000,
    include: ['tests/composables/**/*.test.ts'],
    setupFiles: ['tests/setup/components.ts'],
    reporters: process.env.CI ? ['default', 'junit'] : ['default'],
    outputFile: process.env.CI ? { junit: './test-results/composables/junit.xml' } : undefined,
    coverage: {
      provider: 'v8',
      include: ['composables/**'],
      exclude: ['**/*.test.ts', 'tests/**'],
      reporter: process.env.CI ? ['text', 'lcov'] : ['text'],
      reportsDirectory: './coverage/composables',
    },
    environmentOptions: {
      nuxt: {
        domEnvironment: 'happy-dom',
        mock: {
          indexedDb: true,
          intersectionObserver: true,
        },
      },
    },
  },
})
