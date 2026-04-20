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
  currentLayoutId,
  isLayoutDirty,
} = useConfig()
const { layout } = useLayout()
const theme = useAppTheme()
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
    class="flex items-center justify-between px-4 py-2 border-b border-(--ui-border) bg-(--ui-bg-elevated) gap-3 flex-wrap"
  >
    <div class="flex items-center gap-3 flex-wrap">
      <h1 class="text-lg font-semibold">{{ $t('app.title') }}</h1>
      <UBadge color="primary" variant="subtle">{{ $t('app.badge') }}</UBadge>
      <UBadge
        v-if="loaded"
        :color="isLayoutDirty ? 'warning' : 'neutral'"
        :variant="isLayoutDirty ? 'solid' : 'outline'"
        class="max-w-[22rem] truncate"
        :title="
          isLayoutDirty
            ? $t('app.dirtyTooltip')
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
        <span v-if="isLayoutDirty" class="ml-1 font-semibold">
          {{ $t('app.notSavedBadge') }}
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
      <span v-if="!loaded">{{ $t('app.loading') }}</span>
      <span v-else-if="saving" class="flex items-center gap-1">
        <UIcon name="i-lucide-loader-2" class="animate-spin" />
        {{ $t('app.saving') }}
      </span>
      <span v-else class="flex items-center gap-1 text-(--ui-success)">
        <UIcon name="i-lucide-check" />
        {{ $t('app.saved') }}
      </span>
      <span v-if="lastError" class="text-(--ui-error)">
        {{ lastError }}
      </span>
      <UButton
        size="xs"
        color="neutral"
        variant="ghost"
        :icon="
          theme.resolved.value === 'dark'
            ? 'i-lucide-moon'
            : 'i-lucide-sun'
        "
        :aria-label="
          theme.resolved.value === 'dark'
            ? $t('app.switchToLight')
            : $t('app.switchToDark')
        "
        :title="
          theme.resolved.value === 'dark'
            ? $t('app.switchToLight')
            : $t('app.switchToDark')
        "
        @click="theme.toggle()"
      />
    </div>
  </header>
</template>
