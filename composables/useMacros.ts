import {
  type Macro,
  MACRO_ACTION_PREFIX,
  parseMacroRef,
} from '~/types/config'

// Helpers around the user's macro list. Kept stateless — everything derives
// from the single `config.macros` source of truth in useConfig().

export function useMacros() {
  const { config } = useConfig()

  const macros = computed<Macro[]>(() => config.value.macros ?? [])

  const byId = computed<Record<string, Macro>>(() => {
    const m: Record<string, Macro> = {}
    for (const macro of macros.value) m[macro.id] = macro
    return m
  })

  function macroNameById(id: string): string | undefined {
    return byId.value[id]?.name
  }

  function displayAction(action: string | undefined | null): string {
    if (!action) return ''
    const ref = parseMacroRef(action)
    if (!ref) return action
    const name = macroNameById(ref)
    return name ? `▶ ${name}` : action
  }

  return {
    macros,
    byId,
    macroNameById,
    displayAction,
    MACRO_ACTION_PREFIX,
  }
}
