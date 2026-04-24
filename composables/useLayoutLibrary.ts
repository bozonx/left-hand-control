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
  error: Ref<string | null>
  refresh: () => Promise<void>
  loadPreset: (id: string) => Promise<LayoutPreset | null>
  saveUserPreset: (name: string, preset: LayoutPreset) => Promise<string>
  deleteUserPreset: (name: string) => Promise<void>
}

let singleton: LayoutLibraryState | null = null

export function resetLayoutLibraryStateForTests() {
  singleton = null
}

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
  const error = ref<string | null>(null)

  async function refresh() {
    const tauri = await useTauri()
    if (!tauri) {
      entries.value = [
        {
          id: BUILTIN_LAYOUT_META.id,
          name: BUILTIN_LAYOUT_META.name,
          builtin: true,
        },
      ]
      layoutsDir.value = ''
      error.value = null
      return
    }

    let userNames: string[] = []
    try {
      userNames = await tauri.invoke<string[]>('list_user_layouts')
      layoutsDir.value = await tauri.invoke<string>('get_layouts_dir')
      error.value = null
    } catch (e) {
      error.value = e instanceof Error ? e.message : String(e)
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
    if (!tauri) return null
    let yaml = ''
    try {
      yaml = await tauri.invoke<string>('load_user_layout', { name })
      error.value = null
    } catch (e) {
      error.value = e instanceof Error ? e.message : String(e)
      return null
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
    if (!tauri) return name
    let savedName = name
    savedName = await tauri.invoke<string>('save_user_layout', {
      name,
      contents: yaml,
    })
    await refresh()
    return savedName
  }

  async function deleteUserPreset(name: string) {
    const tauri = await useTauri()
    if (!tauri) return
    await tauri.invoke('delete_user_layout', { name })
    await refresh()
  }

  singleton = {
    entries,
    layoutsDir,
    error,
    refresh,
    loadPreset,
    saveUserPreset,
    deleteUserPreset,
  }
  return singleton
}
