<script setup lang="ts">
import { BUILTIN_LAYOUT_ID } from '~/types/config'
import {
  isUserLayoutId,
  userLayoutNameFromId,
} from '~/composables/useLayoutLibrary'
import { BUILTIN_LAYOUT_META } from '~/utils/layoutPresets'

const badgeTooltipUi = {
  content: 'h-auto max-w-72 py-2',
}

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
</script>

<template>
  <header
    class="flex items-center justify-between px-4 h-[var(--app-header-height)] border-b border-(--ui-border) bg-(--ui-bg-elevated) gap-3 shrink-0 app-chrome"
  >
    <div class="flex items-center gap-2.5 min-w-0">
      <h1 class="text-[0.9375rem] font-semibold whitespace-nowrap">{{ $t('app.title') }}</h1>
      <UTooltip v-if="loaded" :ui="badgeTooltipUi">
        <UBadge
          :color="isLayoutDirty ? 'warning' : 'neutral'"
          :variant="isLayoutDirty ? 'solid' : 'outline'"
          class="max-w-[22rem] truncate"
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
        <template #content>
          <div class="whitespace-pre-wrap text-center">
            {{ isLayoutDirty ? $t('app.dirtyTooltip') : currentLayoutLabel }}
          </div>
        </template>
      </UTooltip>
      <UTooltip v-if="layout" :ui="badgeTooltipUi">
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
        <template #content>
          <div class="whitespace-pre-wrap text-center">
            {{ layout.long }}
          </div>
        </template>
      </UTooltip>
    </div>
    <div class="text-xs text-(--ui-text-muted) flex items-center gap-3 shrink-0">
      <span v-if="!loaded">{{ $t('app.loading') }}</span>
    </div>
  </header>
</template>
