import { describe, expect, it } from 'vitest'

import { createDefaultConfig } from '~/types/config'
import {
  commandFingerprint,
  commandTrustKey,
  commandsTrusted,
} from '~/utils/commandTrust'

describe('command trust', () => {
  it('fingerprints shell scripts deterministically', () => {
    expect(commandFingerprint([
      { id: 'play', name: 'Play', linux: 'playerctl play-pause' },
    ])).toBe('4b1e677e')
  })

  it('requires a matching per-layout fingerprint', () => {
    const config = createDefaultConfig()
    config.settings.currentLayoutId = 'user:test'
    config.commands = [
      { id: 'play', name: 'Play', linux: 'playerctl play-pause' },
    ]

    expect(commandsTrusted(config)).toBe(false)

    config.settings.commandTrust[commandTrustKey(config.settings.currentLayoutId)] = {
      fingerprint: commandFingerprint(config.commands),
      trustedAt: '2026-05-04T00:00:00.000Z',
    }

    expect(commandsTrusted(config)).toBe(true)

    config.commands[0]!.linux = 'notify-send changed'
    expect(commandsTrusted(config)).toBe(false)
  })
})
