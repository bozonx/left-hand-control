<script setup lang="ts">
import AppTooltip from '~/components/shared/AppTooltip.vue'
import { BUILTIN_LAYOUT_ID } from '~/types/config'
import {
  isUserLayoutId,
  userLayoutNameFromId,
} from '~/composables/useLayoutLibrary'
import { BUILTIN_LAYOUT_META } from '~/utils/layoutPresets'

const props = defineProps<{
  activeTab: string
}>()

const emit = defineEmits<{
  'update:activeTab': [value: string]
}>()

const {
  loaded,
  currentLayoutId,
  isLayoutDirty,
} = useConfig()
const { layout } = useLayout()
const { t } = useI18n()

const currentLayoutLabel = computed<string>(() => {
  const id = currentLayoutId.value
  if (!id) return t('app.customLayout')
  if (id === BUILTIN_LAYOUT_ID) return BUILTIN_LAYOUT_META.name
  if (isUserLayoutId(id)) return userLayoutNameFromId(id)
  return id
})

const tabItems = computed(() => [
  { value: 'rules', label: t('tabs.rules'), icon: 'i-lucide-workflow' },
  { value: 'keymap', label: t('tabs.keymap'), icon: 'i-lucide-keyboard' },
  { value: 'macros', label: t('tabs.macros'), icon: 'i-lucide-zap' },
  { value: 'settings', label: t('tabs.settings'), icon: 'i-lucide-settings' },
])
</script>

<template>
  <header
    class="flex items-center justify-between px-4 h-[var(--app-header-height)] border-b border-(--ui-border) bg-(--ui-bg-elevated) gap-3 shrink-0 app-chrome"
  >
    <div class="flex items-center gap-8 min-w-0">
      <div class="flex items-center gap-2.5 shrink-0">
        <UIcon name="i-lucide-keyboard" class="w-5 h-5 text-primary" />
        <h1 class="text-[0.9375rem] font-semibold whitespace-nowrap">{{ $t('app.title') }}</h1>
      </div>

      <div class="flex items-center gap-1 bg-(--ui-bg) border border-(--ui-border) rounded-lg p-1">
        <UButton
          v-for="item in tabItems"
          :key="item.value"
          :color="activeTab === item.value ? 'primary' : 'neutral'"
          :variant="activeTab === item.value ? 'soft' : 'ghost'"
          :icon="item.icon"
          size="sm"
          @click="emit('update:activeTab', item.value)"
        >
          {{ item.label }}
        </UButton>
      </div>
    </div>

    <div class="flex items-center gap-3 shrink-0">
      <template v-if="loaded">
        <AppTooltip
          :text="isLayoutDirty ? $t('app.dirtyTooltip') : currentLayoutLabel"
        >
          <UBadge
            :color="isLayoutDirty ? 'warning' : 'neutral'"
            :variant="isLayoutDirty ? 'solid' : 'outline'"
            class="max-w-[18rem] truncate"
            size="sm"
          >
            <UIcon
              :name="
                isLayoutDirty
                  ? 'i-lucide-triangle-alert'
                  : 'i-lucide-keyboard'
              "
              class="mr-1 shrink-0"
            />
            <span class="text-[0.6875rem] opacity-60 mr-0.5">{{ $t('app.presetLabel') }}</span>
            <span class="truncate">{{ currentLayoutLabel }}</span>
            <span v-if="isLayoutDirty" class="ml-1 font-semibold">
              {{ $t('app.notSavedBadge') }}
            </span>
          </UBadge>
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
      </template>
      <div v-else class="text-xs text-(--ui-text-muted)">
        {{ $t('app.loading') }}
      </div>
    </div>
  </header>
</template>
