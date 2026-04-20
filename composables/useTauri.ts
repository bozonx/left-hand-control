type TauriCore = typeof import('@tauri-apps/api/core')

let tauriCache: TauriCore | null | undefined

export async function useTauri(): Promise<TauriCore | null> {
  if (tauriCache !== undefined) return tauriCache
  try {
    tauriCache = await import('@tauri-apps/api/core')
  } catch {
    tauriCache = null
  }
  return tauriCache
}
