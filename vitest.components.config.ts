import { defineVitestConfig } from '@nuxt/test-utils/config'

export default defineVitestConfig({
  test: {
    name: 'components',
    environment: 'nuxt',
    include: ['tests/components/**/*.test.ts'],
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
