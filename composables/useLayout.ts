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
const _error = ref<string | null>(null)
let _inited = false
let _unlisten: (() => void) | null = null

async function init() {
  if (_inited) return
  _inited = true
  try {
    const core = await import('@tauri-apps/api/core')
    const event = await import('@tauri-apps/api/event')

    try {
      _layout.value = await core.invoke<LayoutInfo | null>('get_current_layout')
    } catch (e) {
      _error.value = String(e)
    }

    _unlisten = await event.listen<LayoutInfo>('layout-changed', (ev) => {
      _layout.value = ev.payload
      _error.value = null
    })
  } catch {
    // Not running inside Tauri — leave as null, no error.
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
    error: computed(() => _error.value),
  }
}
