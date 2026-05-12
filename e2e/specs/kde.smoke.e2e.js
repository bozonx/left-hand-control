import assert from 'node:assert/strict'
import { openHome, waitForAttribute, waitForTestId } from '../helpers/app.js'

describe('KDE Plasma Wayland smoke', () => {
  before(function () {
    if (process.env.LHC_E2E_TARGET !== 'kde-wayland') {
      this.skip()
    }
  })

  it('detects the KDE Wayland session and required KDE services', async () => {
    await openHome()
    await waitForTestId('platform-status-card')

    const os = await waitForAttribute(
      'platform-status-card',
      'data-platform-os',
      (value) => value === 'linux',
    )
    const desktop = await waitForAttribute(
      'platform-status-card',
      'data-platform-desktop',
      (value) => value === 'KDE Plasma',
    )
    const session = await waitForAttribute(
      'platform-status-card',
      'data-platform-session',
      (value) => value === 'wayland',
    )
    const layoutDetection = await waitForAttribute(
      'platform-status-card',
      'data-layout-detection-available',
      (value) => value === 'true',
    )
    const systemActions = await waitForAttribute(
      'platform-status-card',
      'data-system-actions-available',
      (value) => value === 'true',
    )

    assert.equal(os, 'linux')
    assert.equal(desktop, 'KDE Plasma')
    assert.equal(session, 'wayland')
    assert.equal(layoutDetection, 'true')
    assert.equal(systemActions, 'true')
  })
})
