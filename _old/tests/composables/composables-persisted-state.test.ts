import { defineComponent } from 'vue'

import { mountSuspended } from '@nuxt/test-utils/runtime'
import { describe, expect, it, vi, beforeEach, afterEach } from 'vitest'

import { usePersistedState } from '~/composables/usePersistedState'

function harnessOf(options: Parameters<typeof usePersistedState>[0]) {
  let api: ReturnType<typeof usePersistedState>
  const Harness = defineComponent({
    setup() {
      api = usePersistedState(options)
      return {}
    },
    template: '<div />',
  })
  return { Harness, getApi: () => api! }
}

describe('usePersistedState', () => {
  beforeEach(() => {
    vi.useFakeTimers()
  })

  afterEach(() => {
    vi.useRealTimers()
  })

  it('does not schedule save when canSave returns false', async () => {
    const onSave = vi.fn().mockResolvedValue(undefined)
    const { Harness, getApi } = harnessOf({
      delayMs: 100,
      onSave,
      canSave: () => false,
    })
    await mountSuspended(Harness)
    const { scheduleSave } = getApi()

    scheduleSave()
    vi.advanceTimersByTime(200)
    expect(onSave).not.toHaveBeenCalled()
  })

  it('schedules save after delayMs', async () => {
    const onSave = vi.fn().mockResolvedValue(undefined)
    const { Harness, getApi } = harnessOf({
      delayMs: 100,
      onSave,
    })
    await mountSuspended(Harness)
    const { scheduleSave } = getApi()

    scheduleSave()
    expect(onSave).not.toHaveBeenCalled()
    vi.advanceTimersByTime(99)
    expect(onSave).not.toHaveBeenCalled()
    vi.advanceTimersByTime(1)
    expect(onSave).toHaveBeenCalledTimes(1)
  })

  it('debounces multiple scheduleSave calls', async () => {
    const onSave = vi.fn().mockResolvedValue(undefined)
    const { Harness, getApi } = harnessOf({
      delayMs: 100,
      onSave,
    })
    await mountSuspended(Harness)
    const { scheduleSave } = getApi()

    scheduleSave()
    vi.advanceTimersByTime(50)
    scheduleSave()
    vi.advanceTimersByTime(50)
    expect(onSave).not.toHaveBeenCalled()
    vi.advanceTimersByTime(50)
    expect(onSave).toHaveBeenCalledTimes(1)
  })

  it('sets saving to true while onSave runs', async () => {
    let resolveSave: () => void
    const onSave = vi.fn().mockImplementation(
      () => new Promise<void>((resolve) => { resolveSave = resolve }),
    )
    const { Harness, getApi } = harnessOf({
      delayMs: 10,
      onSave,
    })
    const _wrapper = await mountSuspended(Harness)
    const { saving, scheduleSave } = getApi()

    scheduleSave()
    vi.advanceTimersByTime(10)
    expect(saving.value).toBe(true)
    resolveSave!()
    await vi.waitFor(() => expect(saving.value).toBe(false))
    expect(onSave).toHaveBeenCalledTimes(1)
  })

  it('calls onError when onSave rejects', async () => {
    const onError = vi.fn()
    const onSave = vi.fn().mockRejectedValue(new Error('save failed'))
    const { Harness, getApi } = harnessOf({
      delayMs: 10,
      onSave,
      onError,
    })
    await mountSuspended(Harness)
    const { scheduleSave } = getApi()

    scheduleSave()
    await vi.advanceTimersByTimeAsync(10)
    expect(onError).toHaveBeenCalledWith(expect.any(Error))
  })

  it('flush triggers save immediately and clears timer', async () => {
    const onSave = vi.fn().mockResolvedValue(undefined)
    const { Harness, getApi } = harnessOf({
      delayMs: 100,
      onSave,
    })
    await mountSuspended(Harness)
    const { scheduleSave, flush } = getApi()

    scheduleSave()
    await flush()
    expect(onSave).toHaveBeenCalledTimes(1)
    vi.advanceTimersByTime(200)
    expect(onSave).toHaveBeenCalledTimes(1)
  })

  it('flush waits for in-flight save to complete', async () => {
    let resolveSave: () => void
    const onSave = vi.fn().mockImplementation(
      () => new Promise<void>((resolve) => { resolveSave = resolve }),
    )
    const { Harness, getApi } = harnessOf({
      delayMs: 10,
      onSave,
    })
    await mountSuspended(Harness)
    const { scheduleSave, flush } = getApi()

    scheduleSave()
    vi.advanceTimersByTime(10)
    const secondFlush = flush()
    resolveSave!()
    await secondFlush
    expect(onSave).toHaveBeenCalledTimes(1)
  })

  it('persistNow bypasses timer and saves immediately', async () => {
    const onSave = vi.fn().mockResolvedValue(undefined)
    const { Harness, getApi } = harnessOf({
      delayMs: 100,
      onSave,
    })
    await mountSuspended(Harness)
    const { persistNow } = getApi()

    await persistNow()
    expect(onSave).toHaveBeenCalledTimes(1)
  })

  it('persistNow rejects when onSave rejects', async () => {
    const onSave = vi.fn().mockRejectedValue(new Error('boom'))
    const { Harness, getApi } = harnessOf({
      delayMs: 100,
      onSave,
    })
    await mountSuspended(Harness)
    const { persistNow } = getApi()

    await expect(persistNow()).rejects.toThrow('boom')
  })

  it('flush resolves immediately when no timer or save in progress', async () => {
    const onSave = vi.fn().mockResolvedValue(undefined)
    const { Harness, getApi } = harnessOf({
      delayMs: 100,
      onSave,
    })
    await mountSuspended(Harness)
    const { flush } = getApi()

    await flush()
    expect(onSave).not.toHaveBeenCalled()
  })
})
