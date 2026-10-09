// https://nuxt.com/docs/api/configuration/nuxt-config
export default defineNuxtConfig({
  compatibilityDate: '2025-01-01',
  devtools: { enabled: process.env.NODE_ENV === 'development' },

  // Tauri is a desktop runtime, no need for SSR
  ssr: false,

  modules: ['@nuxt/ui'],
  icon: {
    clientBundle: {
      scan: true,
    },
  },

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

  ui: { colorMode: false },

  // Ensure static output for Tauri bundling
  nitro: {
    preset: 'static',
  },

  experimental: {
    appManifest: false,
  },

  // Nuxt dev server configuration for Tauri. The dev script checks the port
  // before launching Nuxt so it cannot silently drift away from Tauri's devUrl.
  devServer: {
    host: 'localhost',
    port: Number.parseInt(process.env.LHC_DEV_PORT || '3010', 10),
  },

  vite: {
    // Prevent Vite from obscuring Rust errors
    clearScreen: false,
    // Tauri expects a fixed port, fail if that port is not available
    server: {
      strictPort: true,
    },
    // Keep the client env surface narrow: only explicit frontend-facing
    // variables should reach `import.meta.env`.
    envPrefix: ['VITE_'],
  },
})
