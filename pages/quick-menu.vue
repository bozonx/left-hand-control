<script setup lang="ts">
import { listen } from '@tauri-apps/api/event'
import { getCurrentWindow } from '@tauri-apps/api/window'
import { invoke } from '@tauri-apps/api/core'
import { useConfig } from '~/composables/useConfig'
import type { QuickAction } from '~/types/config'

const appWindow = getCurrentWindow()
const { config, load } = useConfig()

const actions = computed(() => config.value.quickActions || [])
const runnableActions = computed(() => actions.value.filter((action): action is QuickAction => !!action.action.trim()))
let unlistenShow: (() => void) | null = null
let unlistenFocus: (() => void) | null = null

function onKeydown(e: KeyboardEvent) {
  if (e.key === 'Escape') {
    void appWindow.hide()
  }
}

onMounted(async () => {
  await load()

  unlistenShow = await listen('show_quick_menu', async () => {
    await load()
    await appWindow.center()
    await appWindow.show()
    await appWindow.setFocus()
  })

  window.addEventListener('keydown', onKeydown)

  unlistenFocus = await appWindow.onFocusChanged(({ isFocused }) => {
    if (!isFocused) {
      void appWindow.hide()
    }
  })
})

onBeforeUnmount(() => {
  window.removeEventListener('keydown', onKeydown)
  unlistenShow?.()
  unlistenFocus?.()
})

async function runAction(action: string) {
  await appWindow.hide()
  try {
    await invoke('execute_action', { action })
  } catch (e) {
    console.error('Failed to execute action:', e)
  }
}
</script>

<template>
  <div class="flex h-screen w-screen select-none items-center justify-center overflow-hidden bg-transparent p-4">
    <div
      class="max-h-[90vh] w-full max-w-[420px] overflow-y-auto rounded-lg border border-(--ui-border) bg-(--ui-bg-elevated)/95 p-2 shadow-2xl backdrop-blur-md custom-scrollbar"
    >
      <div v-if="runnableActions.length === 0" class="p-8 text-center">
        <p class="text-(--ui-text-muted)">{{ $t('quickActions.empty') }}</p>
      </div>

      <div v-else class="flex flex-col gap-1">
        <button
          v-for="action in runnableActions"
          :key="action.id"
          type="button"
          class="flex min-h-11 w-full items-center gap-3 rounded-md px-3 py-2 text-left transition-colors hover:bg-(--ui-bg-accented)"
          @click="runAction(action.action)"
        >
          <UIcon name="i-lucide-zap" class="h-4 w-4 shrink-0 text-(--ui-text-muted)" />
          <span class="min-w-0 flex-1 truncate text-sm font-medium">
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
