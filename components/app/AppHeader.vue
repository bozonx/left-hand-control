<script setup lang="ts">
import AppTooltip from '~/components/shared/AppTooltip.vue'
import {
  isUserLayoutId,
  userLayoutNameFromId,
} from '~/composables/useLayoutLibrary'

const route = useRoute()
const tabsScrollRef = ref<HTMLElement | null>(null)

const {
  loaded,
  config,
  flush,
  currentLayoutId,
  isLayoutDirty,
} = useConfig()
const { saveCurrentLayout, saveBusy } = useSettingsScreen()
const mapper = useMapper()
const { layout } = useLayout()
const gameMode = useGameMode()
const { t } = useI18n()

const tabItems = computed(() => [
  { key: 'rules', to: '/rules', label: t('tabs.rules'), icon: 'i-lucide-workflow' },
  { key: 'keymap', to: '/keymap', label: t('tabs.keymap'), icon: 'i-lucide-keyboard' },
  { key: 'macros', to: '/macros', label: t('tabs.macros'), icon: 'i-lucide-zap' },
  { key: 'commands', to: '/commands', label: t('tabs.commands'), icon: 'i-lucide-terminal' },
])

const currentLayoutLabel = computed<string>(() => {
  const id = currentLayoutId.value
  if (!id) {
    // In auto mode, an empty id means "no layout matched" rather than a
    // custom unsaved layout.
    return config.value.settings.layoutMode === 'auto'
      ? t('app.noLayout')
      : t('app.customLayout')
  }
  if (isUserLayoutId(id)) return userLayoutNameFromId(id)
  return id
})

const selectedDevice = computed(() => config.value.settings.inputDevicePath ?? '')
const saveLabel = computed(() => (isLayoutDirty.value ? t('common.save') : t('app.saved')))
const saveIcon = computed(() => (isLayoutDirty.value ? 'i-lucide-save' : 'i-lucide-check'))

function isActive(path: string) {
  return route.path === path
}

function onTabWheel(e: WheelEvent) {
  if (!tabsScrollRef.value) return
  const el = tabsScrollRef.value
  if (e.deltaY !== 0 && el.scrollWidth > el.clientWidth) {
    e.preventDefault()
    el.scrollLeft += e.deltaY
  }
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

async function quitApplication() {
  const tauri = await useTauri()
  if (!tauri) return
  await tauri.invoke('quit_application')
}

onMounted(() => {
  void mapper.refreshStatus()
})
</script>

<template>
  <header
    class="flex items-center justify-between px-4 h-[var(--app-header-height)] border-b border-(--ui-border) bg-(--ui-bg-elevated) gap-3 shrink-0 app-chrome"
    data-tauri-drag-region
  >
    <div class="flex items-center gap-2 min-w-0 flex-1">
      <UButton
        color="neutral"
        variant="ghost"
        :square="true"
        size="sm"
        icon="i-lucide-keyboard"
        :aria-label="$t('app.title')"
        class="shrink-0 -ml-2 transition-all duration-200"
        :class="isActive('/')
          ? 'text-primary bg-(--ui-primary)/8'
          : 'text-(--ui-text-muted) hover:text-primary'"
        to="/"
      />

      <AppTooltip :text="currentLayoutLabel">
        <UButton
          color="neutral"
          variant="ghost"
          size="sm"
          class="shrink min-w-0 max-w-[14rem] transition-all duration-200"
          :class="isActive('/')
            ? 'text-primary bg-(--ui-primary)/8'
            : 'text-(--ui-text-highlighted) hover:text-primary'"
          to="/"
        >
          <span class="block truncate text-sm font-semibold">
            {{ currentLayoutLabel }}
          </span>
        </UButton>
      </AppTooltip>

      <span class="shrink-0 w-1" aria-hidden="true" />

      <div
        ref="tabsScrollRef"
        class="min-w-0 flex-1 overflow-x-auto overflow-y-hidden hide-scrollbar"
        @wheel="onTabWheel"
      >
        <div class="inline-flex min-w-max items-center gap-1.5">
          <UButton
            v-for="item in tabItems"
            :key="item.key"
            :color="isActive(item.to) ? 'primary' : 'neutral'"
            variant="ghost"
            :icon="item.icon"
            :aria-label="item.label"
            size="sm"
            class="px-3 text-(--ui-text-muted) hover:text-primary"
            :class="isActive(item.to)
              ? 'text-primary shadow-none ring-1 ring-inset ring-(--ui-primary)/25 bg-(--ui-primary)/8'
              : 'bg-transparent shadow-none'"
            :to="item.to"
          >
            <span>{{ item.label }}</span>
          </UButton>
        </div>
      </div>
    </div>

    <div class="flex items-center gap-3 shrink-0">
      <template v-if="loaded">
        <AppTooltip
          :disabled="mapper.status.value.running || !!selectedDevice"
          :text="$t('settings.startDisabledTooltip')"
        >
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
        </AppTooltip>

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
              @click="saveCurrentLayout"
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
            <span class="text-xs opacity-60 mr-0.5">{{ $t('app.layoutLanguageLabel') }}</span>
            {{ layout.short }}{{ layout.display ? ` (${layout.display})` : '' }}
          </UBadge>
        </AppTooltip>

        <AppTooltip :text="gameMode.status.value.active ? $t('settings.gameModeActive') : $t('settings.gameModeInactive')">
          <UBadge
            :color="gameMode.status.value.active ? 'error' : 'neutral'"
            :variant="gameMode.status.value.active ? 'solid' : 'outline'"
            size="sm"
            :class="!gameMode.status.value.active ? 'opacity-50' : ''"
          >
            <UIcon name="i-lucide-gamepad-2" class="mr-1 h-3.5 w-3.5" />
            {{ $t('app.gameModeLabel') }}
          </UBadge>
        </AppTooltip>

        <UButton
          :color="isActive('/settings') ? 'primary' : 'neutral'"
          :variant="isActive('/settings') ? 'soft' : 'ghost'"
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
