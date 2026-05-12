import assert from 'node:assert/strict'
import { openHome, waitForAttribute, waitForTestId } from '../helpers/app.js'
import { expectedOsForTarget, isWindowsTarget } from '../helpers/targets.js'

describe('platform smoke', () => {
  it('loads platform diagnostics through Tauri IPC', async () => {
    await openHome()
    await waitForTestId('platform-status-card')

    const os = await waitForAttribute(
      'platform-status-card',
      'data-platform-os',
      (value) => value.length > 0,
    )

    const expectedOs = expectedOsForTarget()
    if (expectedOs) {
      assert.equal(os, expectedOs)
    }

    if (isWindowsTarget()) {
      const desktop = await (
        await waitForTestId('platform-status-card')
      ).getAttribute('data-platform-desktop')
      assert.equal(desktop, '')
    }
  })
})
