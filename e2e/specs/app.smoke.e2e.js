import assert from 'node:assert/strict'
import {
  assertContainsPath,
  completeWelcomeIfPresent,
  openHome,
  openSettings,
  waitForTestId,
} from '../helpers/app.js'

describe('desktop app smoke', () => {
  it('starts the Tauri shell and renders the layouts page', async () => {
    await waitForTestId('app-shell', 60000)
    await completeWelcomeIfPresent()
    await openHome()
    await waitForTestId('layouts-page')
    await waitForTestId('layouts-library-card')
  })

  it('opens settings and uses the isolated E2E storage directory', async () => {
    await openSettings()
    await waitForTestId('mapper-card')
    await waitForTestId('config-path-card')

    const settingsDir = await (await waitForTestId('settings-dir-path')).getText()
    const layoutsDir = await (await waitForTestId('layouts-dir-path')).getText()

    assert.notEqual(settingsDir, '…')
    assert.notEqual(layoutsDir, '…')

    if (process.env.LHC_DEV_DIR) {
      assertContainsPath(settingsDir, `${process.env.LHC_DEV_DIR}/config`)
      assertContainsPath(layoutsDir, `${process.env.LHC_DEV_DIR}/data/layouts`)
    }
  })
})
