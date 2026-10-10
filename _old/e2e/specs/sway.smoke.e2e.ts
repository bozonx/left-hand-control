import assert from 'node:assert/strict'
import { openHome, waitForAttribute, waitForTestId } from '../helpers/app.js'
import { isSwayWaylandTarget } from '../helpers/targets.js'

describe('Sway Wayland smoke', () => {
  before(function () {
    if (!isSwayWaylandTarget()) {
      this.skip()
    }
  })

  it('detects the Sway Wayland session', async () => {
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
      (value) => value === 'Sway',
    )
    const session = await waitForAttribute(
      'platform-status-card',
      'data-platform-session',
      (value) => value === 'wayland',
    )

    assert.equal(os, 'linux')
    assert.equal(desktop, 'Sway')
    assert.equal(session, 'wayland')
  })
})
