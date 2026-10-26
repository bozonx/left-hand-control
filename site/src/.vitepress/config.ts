import path from 'node:path'
import { fileURLToPath } from 'node:url'
import { defineLandingConfig } from 'vitepress-theme-neptu-landing/configs'
import type { LandingUserConfig } from 'vitepress-theme-neptu-landing'

const __dirname = path.dirname(fileURLToPath(import.meta.url))

export default async () => {
  const base = process.env.VITEPRESS_BASE || '/'
  const assetUrl = (path: string) => `${base}${path.replace(/^\//, '')}`

  const config: LandingUserConfig = {
    srcDir: path.resolve(__dirname, '../'),
    base,
    siteUrl:
      process.env.SITE_URL ||
      'https://bozonx.github.io/left-hand-control',

    head: [
      ['meta', { name: 'format-detection', content: 'telephone=no' }],
      ['link', { rel: 'icon', type: 'image/svg+xml', href: assetUrl('/favicon.svg') }],
      ['link', { rel: 'icon', type: 'image/png', sizes: '32x32', href: assetUrl('/img/favicon-32x32.png') }],
      ['link', { rel: 'icon', type: 'image/png', sizes: '16x16', href: assetUrl('/img/favicon-16x16.png') }],
      ['link', { rel: 'apple-touch-icon', sizes: '180x180', href: assetUrl('/img/apple-touch-icon.png') }],
      ['link', { rel: 'shortcut icon', href: assetUrl('/favicon.ico') }],
      ['link', { rel: 'manifest', href: assetUrl('/site.webmanifest') }],
    ],

    themeConfig: {
      repo: 'https://github.com/bozonx/left-hand-control',
      search: { provider: 'local' },
    },
  }

  return defineLandingConfig(config)
}
