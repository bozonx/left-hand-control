// https://nuxt.com/docs/api/configuration/nuxt-config
export default defineNuxtConfig({
  compatibilityDate: '2025-01-01',
  devtools: { enabled: process.env.NODE_ENV === 'development' },

  // Tauri is a desktop runtime, no need for SSR
  ssr: false,

  modules: ['@nuxt/ui'],

  // Auto-import `useI18n` from vue-i18n so components / composables can
  // call it without a manual `import` line. The plugin in `plugins/i18n.ts`
  // is what actually registers the vue-i18n instance with the Vue app.
  imports: {
    presets: [
      {
        from: 'vue-i18n',
        imports: ['useI18n'],
      },
    ],
  },

  css: ['~/assets/css/main.css'],

  // Nuxt UI v3 bundles @nuxtjs/color-mode — configure it explicitly so the
  // default follows the OS and the class flips the Tailwind `dark:` variant.
  // `preference` is authoritative; our `useAppTheme()` composable overwrites
  // it from the persisted `settings.appearance` on boot.
  colorMode: {
    preference: 'system',
    fallback: 'light',
    classSuffix: '',
    storageKey: 'lhc-color-mode',
  },

  // Ensure static output for Tauri bundling
  nitro: {
    preset: 'static',
  },

  experimental: {
    appManifest: false,
  },

  // Nuxt dev server configuration for Tauri
  devServer: {
    host: 'localhost',
    port: 3000,
  },

  vite: {
    // Prevent Vite from obscuring Rust errors
    clearScreen: false,
    // Tauri expects a fixed port, fail if that port is not available
    server: {
      strictPort: true,
      hmr: {
        protocol: 'ws',
        host: 'localhost',
        port: 3001,
      },
    },
    // Keep the client env surface narrow: only explicit frontend-facing
    // variables should reach `import.meta.env`.
    envPrefix: ['VITE_'],
  },
})
