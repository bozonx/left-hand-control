<script setup lang="ts">
import { listen } from '@tauri-apps/api/event'
import { invoke } from '@tauri-apps/api/core'
import {
  EMOJI_HOTKEYS,
  createDefaultEmojiPage,
  textActionRef,
} from '~/types/config'
import type { EmojiHotkey } from '~/types/config'

const { config, load } = useConfig()

const pageIndex = ref(0)
const pages = computed(() => {
  const configured = config.value.emojiPages || []
  return configured.length > 0 ? configured : [createDefaultEmojiPage()]
})
const page = computed(() => pages.value[pageIndex.value] ?? pages.value[0] ?? createDefaultEmojiPage())

let unlistenShow: (() => void) | null = null

function wait(ms: number) {
  return new Promise(resolve => window.setTimeout(resolve, ms))
}

async function closeMenu() {
  await invoke('hide_emoji_menu').catch((e) => {
    logger.error('Failed to hide emoji menu', e)
  })
}

function nextPage() {
  pageIndex.value = (pageIndex.value + 1) % Math.max(1, pages.value.length)
}

async function applyEmoji(emoji: string | undefined) {
  if (!emoji) return
  await closeMenu()
  await wait(80)
  try {
    await invoke('execute_action', { action: textActionRef(emoji) })
  } catch (e) {
    logger.error('Failed to insert emoji', e)
  }
}

function onKeydown(e: KeyboardEvent) {
  if (e.key === 'Escape') {
    e.preventDefault()
    void closeMenu()
    return
  }
  if (e.key === 'Tab') {
    e.preventDefault()
    nextPage()
    return
  }
  const key = e.key.toLowerCase()
  if ((EMOJI_HOTKEYS as readonly string[]).includes(key)) {
    e.preventDefault()
    void applyEmoji(page.value.cells[key as EmojiHotkey])
  }
}

onMounted(async () => {
  await load()

  unlistenShow = await listen('show_emoji_menu', async () => {
    await load()
    pageIndex.value = 0
  })

  window.addEventListener('keydown', onKeydown)
})

onBeforeUnmount(() => {
  window.removeEventListener('keydown', onKeydown)
  unlistenShow?.()
})
</script>

<template>
  <div class="flex h-screen w-screen select-none items-center justify-center overflow-hidden bg-transparent p-4">
    <div
      class="flex w-full max-w-[520px] flex-col rounded-lg border border-(--ui-border) bg-(--ui-bg-elevated)/95 p-3 shadow-2xl backdrop-blur-md"
    >
      <div class="mb-3 flex items-center justify-between gap-3">
        <div class="min-w-0">
          <p class="truncate text-sm font-semibold">{{ page.name }}</p>
          <p class="text-xs text-(--ui-text-muted)">
            {{ $t('emojiMenu.tabHint') }}
          </p>
        </div>
        <UBadge color="neutral" variant="outline" size="sm">
          {{ pageIndex + 1 }} / {{ pages.length }}
        </UBadge>
      </div>

      <div class="grid grid-cols-5 gap-2">
        <button
          v-for="key in EMOJI_HOTKEYS"
          :key="key"
          type="button"
          class="flex aspect-square min-h-20 flex-col items-center justify-center gap-2 rounded-md border border-(--ui-border-muted) bg-(--ui-bg) p-2 transition hover:border-primary hover:bg-primary/10 disabled:cursor-default disabled:opacity-40"
          :disabled="!page.cells[key]"
          @click="applyEmoji(page.cells[key])"
        >
          <span class="text-3xl leading-none">{{ page.cells[key] || ' ' }}</span>
          <span class="font-mono text-xs uppercase text-(--ui-text-muted)">{{ key }}</span>
        </button>
      </div>

      <div class="mt-3 border-t border-(--ui-border-muted) pt-2">
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
