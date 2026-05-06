import { defineVitestConfig } from '@nuxt/test-utils/config'

export default defineVitestConfig({
  test: {
    name: 'components',
    environment: 'nuxt',
    hookTimeout: 30000,
    include: ['tests/components/**/*.test.ts'],
    setupFiles: ['tests/setup/components.ts'],
    reporters: process.env.CI ? ['default', 'junit'] : ['default'],
    outputFile: process.env.CI ? { junit: './test-results/components/junit.xml' } : undefined,
    coverage: {
      provider: 'v8',
      include: ['components/**', 'composables/**', 'pages/**'],
      exclude: ['**/*.test.ts', 'tests/**'],
      reporter: process.env.CI ? ['text', 'lcov'] : ['text'],
      reportsDirectory: './coverage/components',
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
