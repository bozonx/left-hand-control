import { defineVitestConfig } from '@nuxt/test-utils/config'

export default defineVitestConfig({
  test: {
    name: 'components',
    environment: 'nuxt',
    include: ['tests/components/**/*.test.ts'],
    setupFiles: ['tests/setup/components.ts'],
    reporters: process.env.CI ? ['default', 'junit'] : ['default'],
    outputFile: process.env.CI ? { junit: './test-results/components/junit.xml' } : undefined,
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
