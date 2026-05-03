<script setup lang="ts">
import type { AppConfig } from '~/types/config'

const props = defineProps<{
  config: AppConfig
}>()

const emit = defineEmits<{
  'update:config': [config: AppConfig]
}>()

function setNonNegativeInt(
  key:
    | 'defaultHoldTimeoutMs'
    | 'defaultDoubleTapTimeoutMs'
    | 'defaultMacroStepPauseMs'
    | 'defaultMacroModifierDelayMs',
  value: string | number,
) {
  const parsed = typeof value === 'number' ? value : Number(value)
  if (!Number.isFinite(parsed)) return
  emit('update:config', {
    ...props.config,
    settings: {
      ...props.config.settings,
      [key]: Math.max(0, Math.round(parsed)),
    },
  })
}
</script>

<template>
  <UCard>
    <template #header>
      <h2 class="text-sm font-semibold">{{ $t('settings.behaviorTitle') }}</h2>
    </template>

    <div class="space-y-4">
      <div class="grid gap-4 md:grid-cols-2">
        <UFormField>
          <template #label>
            <FieldLabel
              :label="$t('settings.holdTimeout')"
              :hint="$t('settings.holdTimeoutHint')"
            />
          </template>
          <NumericInput
            :model-value="String(props.config.settings.defaultHoldTimeoutMs)"
            :min="0"
            class="w-full md:w-40"
            @update:model-value="(value: string | number) => setNonNegativeInt('defaultHoldTimeoutMs', value)"
          />
        </UFormField>

        <UFormField>
          <template #label>
            <FieldLabel
              :label="$t('settings.doubleTapTimeout')"
              :hint="$t('settings.doubleTapTimeoutHint')"
            />
          </template>
          <NumericInput
            :model-value="String(props.config.settings.defaultDoubleTapTimeoutMs)"
            :min="0"
            class="w-full md:w-40"
            @update:model-value="(value: string | number) => setNonNegativeInt('defaultDoubleTapTimeoutMs', value)"
          />
        </UFormField>
      </div>

      <div class="grid gap-4 pt-2 border-t border-(--ui-border) md:grid-cols-2">
        <UFormField>
          <template #label>
            <FieldLabel
              :label="$t('settings.stepPauseLabel')"
              :hint="$t('settings.stepPauseHint')"
            />
          </template>
          <NumericInput
            :model-value="String(props.config.settings.defaultMacroStepPauseMs)"
            :min="0"
            class="w-full md:w-40"
            @update:model-value="(value: string | number) => setNonNegativeInt('defaultMacroStepPauseMs', value)"
          />
        </UFormField>

        <UFormField>
          <template #label>
            <FieldLabel
              :label="$t('settings.modDelayLabel')"
              :hint="$t('settings.modDelayHint')"
            />
          </template>
          <NumericInput
            :model-value="String(props.config.settings.defaultMacroModifierDelayMs)"
            :min="0"
            class="w-full md:w-40"
            @update:model-value="(value: string | number) => setNonNegativeInt('defaultMacroModifierDelayMs', value)"
          />
        </UFormField>
      </div>
    </div>
  </UCard>
</template>
