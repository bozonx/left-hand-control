// Current keyboard layout reported by the OS.
//
// Works only inside the Tauri shell; in a plain browser (`pnpm dev`) it
// silently stays `null`.

export interface LayoutInfo {
  short: string
  display: string
  long: string
  index: number
  backend: string
}

const _layout = ref<LayoutInfo | null>(null)
const _systemLayouts = ref<LayoutInfo[]>([])
const _error = ref<string | null>(null)
let _inited = false
let _unlisten: (() => void) | null = null

async function init() {
  if (_inited) return
  _inited = true
  try {
    const core = await import('@tauri-apps/api/core')
    const event = await import('@tauri-apps/api/event')

    async function refreshLayouts() {
      _layout.value = await core.invoke<LayoutInfo | null>('get_current_layout')
      _systemLayouts.value = await core.invoke<LayoutInfo[]>('get_system_layouts')
      _error.value = null
    }

    try {
      await refreshLayouts()
    } catch (e) {
      _error.value = String(e)
    }

    _unlisten = await event.listen<LayoutInfo>('layout-changed', (ev) => {
      _layout.value = ev.payload
      void refreshLayouts().catch((e) => {
        _error.value = e instanceof Error ? e.message : String(e)
      })
    })
  } catch {
    // Not running inside Tauri — leave as null, no error.
  }
}

async function setLayout(index: number): Promise<void> {
  try {
    const core = await import('@tauri-apps/api/core')
    await core.invoke('set_current_layout', { index })
    // Refresh immediately; the watcher will also fire `layout-changed`,
    // but going through a refresh here keeps the UI snappy if KDE
    // batches its signal.
    _layout.value = await core.invoke<LayoutInfo | null>('get_current_layout')
    _error.value = null
  } catch (e) {
    _error.value = e instanceof Error ? e.message : String(e)
    throw e
  }
}

export function useLayout() {
  void init()
  onScopeDispose(() => {
    // Keep listener alive across component teardown — state is module-global.
    void _unlisten
  })
  return {
    layout: computed(() => _layout.value),
    systemLayouts: computed(() => _systemLayouts.value),
    error: computed(() => _error.value),
    setLayout,
  }
}

export async function resetLayoutStateForTests() {
  _inited = false
  _layout.value = null
  _systemLayouts.value = []
  _error.value = null
  const unlisten = _unlisten
  _unlisten = null
  if (unlisten) {
    await unlisten()
  }
}
