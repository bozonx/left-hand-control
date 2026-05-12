import path from 'node:path'
import { fileURLToPath } from 'node:url'

const root = path.resolve(fileURLToPath(new URL('..', import.meta.url)))
const appName = process.platform === 'win32' ? 'left-hand-control.exe' : 'left-hand-control'
const appPath =
  process.env.LHC_E2E_APP ||
  path.join(root, 'src-tauri', 'target', 'debug', appName)

export const config = {
  runner: 'local',
  specs: ['./e2e/specs/**/*.e2e.js'],
  maxInstances: 1,
  hostname: process.env.LHC_TAURI_DRIVER_HOST || '127.0.0.1',
  port: Number.parseInt(process.env.LHC_TAURI_DRIVER_PORT || '4444', 10),
  path: '/',
  logLevel: process.env.LHC_E2E_WDIO_LOG_LEVEL || 'warn',
  bail: 0,
  waitforTimeout: 10000,
  connectionRetryTimeout: 120000,
  connectionRetryCount: 3,
  framework: 'mocha',
  reporters: ['spec'],
  mochaOpts: {
    ui: 'bdd',
    timeout: 60000,
  },
  capabilities: [
    {
      browserName: 'wry',
      'tauri:options': {
        application: appPath,
      },
    },
  ],
}
