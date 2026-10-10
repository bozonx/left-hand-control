import { describe, expect, it } from 'vitest'

import { createDefaultConfig } from '~/types/config'
import {
  analyzeRules,
  blockingRuleIssues,
  runtimeConfigForMapper,
} from '~/utils/ruleDiagnostics'

describe('rule diagnostics', () => {
  it('keeps empty-trigger rules saved but removes them from runtime config', () => {
    const config = createDefaultConfig()
    config.rules.push({
      id: 'draft',
      key: '',
      layerId: '',
      tapAction: 'Escape',
      holdAction: '',
      doubleTapAction: '',
    })

    const diagnostics = analyzeRules(config)

    expect(diagnostics.ignoredDraftCount).toBe(1)
    expect(blockingRuleIssues(diagnostics.issues)).toEqual([])
    expect(runtimeConfigForMapper(config).rules).toEqual([])
  })

  it('marks duplicate active triggers as blocking conflicts', () => {
    const config = createDefaultConfig()
    config.rules.push(
      {
        id: 'first',
        key: 'CapsLock',
        layerId: '',
        tapAction: 'Escape',
        holdAction: '',
        doubleTapAction: '',
      },
      {
        id: 'second',
        key: 'CapsLock',
        layerId: '',
        tapAction: 'Tab',
        holdAction: '',
        doubleTapAction: '',
      },
    )

    const issues = blockingRuleIssues(analyzeRules(config).issues)

    expect(issues.map((issue) => issue.code)).toEqual([
      'duplicateTrigger',
      'duplicateTrigger',
    ])
  })

  it('does not send disabled rules to the runtime config', () => {
    const config = createDefaultConfig()
    config.rules.push({
      id: 'disabled',
      enabled: false,
      key: 'CapsLock',
      layerId: '',
      tapAction: 'not-a-real-action',
      holdAction: '',
      doubleTapAction: '',
    })

    expect(analyzeRules(config).issues).toEqual([])
    expect(runtimeConfigForMapper(config).rules).toEqual([])
  })
})
