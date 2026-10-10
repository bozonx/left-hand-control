import type {
  AppSettings,
  LayoutConditionRule,
  LayoutConditionSet,
} from '~/types/config'
import {
  isConditionSetEmpty,
  isLayoutInAuto,
} from '~/utils/layoutAutoSwitch'

export type ConditionKind = 'whitelist' | 'blacklist'

export function useLayoutConditions() {
  const { config } = useConfig()

  function settings(): AppSettings {
    return config.value.settings
  }

  function ensureRule(layoutId: string): LayoutConditionRule {
    const map = settings().layoutConditions
    let rule = map[layoutId]
    if (!rule) {
      rule = {}
      map[layoutId] = rule
    }
    return rule
  }

  function getRule(layoutId: string): LayoutConditionRule | undefined {
    return settings().layoutConditions[layoutId]
  }

  function getConditionSet(
    layoutId: string,
    kind: ConditionKind,
  ): LayoutConditionSet {
    const rule = getRule(layoutId)
    return (
      rule?.[kind] ?? {
        gameMode: undefined,
        layouts: [],
        apps: [],
      }
    )
  }

  function setConditionSet(
    layoutId: string,
    kind: ConditionKind,
    value: LayoutConditionSet,
  ) {
    const rule = ensureRule(layoutId)
    if (isConditionSetEmpty(value)) {
      delete rule[kind]
    } else {
      const next: LayoutConditionSet = {
        gameMode: value.gameMode,
        layouts: [...value.layouts],
      }
      if (value.apps && value.apps.length > 0) {
        next.apps = [...value.apps]
      }
      rule[kind] = next
    }
    cleanupRule(layoutId, rule)
  }

  function setEnabledInAuto(layoutId: string, enabled: boolean) {
    const r = ensureRule(layoutId)
    if (enabled) {
      r.enabledInAuto = true
    } else {
      delete r.enabledInAuto
    }
    cleanupRule(layoutId, r)
  }

  function cleanupRule(layoutId: string, rule: LayoutConditionRule) {
    if (!rule.whitelist && !rule.blacklist && !rule.enabledInAuto) {
      delete settings().layoutConditions[layoutId]
    }
  }

  return {
    getRule,
    getConditionSet,
    setConditionSet,
    setEnabledInAuto,
    isInAuto: (layoutId: string) => isLayoutInAuto(getRule(layoutId)),
  }
}
