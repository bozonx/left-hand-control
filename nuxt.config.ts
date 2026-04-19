// https://nuxt.com/docs/api/configuration/nuxt-config
export default defineNuxtConfig({
  compatibilityDate: '2025-01-01',
  devtools: { enabled: true },

  // Tauri is a desktop runtime, no need for SSR
  ssr: false,

  modules: ['@nuxt/ui'],

  css: ['~/assets/css/main.css'],

  // Ensure static output for Tauri bundling
  nitro: {
    preset: 'static',
  },

  // Nuxt dev server configuration for Tauri
  devServer: {
    host: '127.0.0.1',
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
        host: '127.0.0.1',
        port: 3001,
      },
    },
    // Env variables starting with the item of `envPrefix` will be exposed in tauri's source code through `import.meta.env`
    envPrefix: ['VITE_', 'TAURI_'],
  },
})
