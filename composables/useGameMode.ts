const _status = ref<GameModeStatus>({ active: false, method: null, detectionEnabled: true })
let _inited = false
let _consumerCount = 0
let _unlisten: (() => void) | null = null
let _initPromise: Promise<void> | null = null

export interface GameModeStatus {
  active: boolean
  method: string | null
  detectionEnabled: boolean
}

async function init() {
  if (_inited) return
  if (_initPromise) {
    await _initPromise
    return
  }
  _initPromise = doInit()
  await _initPromise
}

async function doInit() {
  _inited = true
  try {
    const tauri = await useTauri()
    if (!tauri) {
      _inited = false
      _initPromise = null
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
      _initPromise = null
      return
    }
    _unlisten = unlisten
  } catch (e) {
    _inited = false
    _initPromise = null
    logger.error('Failed to init gamemode', e)
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
    _initPromise = null
  })
  return {
    status: readonly(_status),
    ready: () => _initPromise ?? Promise.resolve(),
    refreshStatus: async () => {
      try {
        const tauri = await useTauri()
        if (!tauri) return
        const res = await tauri.invoke<GameModeStatus>('get_gamemode_status')
        _status.value = res
      } catch (e) {
        logger.error('Failed to get gamemode status', e)
      }
    },
  }
}

export async function resetGameModeStateForTests() {
  _inited = false
  _consumerCount = 0
  _initPromise = null
  _status.value = { active: false, method: null, detectionEnabled: true }
  const unlisten = _unlisten
  _unlisten = null
  if (unlisten) await unlisten()
}
