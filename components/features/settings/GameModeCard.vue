<script setup lang="ts">
import type { AppConfig } from '~/types/config'

const props = defineProps<{
  config: AppConfig
}>()
</script>

<template>
  <UCard>
    <template #header>
      <div class="flex items-center gap-2">
        <UIcon name="i-lucide-gamepad-2" class="h-5 w-5 text-(--ui-primary)" />
        <h2 class="text-sm font-semibold">{{ $t('settings.gameModeTitle') }}</h2>
      </div>
      <p class="mt-1 text-sm text-(--ui-text-muted)">
        {{ $t('settings.gameModeSubtitle') }}
      </p>
    </template>

    <div class="space-y-4">
      <div class="grid gap-4 sm:grid-cols-2">
        <UFormField
          :label="$t('settings.gameModeUseGamemoded')"
          :description="$t('settings.gameModeUseGamemodedHint')"
        >
          <UCheckbox v-model="config.settings.gameMode.useGamemoded" />
        </UFormField>

        <UFormField
          :label="$t('settings.gameModeUseFullscreen')"
          :description="$t('settings.gameModeUseFullscreenHint')"
        >
          <UCheckbox v-model="config.settings.gameMode.useFullscreen" />
        </UFormField>
      </div>

      <div class="rounded-lg bg-(--ui-bg-muted) p-3 text-xs text-(--ui-text-muted)">
        <div class="flex items-start gap-2">
          <UIcon name="i-lucide-info" class="mt-0.5 h-4 w-4 shrink-0" />
          <p>
            {{ $t('settings.gameModeInfo') }}
          </p>
        </div>
      </div>
    </div>
  </UCard>
</template>
