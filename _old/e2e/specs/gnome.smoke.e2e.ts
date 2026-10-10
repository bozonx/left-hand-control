import assert from 'node:assert/strict'
import { openHome, waitForAttribute, waitForTestId } from '../helpers/app.js'
import { isGnomeWaylandTarget } from '../helpers/targets.js'

describe('GNOME Wayland smoke', () => {
  before(function () {
    if (!isGnomeWaylandTarget()) {
      this.skip()
    }
  })

  it('detects the GNOME Wayland session', async () => {
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
      (value) => value === 'GNOME',
    )
    const session = await waitForAttribute(
      'platform-status-card',
      'data-platform-session',
      (value) => value === 'wayland',
    )

    assert.equal(os, 'linux')
    assert.equal(desktop, 'GNOME')
    assert.equal(session, 'wayland')
  })
})
