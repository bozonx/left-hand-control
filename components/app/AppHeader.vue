<script setup lang="ts">
import AppTooltip from '~/components/shared/AppTooltip.vue'
import {
  isUserLayoutId,
  userLayoutNameFromId,
} from '~/composables/useLayoutLibrary'

const route = useRoute()

const {
  loaded,
  config,
  flush,
  currentLayoutId,
  isLayoutDirty,
} = useConfig()
const { openSaveModal, saveBusy } = useSettingsScreen()
const mapper = useMapper()
const { layout } = useLayout()
const { t } = useI18n()

const tabItems = computed(() => [
  { key: 'layouts', to: '/layouts', label: t('tabs.layouts'), icon: 'i-lucide-folder-kanban' },
  { key: 'rules', to: '/rules', label: t('tabs.rules'), icon: 'i-lucide-workflow' },
  { key: 'keymap', to: '/keymap', label: t('tabs.keymap'), icon: 'i-lucide-keyboard' },
  { key: 'macros', to: '/macros', label: t('tabs.macros'), icon: 'i-lucide-zap' },
  { key: 'commands', to: '/commands', label: t('tabs.commands'), icon: 'i-lucide-terminal' },
])

const currentLayoutLabel = computed<string>(() => {
  const id = currentLayoutId.value
  if (!id) return t('app.customLayout')
  if (isUserLayoutId(id)) return userLayoutNameFromId(id)
  return id
})

const selectedDevice = computed(() => config.value.settings.inputDevicePath ?? '')
const windowTitle = computed(() => `${t('app.title')} - ${currentLayoutLabel.value}`)
const saveLabel = computed(() => (isLayoutDirty.value ? t('common.save') : t('app.saved')))
const saveIcon = computed(() => (isLayoutDirty.value ? 'i-lucide-save' : 'i-lucide-check'))

function isActive(path: string) {
  if (path === '/layouts' && route.path === '/') return true
  return route.path === path
}

async function openTab(path: string) {
  if (isActive(path)) return
  await navigateTo(path)
}

async function toggleMapper() {
  try {
    await flush()
    if (mapper.status.value.running) {
      await mapper.stop()
      return
    }
    if (!selectedDevice.value) return
    await mapper.start(selectedDevice.value)
  } catch (error) {
    mapper.error.value = error instanceof Error ? error.message : String(error)
  }
}

async function syncWindowTitle(title: string) {
  try {
    const { getCurrentWindow } = await import('@tauri-apps/api/window')
    await getCurrentWindow().setTitle(title)
  } catch {}
}

async function quitApplication() {
  const tauri = await useTauri()
  if (!tauri) return
  await tauri.invoke('quit_application')
}

onMounted(() => {
  void mapper.refreshStatus()
})

watch(
  windowTitle,
  (title) => {
    void syncWindowTitle(title)
  },
  { immediate: true },
)
</script>

<template>
  <header
    class="flex items-center justify-between px-4 h-[var(--app-header-height)] border-b border-(--ui-border) bg-(--ui-bg-elevated) gap-3 shrink-0 app-chrome"
  >
    <div class="flex items-center gap-4 min-w-0 flex-1">
      <div class="flex items-center gap-2.5 shrink-0">
        <UIcon name="i-lucide-keyboard" class="w-5 h-5 text-primary" />
        <h1 class="text-[0.9375rem] font-semibold whitespace-nowrap">{{ $t('app.title') }}</h1>
      </div>

      <div class="min-w-0 flex-1 overflow-x-auto">
        <div class="inline-flex min-w-max items-center gap-1 bg-(--ui-bg) border border-(--ui-border) rounded-lg p-1">
          <UButton
            v-for="item in tabItems"
            :key="item.key"
            :color="isActive(item.to) ? 'primary' : 'neutral'"
            :variant="isActive(item.to) ? 'soft' : 'ghost'"
            :icon="item.icon"
            :square="item.iconOnly"
            :aria-label="item.label"
            size="sm"
            @click="openTab(item.to)"
          >
            <span v-if="!item.iconOnly">{{ item.label }}</span>
          </UButton>
        </div>
      </div>
    </div>

    <div class="flex items-center gap-3 shrink-0">
      <template v-if="loaded">
        <UButton
          :color="mapper.status.value.running ? 'error' : 'primary'"
          :variant="mapper.status.value.running ? 'soft' : 'solid'"
          :icon="mapper.status.value.running ? 'i-lucide-square' : 'i-lucide-play'"
          size="sm"
          class="whitespace-nowrap"
          :loading="mapper.busy.value"
          :disabled="!mapper.status.value.running && !selectedDevice"
          @click="toggleMapper"
        >
          {{ mapper.status.value.running ? $t('settings.stop') : $t('settings.start') }}
        </UButton>

        <AppTooltip
          :text="isLayoutDirty ? $t('app.dirtyTooltip') : currentLayoutLabel"
        >
          <div class="flex items-center gap-2">
            <UButton
              :icon="saveIcon"
              size="sm"
              :color="isLayoutDirty ? 'primary' : 'neutral'"
              :variant="isLayoutDirty ? 'solid' : 'outline'"
              :loading="saveBusy"
              :disabled="!isLayoutDirty"
              @click="openSaveModal"
            >
              {{ saveLabel }}
            </UButton>
          </div>
        </AppTooltip>

        <AppTooltip v-if="layout" :text="layout.long">
          <UBadge
            color="neutral"
            variant="outline"
            size="sm"
            class="font-mono uppercase"
          >
            <UIcon name="i-lucide-languages" class="mr-1 shrink-0" />
            <span class="text-[0.6875rem] opacity-60 mr-0.5">{{ $t('app.layoutLanguageLabel') }}</span>
            {{ layout.short }}{{ layout.display ? ` (${layout.display})` : '' }}
          </UBadge>
        </AppTooltip>

        <UButton
          color="neutral"
          variant="ghost"
          icon="i-lucide-settings"
          size="sm"
          :aria-label="$t('tabs.settings')"
          @click="openTab('/settings')"
        />

        <UButton
          color="neutral"
          variant="ghost"
          icon="i-lucide-log-out"
          size="sm"
          :aria-label="$t('app.quit')"
          @click="quitApplication"
        />
      </template>
      <div v-else class="text-xs text-(--ui-text-muted)">
        {{ $t('app.loading') }}
      </div>
    </div>
  </header>
</template>
