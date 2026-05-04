<script setup lang="ts">
import { listen } from '@tauri-apps/api/event'
import { getCurrentWindow } from '@tauri-apps/api/window'
import { invoke } from '@tauri-apps/api/core'
import { useConfig } from '~/composables/useConfig'

const appWindow = getCurrentWindow()
const { config, load } = useConfig()

const actions = computed(() => config.value.quickActions || [])

onMounted(async () => {
  // Ensure config is loaded in this window too
  await load()

  // Listen for the show event from Rust mapper
  await listen('show_quick_menu', async () => {
    // Refresh config to get latest actions
    await load()
    await appWindow.show()
    await appWindow.setFocus()
  })

  // Hide on Escape
  window.addEventListener('keydown', (e) => {
    if (e.key === 'Escape') {
      void appWindow.hide()
    }
  })

  // Hide on focus loss
  await appWindow.onFocusChanged(({ isFocused }) => {
    if (!isFocused) {
      void appWindow.hide()
    }
  })
})

async function runAction(action: string) {
  try {
    await invoke('execute_action', { action })
  } catch (e) {
    console.error('Failed to execute action:', e)
  } finally {
    await appWindow.hide()
  }
}
</script>

<template>
  <div class="h-screen w-screen flex items-center justify-center bg-transparent overflow-hidden select-none p-4">
    <div 
      class="bg-(--ui-bg-elevated)/90 backdrop-blur-md border border-(--ui-border) rounded-2xl shadow-2xl p-6 min-w-[300px] max-w-[600px] max-h-[90vh] overflow-y-auto custom-scrollbar"
    >
      <div v-if="actions.length === 0" class="text-center p-8">
        <UIcon name="i-lucide-zap-off" class="w-12 h-12 text-(--ui-text-muted) mb-4 opacity-50 mx-auto" />
        <p class="text-(--ui-text-muted)">{{ $t('quickActions.empty') }}</p>
      </div>

      <div v-else class="grid grid-cols-2 sm:grid-cols-3 gap-3">
        <button
          v-for="action in actions"
          :key="action.id"
          class="flex flex-col items-center justify-center p-4 rounded-xl bg-(--ui-bg) hover:bg-primary/10 border border-(--ui-border-muted) hover:border-primary transition-all group gap-2"
          @click="runAction(action.action)"
        >
          <UIcon 
            :name="action.icon || 'i-lucide-zap'" 
            class="w-8 h-8 text-(--ui-text-muted) group-hover:text-primary transition-colors"
          />
          <span class="text-xs font-semibold text-center truncate w-full px-1">
            {{ action.name }}
          </span>
        </button>
      </div>
    </div>
  </div>
</template>

<style scoped>
.custom-scrollbar::-webkit-scrollbar {
  width: 4px;
}
.custom-scrollbar::-webkit-scrollbar-track {
  background: transparent;
}
.custom-scrollbar::-webkit-scrollbar-thumb {
  background: var(--ui-border);
  border-radius: 10px;
}
</style>