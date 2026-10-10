const _status = ref<GameModeStatus>({ active: false, method: null, detectionEnabled: false, stateAvailable: false, control: 'auto' })
let _inited = false
let _consumerCount = 0
let _unlisten: (() => void) | null = null
let _initPromise: Promise<void> | null = null
let _generation = 0

export interface GameModeStatus {
  active: boolean
  method: string | null
  detectionEnabled: boolean
  stateAvailable?: boolean
  control?: 'auto' | 'on' | 'off'
  revision?: number
}

function acceptStatus(status: GameModeStatus) {
  if ((status.revision ?? 0) < (_status.value.revision ?? 0)) return
  _status.value = status
}

async function init() {
  if (_inited) return
  if (_initPromise) {
    await _initPromise
    return
  }
  _initPromise = doInit(++_generation)
  await _initPromise
}

async function doInit(generation: number) {
  _inited = true
  try {
    const tauri = await useTauri()
    if (generation !== _generation) return
    if (!tauri) {
      _inited = false
      _initPromise = null
      return
    }
    const { listen } = await import('@tauri-apps/api/event')
    const unlisten = await listen<GameModeStatus>('game-mode-changed', (event) => {
      if (generation === _generation) acceptStatus(event.payload)
    })
    if (generation !== _generation || _consumerCount === 0) {
      unlisten()
      return
    }
    _unlisten = unlisten
    const res = await tauri.invoke<GameModeStatus>('get_gamemode_status')
    if (generation === _generation) acceptStatus(res)
  } catch (e) {
    if (generation !== _generation) return
    _unlisten?.()
    _unlisten = null
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
    _generation += 1
    _unlisten?.()
    _unlisten = null
    _inited = false
    _initPromise = null
  })
  return {
    status: readonly(_status),
    ready: () => _initPromise ?? Promise.resolve(),
    refreshStatus: async () => {
      const generation = _generation
      try {
        const tauri = await useTauri()
        if (!tauri) return
        const res = await tauri.invoke<GameModeStatus>('get_gamemode_status')
        if (generation === _generation) acceptStatus(res)
      } catch (e) {
        logger.error('Failed to get gamemode status', e)
      }
    },
  }
}

export async function resetGameModeStateForTests() {
  _generation += 1
  _inited = false
  _consumerCount = 0
  _initPromise = null
  _status.value = { active: false, method: null, detectionEnabled: false, stateAvailable: false, control: 'auto' }
  const unlisten = _unlisten
  _unlisten = null
  if (unlisten) await unlisten()
}
