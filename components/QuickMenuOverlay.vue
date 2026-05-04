<script setup lang="ts">
import type { UnlistenFn } from '@tauri-apps/api/event'

const { config } = useConfig()

const visible = ref(false)
const appWindow = shallowRef<Awaited<ReturnType<typeof getAppWindow>> | null>(null)
const shouldHideWindowOnClose = ref(false)
let unlistenShow: UnlistenFn | null = null
let unlistenFocus: UnlistenFn | null = null

const actions = computed(() => config.value.quickActions || [])

async function getAppWindow() {
  const mod = await import('@tauri-apps/api/window')
  return mod.getCurrentWindow()
}

async function showMenu() {
  const win = appWindow.value
  if (win) {
    shouldHideWindowOnClose.value = !(await win.isVisible().catch(() => true))
    await win.show().catch(() => undefined)
    await win.setFocus().catch(() => undefined)
  } else {
    shouldHideWindowOnClose.value = false
  }
  visible.value = true
}

async function hideMenu() {
  visible.value = false
  const win = appWindow.value
  if (win && shouldHideWindowOnClose.value) {
    shouldHideWindowOnClose.value = false
    await win.hide().catch(() => undefined)
  }
}

async function runAction(action: string) {
  try {
    const tauri = await useTauri()
    if (tauri && action.trim()) {
      await tauri.invoke('execute_action', { action })
    }
  } catch (e) {
    console.error('Failed to execute quick action:', e)
  } finally {
    await hideMenu()
  }
}

function onKeydown(e: KeyboardEvent) {
  if (!visible.value) return
  if (e.key === 'Escape') {
    e.preventDefault()
    void hideMenu()
  }
}

onMounted(async () => {
  window.addEventListener('keydown', onKeydown)
  try {
    const events = await import('@tauri-apps/api/event')
    appWindow.value = await getAppWindow()
    unlistenShow = await events.listen('show_quick_menu', () => {
      void showMenu()
    })
    unlistenFocus = await appWindow.value.onFocusChanged(({ isFocused }) => {
      if (!isFocused && visible.value) {
        void hideMenu()
      }
    })
  } catch {
    // Running outside Tauri: keep the overlay inert.
  }
})

onBeforeUnmount(() => {
  window.removeEventListener('keydown', onKeydown)
  unlistenShow?.()
  unlistenFocus?.()
})
</script>

<template>
  <Teleport to="body">
    <div
      v-if="visible"
      class="fixed inset-0 z-[1000] flex items-center justify-center bg-black/20 p-4 backdrop-blur-sm"
      @mousedown.self="hideMenu"
    >
      <div
        class="max-h-[90vh] min-w-[300px] max-w-[600px] overflow-y-auto rounded-xl border border-(--ui-border) bg-(--ui-bg-elevated)/95 p-6 shadow-2xl"
      >
        <div v-if="actions.length === 0" class="p-8 text-center">
          <UIcon name="i-lucide-zap-off" class="mx-auto mb-4 h-12 w-12 text-(--ui-text-muted) opacity-50" />
          <p class="text-(--ui-text-muted)">{{ $t('quickActions.empty') }}</p>
        </div>

        <div v-else class="grid grid-cols-2 gap-3 sm:grid-cols-3">
          <button
            v-for="action in actions"
            :key="action.id"
            type="button"
            class="group flex min-h-24 flex-col items-center justify-center gap-2 rounded-lg border border-(--ui-border-muted) bg-(--ui-bg) p-4 transition hover:border-primary hover:bg-primary/10 disabled:cursor-not-allowed disabled:opacity-50"
            :disabled="!action.action"
            @click="runAction(action.action)"
          >
            <UIcon
              :name="action.icon || 'i-lucide-zap'"
              class="h-8 w-8 text-(--ui-text-muted) transition-colors group-hover:text-primary"
            />
            <span class="w-full truncate px-1 text-center text-xs font-semibold">
              {{ action.name }}
            </span>
          </button>
        </div>
      </div>
    </div>
  </Teleport>
</template>
