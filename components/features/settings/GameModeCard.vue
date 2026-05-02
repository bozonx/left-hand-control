<script setup lang="ts">
import FieldLabel from '~/components/FieldLabel.vue'

const _props = defineProps<{
  useGamemoded: boolean
  useFullscreen: boolean
}>()

const _emit = defineEmits<{
  'update:useGamemoded': [value: boolean]
  'update:useFullscreen': [value: boolean]
}>()
</script>

<template>
  <UCard variant="outline">
    <template #header>
      <div>
        <div class="flex items-center gap-2">
          <UIcon name="i-lucide-gamepad-2" class="h-5 w-5 text-(--ui-primary)" />
          <h2 class="text-sm font-semibold">{{ $t('settings.gameModeTitle') }}</h2>
        </div>
        <p class="mt-1 text-sm text-(--ui-text-muted)">
          {{ $t('settings.gameModeSubtitle') }}
        </p>
      </div>
    </template>

    <div class="space-y-4">
      <div class="grid gap-4 sm:grid-cols-2">
        <UFormField>
          <template #label>
            <FieldLabel
              :label="$t('settings.gameModeUseGamemoded')"
              :hint="$t('settings.gameModeUseGamemodedHint')"
            />
          </template>
          <UCheckbox
            :model-value="useGamemoded"
            @update:model-value="$emit('update:useGamemoded', $event as boolean)"
          />
        </UFormField>

        <UFormField>
          <template #label>
            <FieldLabel
              :label="$t('settings.gameModeUseFullscreen')"
              :hint="$t('settings.gameModeUseFullscreenHint')"
            />
          </template>
          <UCheckbox
            :model-value="useFullscreen"
            @update:model-value="$emit('update:useFullscreen', $event as boolean)"
          />
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
