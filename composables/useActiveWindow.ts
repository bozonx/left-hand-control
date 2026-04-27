// Reactive state for the currently focused window's title and app id.
//
// Source of truth is the Rust-side `active_window` watcher. We seed the
// state via `get_active_window` and then update on the
// `active-window-changed` event. In a plain browser environment (`pnpm
// dev` without Tauri) this returns `null` values and never updates.

import { onMounted, onUnmounted } from 'vue'

export interface ActiveWindow {
  title: string
  appId: string
}

export function useActiveWindow() {
  const state = useState<ActiveWindow | null>('active-window', () => null)

  async function refresh() {
    try {
      const tauri = await useTauri()
      if (!tauri) return
      const res = await tauri.invoke<ActiveWindow | null>('get_active_window')
      state.value = res ?? null
    } catch (e) {
      console.error('Failed to get active window:', e)
    }
  }

  let unlisten: (() => void) | null = null

  onMounted(async () => {
    await refresh()
    try {
      const tauri = await useTauri()
      if (!tauri) return
      const { listen } = await import('@tauri-apps/api/event')
      unlisten = await listen<ActiveWindow>('active-window-changed', (event) => {
        const payload = event.payload
        // Empty payload (no title and no appId) means "no detection".
        if (!payload || (!payload.title && !payload.appId)) {
          state.value = null
        } else {
          state.value = payload
        }
      })
    } catch (e) {
      console.error('Failed to listen for active window events:', e)
    }
  })

  onUnmounted(() => {
    if (unlisten) unlisten()
  })

  return {
    state,
    refresh,
  }
}
