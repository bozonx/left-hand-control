import {
  type AppConfig,
  BASE_LAYER_ID,
  createDefaultConfig,
} from '~/types/config'
import { loadDefaultsYaml } from '~/utils/defaultLayers'

type TauriCore = typeof import('@tauri-apps/api/core')

let tauriCache: TauriCore | null | undefined
async function getTauri(): Promise<TauriCore | null> {
  if (tauriCache !== undefined) return tauriCache
  try {
    tauriCache = await import('@tauri-apps/api/core')
  } catch {
    tauriCache = null
  }
  return tauriCache
}

const BROWSER_STORAGE_KEY = 'lhc:config'

// Dev-only: when VITE_LHC_USE_DEFAULTS is truthy, skip the persisted user
// config entirely on startup and seed the app from `public/default-layers.yaml`.
// Changes made in the UI are kept in memory only (not saved to disk).
const USE_DEFAULTS_ONLY =
  import.meta.env.DEV &&
  ['1', 'true', 'yes', 'on'].includes(
    String(import.meta.env.VITE_LHC_USE_DEFAULTS ?? '').toLowerCase(),
  )

async function readRaw(): Promise<string> {
  const tauri = await getTauri()
  if (tauri) {
    return await tauri.invoke<string>('load_config')
  }
  if (typeof localStorage !== 'undefined') {
    return localStorage.getItem(BROWSER_STORAGE_KEY) ?? ''
  }
  return ''
}

async function writeRaw(contents: string): Promise<void> {
  const tauri = await getTauri()
  if (tauri) {
    await tauri.invoke('save_config', { contents })
    return
  }
  if (typeof localStorage !== 'undefined') {
    localStorage.setItem(BROWSER_STORAGE_KEY, contents)
  }
}

export async function getConfigPath(): Promise<string> {
  const tauri = await getTauri()
  if (!tauri) return '(browser: localStorage)'
  try {
    return await tauri.invoke<string>('get_config_path')
  } catch {
    return ''
  }
}

// Merge a (possibly partial / old-version) persisted config with defaults so
// the UI always sees a complete shape.
function normalize(raw: unknown): AppConfig {
  const base = createDefaultConfig()
  if (!raw || typeof raw !== 'object') return base
  const r = raw as Partial<AppConfig>
  const cfg: AppConfig = {
    version: 1,
    layers:
      Array.isArray(r.layers) && r.layers.length
        ? r.layers
        : base.layers,
    rules: Array.isArray(r.rules) ? r.rules : [],
    layerKeymaps:
      r.layerKeymaps && typeof r.layerKeymaps === 'object'
        ? r.layerKeymaps
        : base.layerKeymaps,
    settings: { ...base.settings, ...(r.settings ?? {}) },
  }
  if (!cfg.layers.some((l) => l.id === BASE_LAYER_ID)) {
    cfg.layers.unshift({ id: BASE_LAYER_ID, name: 'Base' })
  }
  for (const layer of cfg.layers) {
    if (!cfg.layerKeymaps[layer.id]) {
      cfg.layerKeymaps[layer.id] = { keys: {}, extras: [] }
    } else {
      const km = cfg.layerKeymaps[layer.id]
      if (!km.keys || typeof km.keys !== 'object') km.keys = {}
      if (!Array.isArray(km.extras)) km.extras = []
    }
  }
  return cfg
}

interface ConfigState {
  config: Ref<AppConfig>
  loaded: Ref<boolean>
  saving: Ref<boolean>
  lastError: Ref<string | null>
  configPath: Ref<string>
  load: () => Promise<void>
  flush: () => Promise<void>
}

let singleton: ConfigState | null = null

export function useConfig(): ConfigState {
  if (singleton) return singleton

  const config = ref<AppConfig>(createDefaultConfig())
  const loaded = ref(false)
  const saving = ref(false)
  const lastError = ref<string | null>(null)
  const configPath = ref('')

  let saveTimer: ReturnType<typeof setTimeout> | null = null
  let pendingFlushResolvers: Array<() => void> = []

  async function persistNow() {
    if (USE_DEFAULTS_ONLY) {
      // In defaults-only dev mode we intentionally never write to disk.
      const resolvers = pendingFlushResolvers
      pendingFlushResolvers = []
      for (const r of resolvers) r()
      return
    }
    saving.value = true
    try {
      await writeRaw(JSON.stringify(config.value, null, 2))
      lastError.value = null
    } catch (e: unknown) {
      lastError.value = e instanceof Error ? e.message : String(e)
    } finally {
      saving.value = false
      const resolvers = pendingFlushResolvers
      pendingFlushResolvers = []
      for (const r of resolvers) r()
    }
  }

  function scheduleSave() {
    if (!loaded.value) return
    if (saveTimer) clearTimeout(saveTimer)
    saveTimer = setTimeout(() => {
      saveTimer = null
      void persistNow()
    }, 300)
  }

  async function flush() {
    if (saveTimer) {
      clearTimeout(saveTimer)
      saveTimer = null
      await persistNow()
      return
    }
    if (saving.value) {
      await new Promise<void>((resolve) => {
        pendingFlushResolvers.push(resolve)
      })
    }
  }

  async function load() {
    let freshlyInitialized = false
    try {
      if (USE_DEFAULTS_ONLY) {
        const seeded = await loadDefaultsYaml()
        config.value = seeded ?? createDefaultConfig()
        configPath.value = '(dev: defaults-only, not persisted)'
        lastError.value = null
        console.info(
          '[LHC] VITE_LHC_USE_DEFAULTS=true — using public/default-layers.yaml, changes will NOT be saved',
        )
        return
      }
      const raw = await readRaw()
      if (raw) {
        try {
          config.value = normalize(JSON.parse(raw))
        } catch {
          config.value = createDefaultConfig()
        }
      } else {
        // No saved config yet — seed from bundled YAML defaults.
        const seeded = await loadDefaultsYaml()
        config.value = seeded ?? createDefaultConfig()
        freshlyInitialized = true
      }
      configPath.value = await getConfigPath()
      lastError.value = null
    } catch (e: unknown) {
      lastError.value = e instanceof Error ? e.message : String(e)
      config.value = createDefaultConfig()
    } finally {
      loaded.value = true
    }
    // Persist seeded defaults immediately so subsequent launches don't
    // re-read the YAML and overwrite any user edits made before the first
    // auto-save would have fired.
    if (freshlyInitialized) {
      await persistNow()
    }
  }

  // Auto-save: any deep change to `config` while loaded schedules a save.
  watch(
    config,
    () => {
      scheduleSave()
    },
    { deep: true },
  )

  singleton = {
    config,
    loaded,
    saving,
    lastError,
    configPath,
    load,
    flush,
  }
  return singleton
}
