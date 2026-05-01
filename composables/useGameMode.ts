const _status = ref<GameModeStatus>({ active: false, method: null })
let _inited = false
let _unlisten: (() => void) | null = null

export interface GameModeStatus {
  active: boolean
  method: string | null
}

async function init() {
  if (_inited) return
  _inited = true
  try {
    const tauri = await useTauri()
    if (!tauri) return
    const res = await tauri.invoke<GameModeStatus>('get_gamemode_status')
    _status.value = res

    const { listen } = await import('@tauri-apps/api/event')
    _unlisten = await listen<GameModeStatus>('game-mode-changed', (event) => {
      _status.value = event.payload
    })
  } catch (e) {
    console.error('Failed to init gamemode:', e)
  }
}

export function useGameMode() {
  void init()
  onScopeDispose(() => {
    _unlisten?.()
  })
  return {
    status: readonly(_status),
    refreshStatus: async () => {
      try {
        const tauri = await useTauri()
        if (!tauri) return
        const res = await tauri.invoke<GameModeStatus>('get_gamemode_status')
        _status.value = res
      } catch (e) {
        console.error('Failed to get gamemode status:', e)
      }
    },
  }
}

export async function resetGameModeStateForTests() {
  _inited = false
  _status.value = { active: false, method: null }
  const unlisten = _unlisten
  _unlisten = null
  if (unlisten) await unlisten()
}
