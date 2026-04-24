import { ref, onMounted, onUnmounted } from 'vue'

export interface GameModeStatus {
  active: boolean
  method: string | null
}

export function useGameMode() {
  const status = useState<GameModeStatus>('gamemode-status', () => ({
    active: false,
    method: null
  }))

  const refreshStatus = async () => {
    try {
      const tauri = await useTauri()
      if (!tauri) return
      const res = await tauri.invoke<GameModeStatus>('get_gamemode_status')
      status.value = res
    } catch (e) {
      console.error('Failed to get gamemode status:', e)
    }
  }

  let unlisten: (() => void) | null = null

  onMounted(async () => {
    await refreshStatus()
    
    try {
      const tauri = await useTauri()
      if (!tauri) return
      const { listen } = await import('@tauri-apps/api/event')
      unlisten = await listen<GameModeStatus>('game-mode-changed', (event) => {
        status.value = event.payload
      })
    } catch (e) {
      console.error('Failed to listen for gamemode events:', e)
    }
  })

  onUnmounted(() => {
    if (unlisten) unlisten()
  })

  return {
    status,
    refreshStatus
  }
}
