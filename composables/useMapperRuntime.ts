import { layoutSnapshotOf, emptyLayoutPreset, applyPresetToConfig } from '~/utils/layoutPresets'
import type { AppConfig } from '~/types/config'
import type { MapperStatus } from '~/composables/useMapper'
import { commandsTrusted } from '~/utils/commandTrust'
import {
  analyzeRules,
  blockingRuleIssues,
  ruleIssueFallbackMessage,
  runtimeConfigForMapper,
} from '~/utils/ruleDiagnostics'

export function useMapperRuntime(
  deps: {
    status: Ref<MapperStatus>
    busy: Ref<boolean>
    invokeStart: (devicePath: string, mouseDevicePath?: string) => Promise<void>
    invokeStop: () => Promise<void>
    invokeUpdateConfig: (config: AppConfig) => Promise<void>
  },
) {
  const { config } = useConfig()
  const { activeAutoLayoutId } = useLayoutSwitcher()
  const library = useLayoutLibrary()

  let reloadTimer: ReturnType<typeof setTimeout> | null = null
  let reloadPending = false

  async function computeActiveConfig(): Promise<AppConfig> {
    const settings = config.value.settings
    const activeId = settings.layoutMode === 'auto'
      ? activeAutoLayoutId?.value
      : settings.manualActiveLayoutId

    // undefined in auto mode means the switcher hasn't run yet — keep current config
    // rather than wiping layers with an empty preset.
    if (settings.layoutMode === 'auto' && activeId === undefined) {
      return config.value
    }

    if (activeId === settings.currentLayoutId) {
      return config.value
    }

    let preset = emptyLayoutPreset()
    if (activeId) {
      const loaded = await library.loadPreset(activeId)
      if (!loaded) {
        throw new Error(`Active layout could not be loaded: ${activeId}`)
      }
      if (loaded) preset = loaded
    }
    return applyPresetToConfig(config.value, preset, activeId ?? undefined)
  }

  async function computeRuntimeConfig(): Promise<AppConfig> {
    const activeConfig = await computeActiveConfig()
    const diagnostics = analyzeRules(activeConfig)
    const blockingIssues = blockingRuleIssues(diagnostics.issues)
    if (blockingIssues.length > 0) {
      throw new Error(ruleIssueFallbackMessage(blockingIssues[0]!))
    }
    return runtimeConfigForMapper(activeConfig)
  }

  const runtimeSnapshot = async () => {
    const cfg = await computeRuntimeConfig()
    const settings = config.value.settings
    const activeLayoutId = settings.layoutMode === 'auto'
      ? activeAutoLayoutId?.value
      : settings.manualActiveLayoutId
    return JSON.stringify({
      activeLayoutId: activeLayoutId ?? null,
      layout: layoutSnapshotOf(cfg),
      defaultHoldTimeoutMs: cfg.settings.defaultHoldTimeoutMs,
      defaultDoubleTapTimeoutMs: cfg.settings.defaultDoubleTapTimeoutMs,
      defaultMacroStepPauseMs: cfg.settings.defaultMacroStepPauseMs,
      defaultMacroModifierDelayMs: cfg.settings.defaultMacroModifierDelayMs,
      commandsTrusted: commandsTrusted(cfg),
    })
  }

  let lastRuntimeSnapshot = JSON.stringify({
    activeLayoutId:
      config.value.settings.layoutMode === 'auto'
        ? activeAutoLayoutId?.value ?? null
        : config.value.settings.manualActiveLayoutId ?? null,
    layout: layoutSnapshotOf(config.value),
    defaultHoldTimeoutMs: config.value.settings.defaultHoldTimeoutMs,
    defaultDoubleTapTimeoutMs: config.value.settings.defaultDoubleTapTimeoutMs,
    defaultMacroStepPauseMs: config.value.settings.defaultMacroStepPauseMs,
    defaultMacroModifierDelayMs: config.value.settings.defaultMacroModifierDelayMs,
    commandsTrusted: commandsTrusted(config.value),
  })
  void runtimeSnapshot()
    .then((s) => {
      lastRuntimeSnapshot = s
    })
    .catch((error) => {
      logger.debug('[mapper] initial runtime snapshot skipped', error)
    })

  function initSnapshot() {
    lastRuntimeSnapshot = JSON.stringify({
      activeLayoutId:
        config.value.settings.layoutMode === 'auto'
          ? activeAutoLayoutId?.value ?? null
          : config.value.settings.manualActiveLayoutId ?? null,
      layout: layoutSnapshotOf(config.value),
      defaultHoldTimeoutMs: config.value.settings.defaultHoldTimeoutMs,
      defaultDoubleTapTimeoutMs: config.value.settings.defaultDoubleTapTimeoutMs,
      defaultMacroStepPauseMs: config.value.settings.defaultMacroStepPauseMs,
      defaultMacroModifierDelayMs: config.value.settings.defaultMacroModifierDelayMs,
      commandsTrusted: commandsTrusted(config.value),
    })
    void runtimeSnapshot()
      .then((s) => {
        lastRuntimeSnapshot = s
      })
      .catch((error) => {
        logger.debug('[mapper] initial runtime snapshot skipped', error)
      })
  }

  function scheduleReload() {
    if (reloadTimer) clearTimeout(reloadTimer)
    reloadTimer = setTimeout(() => {
      reloadTimer = null
      void reloadRunningMapper()
    }, 150)
  }

  async function reloadRunningMapper() {
    const devicePath = deps.status.value.device_path ?? config.value.settings.inputDevicePath ?? ''
    if (!deps.status.value.running || !devicePath) return
    if (deps.busy.value) {
      reloadPending = true
      return
    }

    deps.busy.value = true
    try {
      const nextRuntimeSnapshot = await runtimeSnapshot()
      if (nextRuntimeSnapshot === lastRuntimeSnapshot) return
      const nextConfig = await computeRuntimeConfig()
      await deps.invokeUpdateConfig(nextConfig)
      lastRuntimeSnapshot = nextRuntimeSnapshot
    } catch (error) {
      logger.error('[mapper] live config update failed', error)
    } finally {
      deps.busy.value = false
      if (reloadPending) {
        reloadPending = false
        scheduleReload()
      }
    }
  }

  function resetState() {
    if (reloadTimer) {
      clearTimeout(reloadTimer)
      reloadTimer = null
    }
    reloadPending = false
  }

  return {
    computeActiveConfig,
    computeRuntimeConfig,
    runtimeSnapshot,
    initSnapshot,
    scheduleReload,
    reloadRunningMapper,
    resetState,
  }
}
