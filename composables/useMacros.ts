import {
  type Macro,
  MACRO_ACTION_PREFIX,
  parseMacroRef,
  parseSystemRef,
} from '~/types/config'
import { systemActionById } from '~/utils/systemActions'
import { systemMacroById } from '~/utils/systemMacros'

// Helpers around macro references. `macro:<id>` resolves against the user's
// `config.macros` first and falls back to the built-in system macros
// catalog (see `utils/systemMacros.ts`). User macros with the same id as
// a system macro override it.

export function useMacros() {
  const { config } = useConfig()

  const macros = computed<Macro[]>(() => config.value.macros ?? [])

  const byId = computed<Record<string, Macro>>(() => {
    const m: Record<string, Macro> = {}
    for (const macro of macros.value) m[macro.id] = macro
    return m
  })

  function macroNameById(id: string): string | undefined {
    return byId.value[id]?.name ?? systemMacroById(id)?.name
  }

  function displayAction(action: string | undefined | null): string {
    if (!action) return ''
    const macroRef = parseMacroRef(action)
    if (macroRef) {
      const name = macroNameById(macroRef)
      return name ? `▶ ${name}` : action
    }
    const sysRef = parseSystemRef(action)
    if (sysRef) {
      const found = systemActionById(sysRef)
      return found ? `⚙ ${found.name}` : action
    }
    return action
  }

  return {
    macros,
    byId,
    macroNameById,
    displayAction,
    MACRO_ACTION_PREFIX,
  }
}
