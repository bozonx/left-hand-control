<script setup lang="ts">
import type { AppConfig, TapDecision } from '~/types/config'
import NumericInput from '~/components/shared/NumericInput.vue'

const props = defineProps<{
  config: AppConfig
}>()

const emit = defineEmits<{
  'update:config': [config: AppConfig]
}>()

const { t } = useI18n()

const tapDecisionItems = computed<Array<{ label: string, value: TapDecision }>>(() => [
  { label: t('settings.tapDecisionPermissive'), value: 'permissiveHold' },
  { label: t('settings.tapDecisionHoldOnPress'), value: 'holdOnOtherKeyPress' },
])

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

function setTapDecision(value: TapDecision) {
  emit('update:config', {
    ...props.config,
    settings: {
      ...props.config.settings,
      tapDecision: value,
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
      <UFormField>
        <template #label>
          <FieldLabel
            :label="$t('settings.tapDecision')"
            :hint="$t('settings.tapDecisionHint')"
          />
        </template>
        <USelectMenu
          :model-value="props.config.settings.tapDecision"
          :items="tapDecisionItems"
          value-key="value"
          class="w-full md:w-80"
          @update:model-value="(value: TapDecision) => setTapDecision(value)"
        />
      </UFormField>

      <div class="grid gap-4 pt-2 border-t border-(--ui-border) md:grid-cols-2">
        <UFormField>
          <template #label>
            <FieldLabel
              :label="$t('settings.holdTimeout')"
              :hint="$t('settings.holdTimeoutHint')"
            />
          </template>
          <NumericInput
            :model-value="props.config.settings.defaultHoldTimeoutMs"
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
            :model-value="props.config.settings.defaultDoubleTapTimeoutMs"
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
            :model-value="props.config.settings.defaultMacroStepPauseMs"
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
            :model-value="props.config.settings.defaultMacroModifierDelayMs"
            :min="0"
            class="w-full md:w-40"
            @update:model-value="(value: string | number) => setNonNegativeInt('defaultMacroModifierDelayMs', value)"
          />
        </UFormField>
      </div>
    </div>
  </UCard>
</template>
