import {
  type LayoutPreset,
  BUILTIN_LAYOUT_ID,
  USER_LAYOUT_PREFIX,
} from '~/types/config'
import {
  BUILTIN_LAYOUT_META,
  loadBuiltinLayout,
  parseLayoutYaml,
  serializeLayoutYaml,
} from '~/utils/layoutPresets'

const BROWSER_LAYOUTS_KEY = 'lhc:layouts'

function browserLayouts(): Record<string, string> {
  if (typeof localStorage === 'undefined') return {}
  try {
    return JSON.parse(localStorage.getItem(BROWSER_LAYOUTS_KEY) ?? '{}')
  } catch {
    return {}
  }
}

function saveBrowserLayouts(data: Record<string, string>) {
  if (typeof localStorage === 'undefined') return
  localStorage.setItem(BROWSER_LAYOUTS_KEY, JSON.stringify(data))
}

export function userLayoutId(name: string): string {
  return `${USER_LAYOUT_PREFIX}${name}`
}

export function isUserLayoutId(id: string | undefined): boolean {
  return !!id && id.startsWith(USER_LAYOUT_PREFIX)
}

export function userLayoutNameFromId(id: string): string {
  return id.startsWith(USER_LAYOUT_PREFIX)
    ? id.slice(USER_LAYOUT_PREFIX.length)
    : id
}

export interface LayoutLibraryEntry {
  id: string
  name: string
  // true for the bundled built-in preset.
  builtin: boolean
}

interface LayoutLibraryState {
  entries: Ref<LayoutLibraryEntry[]>
  layoutsDir: Ref<string>
  refresh: () => Promise<void>
  loadPreset: (id: string) => Promise<LayoutPreset | null>
  saveUserPreset: (name: string, preset: LayoutPreset) => Promise<string>
  deleteUserPreset: (name: string) => Promise<void>
}

let singleton: LayoutLibraryState | null = null

export function useLayoutLibrary(): LayoutLibraryState {
  if (singleton) return singleton

  const entries = ref<LayoutLibraryEntry[]>([
    {
      id: BUILTIN_LAYOUT_META.id,
      name: BUILTIN_LAYOUT_META.name,
      builtin: true,
    },
  ])
  const layoutsDir = ref('')

  async function refresh() {
    const tauri = await useTauri()
    let userNames: string[] = []
    if (tauri) {
      try {
        userNames = await tauri.invoke<string[]>('list_user_layouts')
        layoutsDir.value = await tauri.invoke<string>('get_layouts_dir')
      } catch (e) {
        console.error('[LHC] list_user_layouts failed:', e)
      }
    } else {
      userNames = Object.keys(browserLayouts()).sort()
      layoutsDir.value = '(browser: localStorage)'
    }
    entries.value = [
      {
        id: BUILTIN_LAYOUT_META.id,
        name: BUILTIN_LAYOUT_META.name,
        builtin: true,
      },
      ...userNames.map((n) => ({
        id: userLayoutId(n),
        name: n,
        builtin: false,
      })),
    ]
  }

  async function loadPreset(id: string): Promise<LayoutPreset | null> {
    if (id === BUILTIN_LAYOUT_ID) {
      return await loadBuiltinLayout()
    }
    if (!isUserLayoutId(id)) return null
    const name = userLayoutNameFromId(id)
    const tauri = await useTauri()
    let yaml = ''
    if (tauri) {
      try {
        yaml = await tauri.invoke<string>('load_user_layout', { name })
      } catch (e) {
        console.error('[LHC] load_user_layout failed:', e)
        return null
      }
    } else {
      yaml = browserLayouts()[name] ?? ''
    }
    if (!yaml) return null
    return parseLayoutYaml(yaml, name)
  }

  async function saveUserPreset(
    name: string,
    preset: LayoutPreset,
  ): Promise<string> {
    const toSave: LayoutPreset = { ...preset, name }
    const yaml = serializeLayoutYaml(toSave)
    const tauri = await useTauri()
    let savedName = name
    if (tauri) {
      savedName = await tauri.invoke<string>('save_user_layout', {
        name,
        contents: yaml,
      })
    } else {
      const all = browserLayouts()
      all[name] = yaml
      saveBrowserLayouts(all)
    }
    await refresh()
    return savedName
  }

  async function deleteUserPreset(name: string) {
    const tauri = await useTauri()
    if (tauri) {
      await tauri.invoke('delete_user_layout', { name })
    } else {
      const all = browserLayouts()
      delete all[name]
      saveBrowserLayouts(all)
    }
    await refresh()
  }

  singleton = {
    entries,
    layoutsDir,
    refresh,
    loadPreset,
    saveUserPreset,
    deleteUserPreset,
  }
  return singleton
}
