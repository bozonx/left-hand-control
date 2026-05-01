export interface PersistedStateOptions {
  delayMs: number
  onSave: () => Promise<void>
  onError?: (error: unknown) => void
  canSave?: () => boolean
}

export function usePersistedState(options: PersistedStateOptions) {
  const saving = ref(false)
  let saveTimer: ReturnType<typeof setTimeout> | null = null
  let pendingFlushWaiters: Array<{
    resolve: () => void
    reject: (error: unknown) => void
  }> = []

  async function persistNow() {
    saving.value = true
    try {
      await options.onSave()
      const waiters = pendingFlushWaiters
      pendingFlushWaiters = []
      for (const waiter of waiters) waiter.resolve()
    } catch (error) {
      const waiters = pendingFlushWaiters
      pendingFlushWaiters = []
      for (const waiter of waiters) waiter.reject(error)
      options.onError?.(error)
      throw error
    } finally {
      saving.value = false
    }
  }

  function scheduleSave() {
    if (options.canSave && !options.canSave()) return
    if (saveTimer) clearTimeout(saveTimer)
    saveTimer = setTimeout(() => {
      saveTimer = null
      void persistNow().catch(() => {})
    }, options.delayMs)
  }

  async function flush() {
    if (saveTimer) {
      clearTimeout(saveTimer)
      saveTimer = null
      await persistNow()
      return
    }
    if (saving.value) {
      await new Promise<void>((resolve, reject) => {
        pendingFlushWaiters.push({ resolve, reject })
      })
    }
  }

  return { saving, scheduleSave, flush, persistNow }
}
