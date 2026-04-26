// Pure helpers for picking the active layout in auto mode and for
// evaluating layout / rule conditions.
//
// All inputs are plain JSON-serialisable values so the functions are
// trivial to unit-test.

import type {
  AppSettings,
  LayoutConditionRule,
  LayoutConditionSet,
} from '~/types/config'

export interface AutoSwitchContext {
  // Short id of the current system keyboard layout (e.g. 'us', 'ru').
  // `null` means layout detection is unavailable; layout-based
  // conditions then evaluate as "no match".
  currentSystemLayout: string | null
  // Whether Game Mode is currently active.
  gameModeActive: boolean
}

// Returns true when a condition set matches the current context.
// An empty set (no `gameMode` and no `layouts`) is considered a no-op
// and **does not match** by itself — the caller decides what to do
// with an absent set.
export function matchesConditionSet(
  set: LayoutConditionSet,
  ctx: AutoSwitchContext,
): boolean {
  if (set.gameMode === 'on' && !ctx.gameModeActive) return false
  if (set.gameMode === 'off' && ctx.gameModeActive) return false
  if (set.layouts.length > 0) {
    if (!ctx.currentSystemLayout) return false
    if (!set.layouts.includes(ctx.currentSystemLayout)) return false
  }
  return true
}

// Decision for whether a layout's rules should currently fire.
//   'allow' — apply the layout's rules normally.
//   'block' — act as native passthrough (do not apply rules).
export type LayoutGate = 'allow' | 'block'

export function evaluateLayoutGate(
  rule: LayoutConditionRule | undefined,
  ctx: AutoSwitchContext,
): LayoutGate {
  if (!rule) return 'allow'
  // Blacklist takes precedence: if it matches, the layout is blocked.
  if (rule.blacklist && matchesConditionSet(rule.blacklist, ctx)) {
    return 'block'
  }
  // Whitelist, when set, must match for the layout to be active.
  if (rule.whitelist && !matchesConditionSet(rule.whitelist, ctx)) {
    return 'block'
  }
  return 'allow'
}

// Returns true when an entry should be considered for auto-mode picking
// (i.e. it has whitelist/blacklist conditions or the user explicitly
// included it via the auto-toggle).
export function isLayoutInAuto(rule: LayoutConditionRule | undefined): boolean {
  if (!rule) return false
  if (rule.includedInAuto) return true
  if (rule.whitelist || rule.blacklist) return true
  return false
}

// Pick the active layout id in auto mode.
// Iterates over `availableIds` in `layoutOrder`-priority order, returns
// the first id whose layout is included in auto and whose conditions
// match. Falls back to `autoDefaultLayoutId` when no included layout
// matches. Returns `null` when nothing is selected (true native
// passthrough).
export function pickActiveLayout(
  availableIds: string[],
  settings: Pick<
    AppSettings,
    'layoutOrder' | 'layoutConditions' | 'autoDefaultLayoutId'
  >,
  ctx: AutoSwitchContext,
): string | null {
  const ordered = orderLayoutIds(availableIds, settings.layoutOrder)
  for (const id of ordered) {
    const rule = settings.layoutConditions[id]
    if (!isLayoutInAuto(rule)) continue
    if (evaluateLayoutGate(rule, ctx) === 'allow') {
      // For an "included" layout to match it must have at least one
      // active whitelist condition, otherwise it would always win and
      // make the priority list unusable.
      if (rule?.whitelist && matchesConditionSet(rule.whitelist, ctx)) {
        return id
      }
    }
  }
  if (
    settings.autoDefaultLayoutId &&
    availableIds.includes(settings.autoDefaultLayoutId)
  ) {
    return settings.autoDefaultLayoutId
  }
  return null
}

// Sort `availableIds` according to `layoutOrder`; ids missing from
// `layoutOrder` are appended at the end (preserving their original
// order).
export function orderLayoutIds(
  availableIds: string[],
  layoutOrder: string[],
): string[] {
  const set = new Set(availableIds)
  const out: string[] = []
  const seen = new Set<string>()
  for (const id of layoutOrder) {
    if (set.has(id) && !seen.has(id)) {
      out.push(id)
      seen.add(id)
    }
  }
  for (const id of availableIds) {
    if (!seen.has(id)) {
      out.push(id)
      seen.add(id)
    }
  }
  return out
}

export function isConditionSetEmpty(
  set: LayoutConditionSet | undefined,
): boolean {
  if (!set) return true
  return !set.gameMode && set.layouts.length === 0
}
