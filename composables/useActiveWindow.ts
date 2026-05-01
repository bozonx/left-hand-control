// Reactive state for the currently focused window's title and app id.
//
// Source of truth is the Rust-side `active_window` watcher. We seed the
// state via `get_active_window` and then update on the
// `active-window-changed` event. In a plain browser environment (`pnpm
// dev` without Tauri) this returns `null` values and never updates.

export interface ActiveWindow {
  title: string
  appId: string
}

const _state = ref<ActiveWindow | null>(null)
let _inited = false
let _unlisten: (() => void) | null = null

async function init() {
  if (_inited) return
  _inited = true
  try {
    const tauri = await useTauri()
    if (!tauri) return
    const res = await tauri.invoke<ActiveWindow | null>('get_active_window')
    _state.value = res ?? null

    const { listen } = await import('@tauri-apps/api/event')
    _unlisten = await listen<ActiveWindow>('active-window-changed', (event) => {
      const payload = event.payload
      // Empty payload (no title and no appId) means "no detection".
      if (!payload || (!payload.title && !payload.appId)) {
        _state.value = null
      } else {
        _state.value = payload
      }
    })
  } catch (e) {
    console.error('Failed to init active window:', e)
  }
}

export function useActiveWindow() {
  void init()
  onScopeDispose(() => {
    _unlisten?.()
  })
  return {
    state: readonly(_state),
    refresh: async () => {
      try {
        const tauri = await useTauri()
        if (!tauri) return
        const res = await tauri.invoke<ActiveWindow | null>('get_active_window')
        _state.value = res ?? null
      } catch (e) {
        console.error('Failed to get active window:', e)
      }
    },
  }
}

export async function resetActiveWindowStateForTests() {
  _inited = false
  _state.value = null
  const unlisten = _unlisten
  _unlisten = null
  if (unlisten) await unlisten()
}
