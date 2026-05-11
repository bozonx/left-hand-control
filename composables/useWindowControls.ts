// Custom title-bar window controls.
//
// The app runs with native window decorations disabled (`decorations: false`),
// so it draws its own caption buttons. The button set + side is read from the
// desktop environment by the Rust `get_window_controls_layout` command (see
// `src-tauri/src/window_controls.rs`); in a plain browser (`pnpm dev`) there
// is no window to control, so everything degrades to no-ops and the controls
// are hidden.

export type WindowButton = 'minimize' | 'maximize' | 'close'

// Mirrors `@tauri-apps/api/window`'s internal (non-exported) `ResizeDirection`
// union; a structurally identical literal union is assignable to it.
export type WindowResizeDirection =
  | 'North'
  | 'South'
  | 'East'
  | 'West'
  | 'NorthEast'
  | 'NorthWest'
  | 'SouthEast'
  | 'SouthWest'

export interface WindowControlsLayout {
  left: WindowButton[]
  right: WindowButton[]
  source: string
}

const DEFAULT_LAYOUT: WindowControlsLayout = {
  left: [],
  right: ['minimize', 'maximize', 'close'],
  source: 'default',
}

const _layout = ref<WindowControlsLayout>(DEFAULT_LAYOUT)
const _maximized = ref(false)
// True only inside the Tauri shell — gates rendering of the custom controls.
const _available = ref(false)
let _inited = false

type TauriWindowApi = typeof import('@tauri-apps/api/window')
let _windowApi: TauriWindowApi | null | undefined

async function windowApi(): Promise<TauriWindowApi | null> {
  if (_windowApi !== undefined) return _windowApi
  const tauri = await useTauri()
  if (!tauri) {
    _windowApi = null
    return _windowApi
  }
  try {
    _windowApi = await import('@tauri-apps/api/window')
  } catch {
    _windowApi = null
  }
  return _windowApi
}

async function init() {
  if (_inited) return
  _inited = true

  const tauri = await useTauri()
  if (!tauri) return
  _available.value = true

  try {
    _layout.value = await tauri.invoke<WindowControlsLayout>(
      'get_window_controls_layout',
    )
  } catch {
    // Keep the default layout — the controls still work.
  }

  const api = await windowApi()
  if (!api) return
  const win = api.getCurrentWindow()
  try {
    _maximized.value = await win.isMaximized()
  } catch {
    /* ignore */
  }
  // Maximize / unmaximize surface as resize events; refresh the icon then.
  try {
    await win.onResized(() => {
      void win
        .isMaximized()
        .then((v) => {
          _maximized.value = v
        })
        .catch(() => {})
    })
  } catch {
    /* ignore */
  }
}

export function useWindowControls() {
  if (import.meta.client) void init()

  async function minimize() {
    const api = await windowApi()
    await api?.getCurrentWindow().minimize()
  }

  async function toggleMaximize() {
    const api = await windowApi()
    await api?.getCurrentWindow().toggleMaximize()
  }

  // "Close" sends the app to the tray and drops it from the taskbar, matching
  // the close-to-tray behaviour wired up in the Rust side.
  async function closeToTray() {
    const tauri = await useTauri()
    await tauri?.invoke('hide_main_window_command')
  }

  async function startResizeDragging(direction: WindowResizeDirection) {
    const api = await windowApi()
    await api?.getCurrentWindow().startResizeDragging(direction)
  }

  return {
    layout: readonly(_layout),
    isMaximized: readonly(_maximized),
    available: readonly(_available),
    minimize,
    toggleMaximize,
    closeToTray,
    startResizeDragging,
  }
}
