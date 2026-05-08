import type { AppConfig, LayerRule } from '~/types/config'
import { validateActionValue } from '~/utils/actionValidation'
import {
  isCanonicalAction,
  isKeystrokeAction,
  isKnownActionKey,
} from '~/utils/actionSyntax'

export type RuleIssueSeverity = 'error' | 'warning'

export type RuleIssueCode =
  | 'missingTrigger'
  | 'invalidTrigger'
  | 'duplicateTrigger'
  | 'unknownLayer'
  | 'invalidTapAction'
  | 'invalidHoldAction'
  | 'invalidDoubleTapAction'

export interface RuleIssue {
  code: RuleIssueCode
  severity: RuleIssueSeverity
  ruleId: string
  trigger?: string
}

export interface RuleRuntimeDiagnostics {
  issues: RuleIssue[]
  ignoredDraftCount: number
  runnableRuleCount: number
}

function isEnabled(rule: LayerRule): boolean {
  return rule.enabled !== false
}

function hasTrigger(rule: LayerRule): boolean {
  return rule.key.trim().length > 0
}

function triggerHasIssue(trigger: string): boolean {
  return !isKnownActionKey(trigger) || ['MouseLeft', 'MouseRight', 'MouseMiddle'].includes(trigger)
}

function actionHasIssue(action: string | null | undefined, config: AppConfig): boolean {
  return validateActionValue(action, config) !== null
}

function holdActionHasIssue(action: string | null | undefined): boolean {
  const raw = action ?? ''
  return raw.length > 0 && (!isCanonicalAction(raw) || !isKeystrokeAction(raw))
}

export function analyzeRules(config: AppConfig): RuleRuntimeDiagnostics {
  const issues: RuleIssue[] = []
  const layerIds = new Set(config.layers.map((layer) => layer.id))
  const rulesByTrigger = new Map<string, LayerRule[]>()
  let ignoredDraftCount = 0
  let runnableRuleCount = 0

  for (const rule of config.rules) {
    if (!isEnabled(rule)) continue

    if (!hasTrigger(rule)) {
      ignoredDraftCount += 1
      issues.push({
        code: 'missingTrigger',
        severity: 'warning',
        ruleId: rule.id,
      })
      continue
    }

    runnableRuleCount += 1
    const trigger = rule.key.trim()
    if (triggerHasIssue(trigger)) {
      issues.push({
        code: 'invalidTrigger',
        severity: 'error',
        ruleId: rule.id,
        trigger,
      })
    }
    const existing = rulesByTrigger.get(trigger) ?? []
    existing.push(rule)
    rulesByTrigger.set(trigger, existing)

    if (rule.layerId && !layerIds.has(rule.layerId)) {
      issues.push({
        code: 'unknownLayer',
        severity: 'error',
        ruleId: rule.id,
        trigger,
      })
    }
    if (actionHasIssue(rule.tapAction, config)) {
      issues.push({
        code: 'invalidTapAction',
        severity: 'error',
        ruleId: rule.id,
        trigger,
      })
    }
    if (holdActionHasIssue(rule.holdAction)) {
      issues.push({
        code: 'invalidHoldAction',
        severity: 'error',
        ruleId: rule.id,
        trigger,
      })
    }
    if (actionHasIssue(rule.doubleTapAction, config)) {
      issues.push({
        code: 'invalidDoubleTapAction',
        severity: 'error',
        ruleId: rule.id,
        trigger,
      })
    }
  }

  for (const [trigger, rules] of rulesByTrigger) {
    if (rules.length < 2) continue
    for (const rule of rules) {
      issues.push({
        code: 'duplicateTrigger',
        severity: 'error',
        ruleId: rule.id,
        trigger,
      })
    }
  }

  return {
    issues,
    ignoredDraftCount,
    runnableRuleCount,
  }
}

export function ruleIssuesById(issues: RuleIssue[]): Record<string, RuleIssue[]> {
  return issues.reduce<Record<string, RuleIssue[]>>((acc, issue) => {
    acc[issue.ruleId] = [...(acc[issue.ruleId] ?? []), issue]
    return acc
  }, {})
}

export function runtimeConfigForMapper(config: AppConfig): AppConfig {
  return {
    ...config,
    rules: config.rules.filter((rule) => isEnabled(rule) && hasTrigger(rule)),
  }
}

export function blockingRuleIssues(issues: RuleIssue[]): RuleIssue[] {
  return issues.filter((issue) => issue.severity === 'error')
}

export function ruleIssueMessageKey(issue: RuleIssue): string {
  return `rules.issueMessages.${issue.code}`
}

export function ruleIssueFallbackMessage(issue: RuleIssue): string {
  const trigger = issue.trigger ? `"${issue.trigger}"` : 'without a trigger'
  switch (issue.code) {
    case 'missingTrigger':
      return 'Rule without a trigger is saved as a draft and will be ignored.'
    case 'invalidTrigger':
      return `Trigger ${trigger} cannot be used for a rule.`
    case 'duplicateTrigger':
      return `Trigger ${trigger} is used by more than one active rule.`
    case 'unknownLayer':
      return `Rule ${trigger} points to a layer that no longer exists.`
    case 'invalidTapAction':
      return `Rule ${trigger} has an invalid tap action.`
    case 'invalidHoldAction':
      return `Rule ${trigger} has an invalid hold action.`
    case 'invalidDoubleTapAction':
      return `Rule ${trigger} has an invalid double-tap action.`
  }
}
