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
  // Title of the currently focused window. `null` means detection is
  // unavailable; app-based conditions then evaluate as "no match".
  activeWindowTitle?: string | null
  // Application id / WM_CLASS of the currently focused window.
  activeWindowAppId?: string | null
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
  if (set.apps && set.apps.length > 0) {
    if (!matchesActiveWindow(set.apps, ctx)) return false
  }
  return true
}

// Returns true when at least one substring (case-insensitive) is found
// either in the active window title or in its app id.
function matchesActiveWindow(
  needles: string[],
  ctx: AutoSwitchContext,
): boolean {
  const title = (ctx.activeWindowTitle ?? '').toLowerCase()
  const appId = (ctx.activeWindowAppId ?? '').toLowerCase()
  if (!title && !appId) return false
  return needles
    .map((n) => n.trim().toLowerCase())
    .filter((n) => n.length > 0)
    .some((n) => title.includes(n) || appId.includes(n))
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

  // When a whitelist exists, it must match.
  if (rule.whitelist) {
    // Blacklist takes precedence: if it matches, the layout is blocked.
    if (rule.blacklist && matchesConditionSet(rule.blacklist, ctx)) {
      return 'block'
    }
    // Whitelist must match for the layout to be active.
    if (!matchesConditionSet(rule.whitelist, ctx)) {
      return 'block'
    }
    return 'allow'
  }

  // When only a blacklist exists (no whitelist), block only if the
  // blacklist matches — everything else is allowed.
  if (rule.blacklist) {
    if (matchesConditionSet(rule.blacklist, ctx)) {
      return 'block'
    }
    return 'allow'
  }

  return 'allow'
}

// Returns true when an entry should be considered for auto-mode picking.
// A layout is included when it has a whitelist or blacklist condition and is
// not explicitly disabled via the auto-toggle.
export function isLayoutInAuto(rule: LayoutConditionRule | undefined): boolean {
  if (!rule) return false
  if (rule.disabledInAuto) return false
  if (rule.whitelist) return true
  if (rule.blacklist) return true
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
      return id
    }
  }
  if (
    settings.autoDefaultLayoutId &&
    availableIds.includes(settings.autoDefaultLayoutId)
  ) {
    const defaultRule = settings.layoutConditions[settings.autoDefaultLayoutId]
    if (!defaultRule || !defaultRule.disabledInAuto) {
      return settings.autoDefaultLayoutId
    }
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
  return !set.gameMode && set.layouts.length === 0 && (set.apps?.length ?? 0) === 0
}
