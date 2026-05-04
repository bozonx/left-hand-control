import {
  commandFingerprint,
  commandTrustKey,
  commandsTrusted,
} from '~/utils/commandTrust'

export function useCommandTrust() {
  const { config, flush } = useConfig()

  const trustKey = computed(() => commandTrustKey(config.value.settings.currentLayoutId))
  const fingerprint = computed(() => commandFingerprint(config.value.commands))
  const hasShellCommands = computed(() => config.value.commands.length > 0)
  const isTrusted = computed(() => commandsTrusted(config.value))
  const needsApproval = computed(() => hasShellCommands.value && !isTrusted.value)

  async function approve() {
    config.value.settings.commandTrust[trustKey.value] = {
      fingerprint: fingerprint.value,
      trustedAt: new Date().toISOString(),
    }
    await nextTick()
    await flush()
  }

  async function revoke() {
    delete config.value.settings.commandTrust[trustKey.value]
    await nextTick()
    await flush()
  }

  return {
    trustKey,
    fingerprint,
    hasShellCommands,
    isTrusted,
    needsApproval,
    approve,
    revoke,
  }
}
