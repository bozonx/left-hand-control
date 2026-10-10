import { effectScope, onScopeDispose, readonly, ref, type EffectScope } from 'vue'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import type { GameModeStatus } from '~/composables/useGameMode'

const { invokeMock, listenMock, useTauriMock } = vi.hoisted(() => ({
  invokeMock: vi.fn(), listenMock: vi.fn(), useTauriMock: vi.fn(),
}))

vi.mock('@tauri-apps/api/event', () => ({ listen: listenMock }))

function status(revision: number, active: boolean): GameModeStatus {
  return { revision, active, method: active ? 'manual' : null, detectionEnabled: false, stateAvailable: true, control: active ? 'on' : 'off' }
}

describe('game mode status delivery', () => {
  let scope: EffectScope
  let bridge: typeof import('~/composables/useGameMode')

  beforeEach(async () => {
    vi.resetModules()
    invokeMock.mockReset()
    listenMock.mockReset()
    useTauriMock.mockReset().mockResolvedValue({ invoke: invokeMock })
    vi.stubGlobal('ref', ref)
    vi.stubGlobal('readonly', readonly)
    vi.stubGlobal('onScopeDispose', onScopeDispose)
    vi.stubGlobal('useTauri', useTauriMock)
    vi.stubGlobal('logger', { error: vi.fn() })
    bridge = await import('~/composables/useGameMode')
    scope = effectScope()
  })

  afterEach(() => { scope.stop() })

  it('keeps a manual change when an older initial snapshot or event arrives later', async () => {
    let receive: (event: { payload: GameModeStatus }) => void = () => {}
    let resolveSnapshot: (value: GameModeStatus) => void = () => {}
    invokeMock.mockReturnValue(new Promise<GameModeStatus>((resolve) => { resolveSnapshot = resolve }))
    listenMock.mockImplementation(async (_event, callback) => {
      receive = callback
      return vi.fn()
    })
    const api = scope.run(bridge.useGameMode)!
    await vi.waitFor(() => expect(invokeMock).toHaveBeenCalledWith('get_gamemode_status'))
    receive({ payload: status(3, true) })
    resolveSnapshot(status(2, false))
    await api.ready()
    expect(api.status.value).toEqual(status(3, true))
    receive({ payload: status(1, false) })
    expect(api.status.value.active).toBe(true)
    receive({ payload: status(4, false) })
    expect(api.status.value).toEqual(status(4, false))
  })

  it('ignores snapshots after the last consumer has gone away', async () => {
    let resolveSnapshot: (value: GameModeStatus) => void = () => {}
    const unlisten = vi.fn()
    invokeMock.mockReturnValue(new Promise<GameModeStatus>((resolve) => { resolveSnapshot = resolve }))
    listenMock.mockResolvedValue(unlisten)
    const api = scope.run(bridge.useGameMode)!
    await vi.waitFor(() => expect(invokeMock).toHaveBeenCalled())
    const ready = api.ready()
    scope.stop()
    resolveSnapshot(status(1, true))
    await ready
    expect(unlisten).toHaveBeenCalledOnce()
    expect(api.status.value.active).toBe(false)
  })
})
