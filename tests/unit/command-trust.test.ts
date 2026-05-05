import { describe, expect, it } from 'vitest'

import { createDefaultConfig } from '~/types/config'
import type { AppConfig, Command, CommandTrustEntry } from '~/types/config'
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

  it('computes a fingerprint and verifies trust', () => {
    const config = createDefaultConfig()
    config.commands = [
      { id: 'music', name: 'Music', linux: 'playerctl play-pause', windows: '', macos: '' },
      { id: 'notify', name: 'Notify', linux: 'notify-send hello', windows: '', macos: '' },
    ]
    const fingerprint = commandFingerprint(config.commands)

    expect(fingerprint).toBe('950c50f2')

    config.settings.commandTrust[commandTrustKey('user:custom')] = {
      fingerprint,
      trustedAt: '2024-01-01T00:00:00.000Z',
    }
    config.settings.currentLayoutId = 'user:custom'

    expect(commandsTrusted(config)).toBe(true)
  })

  it('empty command list is always trusted', () => {
    const config = createDefaultConfig()
    config.commands = []
    expect(commandsTrusted(config)).toBe(true)
  })

  it('rejects wrong fingerprint', () => {
    const config = createDefaultConfig()
    config.commands = [
      { id: 'music', name: 'Music', linux: 'playerctl play-pause', windows: '', macos: '' },
    ]
    config.settings.commandTrust[commandTrustKey('user:custom')] = {
      fingerprint: '00000000',
      trustedAt: '2024-01-01T00:00:00.000Z',
    }
    config.settings.currentLayoutId = 'user:custom'

    expect(commandsTrusted(config)).toBe(false)
  })

  it('changing linux command invalidates trust', () => {
    const commands = [
      { id: 'music', name: 'Music', linux: 'playerctl play-pause', windows: '', macos: '' },
    ]
    const fp1 = commandFingerprint(commands)
    commands[0]!.linux = 'playerctl play-pause --changed'
    const fp2 = commandFingerprint(commands)
    expect(fp1).not.toBe(fp2)
  })

  it('changing command name does not affect fingerprint', () => {
    const commands = [
      { id: 'music', name: 'Music', linux: 'playerctl play-pause', windows: 'win', macos: 'mac' },
    ]
    const fp1 = commandFingerprint(commands)
    commands[0]!.name = 'Different'
    commands[0]!.windows = 'changed'
    commands[0]!.macos = 'changed'
    expect(commandFingerprint(commands)).toBe(fp1)
  })
})
