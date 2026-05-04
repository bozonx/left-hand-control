import type { AppConfig } from '~/types/config'
import {
  parseCommandRef,
  parseMacroRef,
  parseSystemRef,
} from '~/types/config'
import { isCanonicalAction } from '~/utils/actionSyntax'
import { systemActionById } from '~/utils/systemActions'
import { systemMacroById } from '~/utils/systemMacros'

export type ActionValidationIssue =
  | 'invalidSyntax'
  | 'macroNotAllowed'
  | 'macroSelfReference'
  | 'unknownMacro'
  | 'unknownCommand'
  | 'unknownSystemAction'

export interface ActionValidationOptions {
  allowMacros?: boolean
  excludedMacroId?: string
}

export function validateActionValue(
  action: string | null | undefined,
  config: AppConfig,
  options: ActionValidationOptions = {},
): ActionValidationIssue | null {
  const raw = action ?? ''
  if (!raw) return null
  if (!isCanonicalAction(raw)) return 'invalidSyntax'

  const macroId = parseMacroRef(raw)
  if (macroId !== null) {
    if (options.allowMacros === false) return 'macroNotAllowed'
    if (options.excludedMacroId && macroId === options.excludedMacroId) {
      return 'macroSelfReference'
    }
    if (config.macros.some((macro) => macro.id === macroId) || systemMacroById(macroId)) {
      return null
    }
    return 'unknownMacro'
  }

  const commandId = parseCommandRef(raw)
  if (commandId !== null) {
    return config.commands.some((command) => command.id === commandId)
      ? null
      : 'unknownCommand'
  }

  const systemId = parseSystemRef(raw)
  if (systemId !== null) {
    return systemActionById(systemId) ? null : 'unknownSystemAction'
  }

  return null
}
