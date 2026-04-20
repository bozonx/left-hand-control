<script setup lang="ts">
import { BUILTIN_LAYOUT_ID } from '~/types/config'
import {
  isUserLayoutId,
  userLayoutNameFromId,
} from '~/composables/useLayoutLibrary'
import { BUILTIN_LAYOUT_META } from '~/utils/layoutPresets'

const {
  loaded,
  saving,
  lastError,
  needsWelcome,
  currentLayoutId,
  isLayoutDirty,
  load,
} = useConfig()
const { layout } = useLayout()

const tabItems = [
  { value: 'rules', slot: 'rules', label: 'Слои', icon: 'i-lucide-layers' },
  { value: 'keymap', slot: 'keymap', label: 'Раскладка', icon: 'i-lucide-keyboard' },
  { value: 'macros', slot: 'macros', label: 'Макросы', icon: 'i-lucide-zap' },
  { value: 'settings', slot: 'settings', label: 'Настройки', icon: 'i-lucide-settings' },
] as const

const active = ref<string>('rules')

const currentLayoutLabel = computed<string>(() => {
  const id = currentLayoutId.value
  if (!id) return 'Пользовательская раскладка'
  if (id === BUILTIN_LAYOUT_ID) return BUILTIN_LAYOUT_META.name
  if (isUserLayoutId(id)) return userLayoutNameFromId(id)
  return id
})

onMounted(() => {
  void load()
})
</script>

<template>
  <WelcomeScreen v-if="loaded && needsWelcome" />

  <div v-else class="min-h-screen flex flex-col">
    <header
      class="flex items-center justify-between px-6 py-3 border-b border-(--ui-border) bg-(--ui-bg-elevated) gap-4 flex-wrap"
    >
      <div class="flex items-center gap-3 flex-wrap">
        <h1 class="text-lg font-semibold">Left Hand Control</h1>
        <UBadge color="primary" variant="subtle">Linux key-mapper</UBadge>
        <UBadge
          v-if="loaded"
          :color="isLayoutDirty ? 'warning' : 'neutral'"
          :variant="isLayoutDirty ? 'solid' : 'outline'"
          class="max-w-[22rem] truncate"
          :title="
            isLayoutDirty
              ? 'В текущей раскладке есть несохранённые изменения. Сохраните её в Настройках → Раскладки, иначе они потеряются при переключении.'
              : currentLayoutLabel
          "
        >
          <UIcon
            :name="
              isLayoutDirty
                ? 'i-lucide-alert-triangle'
                : 'i-lucide-keyboard'
            "
            class="mr-1"
          />
          <span class="truncate">{{ currentLayoutLabel }}</span>
          <span v-if="isLayoutDirty" class="ml-1 font-semibold">•
            не сохранено
          </span>
        </UBadge>
        <UBadge
          v-if="layout"
          color="neutral"
          variant="outline"
          :title="layout.long"
          class="font-mono uppercase"
        >
          <UIcon name="i-lucide-languages" class="mr-1" />
          {{ layout.short }}{{ layout.display ? ` (${layout.display})` : '' }}
        </UBadge>
      </div>
      <div class="text-xs text-(--ui-text-muted) flex items-center gap-3">
        <span v-if="!loaded">загрузка…</span>
        <span v-else-if="saving" class="flex items-center gap-1">
          <UIcon name="i-lucide-loader-2" class="animate-spin" />
          сохранение
        </span>
        <span v-else class="flex items-center gap-1 text-(--ui-success)">
          <UIcon name="i-lucide-check" />
          сохранено
        </span>
        <span v-if="lastError" class="text-(--ui-error)">
          {{ lastError }}
        </span>
      </div>
    </header>

    <main class="flex-1 p-6 w-full">
      <UTabs
        v-model="active"
        :items="tabItems"
        class="w-full"
      >
        <template #rules>
          <div class="mt-4">
            <RulesTab v-if="loaded" />
          </div>
        </template>
        <template #keymap>
          <div class="mt-4">
            <KeymapTab v-if="loaded" />
          </div>
        </template>
        <template #macros>
          <div class="mt-4">
            <MacrosTab v-if="loaded" />
          </div>
        </template>
        <template #settings>
          <div class="mt-4">
            <SettingsTab v-if="loaded" />
          </div>
        </template>
      </UTabs>
    </main>
  </div>
</template>
