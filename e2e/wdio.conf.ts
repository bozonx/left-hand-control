import path from 'node:path'
import { fileURLToPath } from 'node:url'

const root = path.resolve(fileURLToPath(new URL('..', import.meta.url)))
const appName = process.platform === 'win32' ? 'left-hand-control.exe' : 'left-hand-control'
const appPath =
  process.env.LHC_E2E_APP ||
  path.join(root, 'src-tauri', 'target', 'debug', appName)

const screenshotPath = path.join(root, 'e2e', 'screenshots')

export const config: WebdriverIO.Config = {
  runner: 'local',
  specs: [path.join(root, 'e2e', 'specs', '**', '*.e2e.ts')],
  maxInstances: 1,
  hostname: process.env.LHC_TAURI_DRIVER_HOST || '127.0.0.1',
  port: Number.parseInt(process.env.LHC_TAURI_DRIVER_PORT || '4444', 10),
  path: '/',
  logLevel: (process.env.LHC_E2E_WDIO_LOG_LEVEL || 'warn') as any,
  bail: 0,
  waitforTimeout: 10000,
  connectionRetryTimeout: 120000,
  connectionRetryCount: 3,
  specFileRetries: 2,
  specFileRetriesDelay: 3,
  specFileRetriesDeferred: false,
  framework: 'mocha',
  reporters: ['spec'],
  mochaOpts: {
    ui: 'bdd',
    timeout: 60000,
  },
  capabilities: [
    {
      'tauri:options': {
        application: appPath,
      },
    } as any,
  ],
  before() {
    // Intentionally empty; reserved for pre-test app reset when needed.
  },
  afterTest: async function (test, _context, result) {
    if (!result.passed) {
      const fs = await import('node:fs')
      if (!fs.existsSync(screenshotPath)) {
        fs.mkdirSync(screenshotPath, { recursive: true })
      }
      const fileName = `${test.parent} - ${test.title} - ${Date.now()}.png`
        .replace(/[^a-zA-Z0-9.\-_]/g, '_')
      const filePath = path.join(screenshotPath, fileName)
      await browser.saveScreenshot(filePath)
    }
  },
}
