const _status = ref<GameModeStatus>({ active: false, method: null, detectionEnabled: true })
let _inited = false
let _consumerCount = 0
let _unlisten: (() => void) | null = null

export interface GameModeStatus {
  active: boolean
  method: string | null
  detectionEnabled: boolean
}

async function init() {
  if (_inited) return
  _inited = true
  try {
    const tauri = await useTauri()
    if (!tauri) {
      _inited = false
      return
    }
    const res = await tauri.invoke<GameModeStatus>('get_gamemode_status')
    _status.value = res

    const { listen } = await import('@tauri-apps/api/event')
    const unlisten = await listen<GameModeStatus>('game-mode-changed', (event) => {
      _status.value = event.payload
    })
    if (_consumerCount === 0) {
      unlisten()
      _inited = false
      return
    }
    _unlisten = unlisten
  } catch (e) {
    _inited = false
    console.error('Failed to init gamemode:', e)
  }
}

export function useGameMode() {
  _consumerCount += 1
  void init()
  onScopeDispose(() => {
    _consumerCount = Math.max(0, _consumerCount - 1)
    if (_consumerCount > 0) return
    _unlisten?.()
    _unlisten = null
    _inited = false
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
  _consumerCount = 0
  _status.value = { active: false, method: null, detectionEnabled: true }
  const unlisten = _unlisten
  _unlisten = null
  if (unlisten) await unlisten()
}
