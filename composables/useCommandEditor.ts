import { parseCommandRef, type Command } from '~/types/config'
import { useEntityEditor } from '~/composables/useEntityEditor'
import { useActionUsage } from '~/composables/useActionUsage'

const ID_RE = /^[A-Za-z0-9_-]{1,64}$/

export function useCommandEditor() {
  const { config } = useConfig()
  const { t } = useI18n()
  const commandsRef = computed({
    get: () => config.value.commands,
    set: (val) => { config.value.commands = val },
  })
  const { newId, uiKeyOf, moveEntity } = useEntityEditor(commandsRef)
  const { usage } = useActionUsage(parseCommandRef)

  function addCommand() {
    if (!Array.isArray(config.value.commands)) config.value.commands = []
    const command: Command = {
      id: newId(),
      name: t('commands.defaultName'),
      linux: '',
    }
    config.value.commands.unshift(command)
    return command
  }

  function removeCommand(uiKey: string) {
    config.value.commands = config.value.commands.filter(
      (command) => uiKeyOf(command) !== uiKey,
    )
  }

  function moveCommand(uiKey: string, delta: number) {
    moveEntity(uiKey, delta)
  }

  const idCounts = computed<Record<string, number>>(() => {
    const counts: Record<string, number> = {}
    for (const command of config.value.commands) {
      counts[command.id] = (counts[command.id] ?? 0) + 1
    }
    return counts
  })

  function idError(command: Command): string | null {
    const raw = command.id ?? ''
    if (raw.trim() === '') return t('commands.idErrors.empty')
    if (!ID_RE.test(raw)) return t('commands.idErrors.format')
    if ((idCounts.value[raw] ?? 0) > 1) return t('commands.idErrors.dupUser')
    return null
  }

  function linuxError(command: Command): string | null {
    if (command.linux.trim()) return null
    return t('commands.linuxErrors.empty')
  }

  const hasErrors = computed(() =>
    config.value.commands.some((command) => idError(command) !== null || linuxError(command) !== null),
  )

  return {
    addCommand,
    removeCommand,
    moveCommand,
    uiKeyOf,
    idError,
    linuxError,
    hasErrors,
    usage,
  }
}
