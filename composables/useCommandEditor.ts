import { parseCommandRef, type Command } from '~/types/config'
import { randomId } from '~/utils/keys'

const ID_RE = /^[A-Za-z0-9_-]{1,64}$/

export function useCommandEditor() {
  const { config } = useConfig()
  const { t } = useI18n()
  const commandUiKeys = new WeakMap<Command, string>()

  function newCommandId(base?: string): string {
    if (base && !config.value.commands.some((command) => command.id === base)) return base
    if (base) {
      for (let i = 2; i < 1000; i += 1) {
        const candidate = `${base}${i}`
        if (!config.value.commands.some((command) => command.id === candidate)) return candidate
      }
    }
    let id = ''
    do {
      id = randomId()
    } while (config.value.commands.some((command) => command.id === id))
    return id
  }

  function addCommand() {
    if (!Array.isArray(config.value.commands)) config.value.commands = []
    const command: Command = {
      id: newCommandId(),
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
    const index = config.value.commands.findIndex((command) => uiKeyOf(command) === uiKey)
    const next = index + delta
    if (index < 0 || next < 0 || next >= config.value.commands.length) return
    const commands = config.value.commands.slice()
    const [item] = commands.splice(index, 1) as [Command]
    commands.splice(next, 0, item)
    config.value.commands = commands
  }

  function uiKeyOf(command: Command) {
    let key = commandUiKeys.get(command)
    if (!key) {
      key = randomId()
      commandUiKeys.set(command, key)
    }
    return key
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

  const usage = computed(() => {
    const byCommand: Record<string, string[]> = {}
    const note = (id: string, where: string) => {
      if (!byCommand[id]) byCommand[id] = []
      byCommand[id].push(where)
    }
    const noteIfCommand = (action: string | null | undefined, where: string) => {
      const id = action ? parseCommandRef(action) : null
      if (id) note(id, where)
    }
    for (const rule of config.value.rules) {
      const ruleLabel = rule.key || '?'
      noteIfCommand(rule.tapAction, `rule ${ruleLabel} (tap)`)
      noteIfCommand(rule.holdAction, `rule ${ruleLabel} (hold)`)
      noteIfCommand(rule.doubleTapAction, `rule ${ruleLabel} (double-tap)`)
    }
    for (const [layerId, keymap] of Object.entries(config.value.layerKeymaps)) {
      for (const [code, action] of Object.entries(keymap.keys ?? {})) {
        noteIfCommand(action, `${layerId}.${code}`)
      }
      for (const extra of keymap.extras ?? []) {
        noteIfCommand(extra.action, `${layerId}.${extra.name || 'extra'}`)
      }
    }
    for (const macro of config.value.macros) {
      for (const [index, step] of macro.steps.entries()) {
        noteIfCommand(step.keystroke, `macro ${macro.id} (#${index + 1})`)
      }
    }
    return byCommand
  })

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
