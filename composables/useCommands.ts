import {
  COMMAND_ACTION_PREFIX,
  type Command,
  parseCommandRef,
} from '~/types/config'

export function useCommands() {
  const { config } = useConfig()

  const commands = computed<Command[]>(() => config.value.commands ?? [])

  const byId = computed<Record<string, Command>>(() => {
    const entries: Record<string, Command> = {}
    for (const command of commands.value) entries[command.id] = command
    return entries
  })

  function commandNameById(id: string): string | undefined {
    return byId.value[id]?.name
  }

  function displayCommand(action: string | undefined | null): string {
    if (!action) return ''
    const commandRef = parseCommandRef(action)
    if (!commandRef) return action
    const name = commandNameById(commandRef)
    return name ? `> ${name}` : action
  }

  return {
    commands,
    byId,
    commandNameById,
    displayCommand,
    COMMAND_ACTION_PREFIX,
  }
}
