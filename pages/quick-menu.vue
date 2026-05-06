<script setup lang="ts">
import { listen } from '@tauri-apps/api/event'
import { invoke } from '@tauri-apps/api/core'
import { useConfig } from '~/composables/useConfig'
import type { QuickAction } from '~/types/config'

const { config, load } = useConfig()

const actions = computed(() => config.value.quickActions || [])
const runnableActions = computed(() => actions.value.filter((action): action is QuickAction => !!action.action.trim()))
let unlistenShow: (() => void) | null = null

function wait(ms: number) {
  return new Promise(resolve => window.setTimeout(resolve, ms))
}

async function closeMenu() {
  await invoke('hide_quick_menu').catch((e) => {
    console.error('Failed to hide quick menu:', e)
  })
}

function onKeydown(e: KeyboardEvent) {
  if (e.key === 'Escape') {
    void closeMenu()
  }
}

onMounted(async () => {
  await load()

  unlistenShow = await listen('show_quick_menu', async () => {
    await load()
  })

  window.addEventListener('keydown', onKeydown)
})

onBeforeUnmount(() => {
  window.removeEventListener('keydown', onKeydown)
  unlistenShow?.()
})

async function runAction(action: string) {
  await closeMenu()
  await wait(80)
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
      class="flex max-h-[90vh] w-full max-w-[420px] flex-col rounded-lg border border-(--ui-border) bg-(--ui-bg-elevated)/95 p-2 shadow-2xl backdrop-blur-md"
    >
      <div class="min-h-0 overflow-y-auto custom-scrollbar">
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

      <div class="mt-2 border-t border-(--ui-border-muted) pt-2">
        <button
          type="button"
          class="flex h-10 w-full items-center justify-center rounded-md px-3 text-sm font-medium text-(--ui-text-muted) transition-colors hover:bg-(--ui-bg-accented) hover:text-(--ui-text)"
          @click="closeMenu"
        >
          {{ $t('common.cancel') }}
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
