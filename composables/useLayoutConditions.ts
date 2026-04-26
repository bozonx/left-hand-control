// Helpers for reading and mutating the per-layout conditions stored in
// `AppSettings.layoutConditions`, plus the auto-mode fallback default.
//
// Centralised here so the UI does not have to think about object
// initialisation, the "auto-include on first condition" rule, or the
// mutual exclusivity between `autoDefaultLayoutId` and conditions.

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
      }
    )
  }

  function setConditionSet(
    layoutId: string,
    kind: ConditionKind,
    value: LayoutConditionSet,
  ) {
    if (settings().autoDefaultLayoutId === layoutId) {
      // Default layout cannot have conditions.
      return
    }
    const rule = ensureRule(layoutId)
    if (isConditionSetEmpty(value)) {
      delete rule[kind]
    } else {
      rule[kind] = { gameMode: value.gameMode, layouts: [...value.layouts] }
      // Auto-enable participation when the user adds the first condition.
      rule.includedInAuto = true
    }
    cleanupRule(layoutId, rule)
  }

  function setIncludedInAuto(layoutId: string, included: boolean) {
    if (!included) {
      // Cannot opt out while conditions are still defined.
      const rule = getRule(layoutId)
      if (rule?.whitelist || rule?.blacklist) return
    }
    const rule = ensureRule(layoutId)
    rule.includedInAuto = included
    cleanupRule(layoutId, rule)
  }

  function setAsDefault(layoutId: string | undefined) {
    const s = settings()
    if (!layoutId) {
      s.autoDefaultLayoutId = undefined
      return
    }
    // Default cannot have conditions — drop them defensively.
    const rule = s.layoutConditions[layoutId]
    if (rule) {
      delete rule.whitelist
      delete rule.blacklist
      // Default doesn't need the includedInAuto flag — it's a fallback,
      // not a candidate. Drop the rule entirely if nothing remains.
      cleanupRule(layoutId, rule)
    }
    s.autoDefaultLayoutId = layoutId
  }

  function cleanupRule(layoutId: string, rule: LayoutConditionRule) {
    if (!rule.whitelist && !rule.blacklist && !rule.includedInAuto) {
      delete settings().layoutConditions[layoutId]
    }
  }

  return {
    getRule,
    getConditionSet,
    setConditionSet,
    setIncludedInAuto,
    setAsDefault,
    isInAuto: (layoutId: string) => isLayoutInAuto(getRule(layoutId)),
  }
}
