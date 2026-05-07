<script setup lang="ts">
import { listen } from '@tauri-apps/api/event'
import { invoke } from '@tauri-apps/api/core'
import { useConfig } from '~/composables/useConfig'
import {
  LEFT_HAND_HOTKEYS,
  LEFT_HAND_HOTKEY_LABELS,
  type LeftHandHotkey,
  type QuickAction,
} from '~/types/config'

const { config, load } = useConfig()

const actions = computed(() => config.value.quickActions || [])
const runnableActions = computed(() => actions.value.filter((action): action is QuickAction => !!action.action.trim()))
const pageIndex = ref(0)
const pages = computed(() => {
  const chunks: QuickAction[][] = []
  for (let i = 0; i < runnableActions.value.length; i += LEFT_HAND_HOTKEYS.length) {
    chunks.push(runnableActions.value.slice(i, i + LEFT_HAND_HOTKEYS.length))
  }
  return chunks
})
const page = computed(() => pages.value[pageIndex.value] ?? pages.value[0] ?? [])
let unlistenShow: (() => void) | null = null

function wait(ms: number) {
  return new Promise(resolve => window.setTimeout(resolve, ms))
}

async function closeMenu() {
  await invoke('hide_quick_menu').catch((e) => {
    logger.error('Failed to hide quick menu', e)
  })
}

function nextPage() {
  pageIndex.value = (pageIndex.value + 1) % Math.max(1, pages.value.length)
}

function prevPage() {
  pageIndex.value =
    (pageIndex.value - 1 + Math.max(1, pages.value.length)) %
    Math.max(1, pages.value.length)
}

function onKeydown(e: KeyboardEvent) {
  if (e.key === 'Escape') {
    e.preventDefault()
    void closeMenu()
    return
  }
  if (e.key === 'Tab') {
    e.preventDefault()
    if (e.shiftKey) {
      prevPage()
    } else {
      nextPage()
    }
    return
  }
  const hotkeyIndex = (LEFT_HAND_HOTKEYS as readonly string[]).indexOf(e.code)
  if (hotkeyIndex !== -1) {
    e.preventDefault()
    const action = page.value[hotkeyIndex]
    if (action) {
      void runAction(action.action)
    }
  }
}

onMounted(async () => {
  await load()

  unlistenShow = await listen('show_quick_menu', async () => {
    await load()
    pageIndex.value = 0
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
    logger.error('Failed to execute action', e)
  }
}
</script>

<template>
  <div class="flex h-screen w-screen select-none items-center justify-center overflow-hidden bg-transparent p-4">
    <div
      class="flex w-full max-w-[720px] flex-col rounded-lg border border-(--ui-border) bg-(--ui-bg-elevated)/95 p-3 shadow-2xl backdrop-blur-md"
    >
      <div class="mb-3 flex items-center justify-between gap-3">
        <div class="min-w-0">
          <p class="truncate text-sm font-semibold">
            {{ $t('quickActions.title') }}
          </p>
          <p class="text-xs text-(--ui-text-muted)">
            {{ $t('quickActions.menuHint') }}
          </p>
        </div>
        <UBadge color="neutral" variant="outline" size="sm">
          {{ pageIndex + 1 }} / {{ Math.max(1, pages.length) }}
        </UBadge>
      </div>

      <div v-if="runnableActions.length === 0" class="p-8 text-center">
        <p class="text-(--ui-text-muted)">{{ $t('quickActions.empty') }}</p>
      </div>

      <div v-else class="grid grid-cols-5 gap-2">
        <button
          v-for="(key, index) in LEFT_HAND_HOTKEYS"
          :key="key"
          type="button"
          class="flex h-24 min-w-0 flex-col items-start justify-between gap-2 rounded-md border border-(--ui-border-muted) bg-(--ui-bg) p-3 text-left transition hover:border-primary hover:bg-primary/10 disabled:cursor-default disabled:opacity-40"
          :disabled="!page[index]"
          @click="page[index] && runAction(page[index].action)"
        >
          <span class="flex min-w-0 items-center gap-2 self-stretch">
            <UIcon
              :name="page[index]?.icon || 'i-lucide-zap'"
              class="h-4 w-4 shrink-0 text-(--ui-text-muted)"
            />
            <span class="quick-menu-action-name truncate text-sm font-medium">
              {{ page[index]?.name || ' ' }}
            </span>
          </span>
          <span class="font-mono text-xs uppercase text-(--ui-text-muted)">
            {{ LEFT_HAND_HOTKEY_LABELS[key as LeftHandHotkey] }}
          </span>
        </button>
      </div>

      <div v-if="pages.length > 1" class="mt-2 flex items-center justify-center gap-1.5">
        <button
          v-for="(_, index) in pages"
          :key="index"
          type="button"
          class="h-1.5 rounded-full transition-all"
          :class="
            index === pageIndex
              ? 'w-4 bg-primary'
              : 'w-1.5 bg-(--ui-border-accented) hover:bg-(--ui-text-muted)'
          "
          @click="pageIndex = index"
        />
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
.quick-menu-action-name {
  min-width: 0;
}
</style>
