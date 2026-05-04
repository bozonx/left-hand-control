import type { AppConfig, Command } from '~/types/config'

export const CUSTOM_LAYOUT_TRUST_KEY = 'custom'

export function commandTrustKey(layoutId: string | undefined): string {
  return layoutId || CUSTOM_LAYOUT_TRUST_KEY
}

export function commandFingerprint(commands: Command[]): string {
  let hash = 0x811c9dc5
  const encoder = new TextEncoder()
  const update = (text: string) => {
    for (const byte of encoder.encode(text)) {
      hash ^= byte
      hash = Math.imul(hash, 0x01000193) >>> 0
    }
  }
  for (const command of commands) {
    update(command.id)
    update('\u0000')
    update(command.linux)
    update('\u0000')
  }
  return hash.toString(16).padStart(8, '0')
}

export function commandsTrusted(config: AppConfig): boolean {
  if (config.commands.length === 0) return true
  const key = commandTrustKey(config.settings.currentLayoutId)
  return config.settings.commandTrust[key]?.fingerprint === commandFingerprint(config.commands)
}
