<script setup lang="ts">
import type { MacroStep } from '~/types/config'

const _props = defineProps<{
  step: MacroStep
  idx: number
  isFirst: boolean
  isLast: boolean
  macroId: string
  stepError?: (step: MacroStep) => string | null
  stepWarning?: (step: MacroStep) => string | null
}>()

const emit = defineEmits<{
  moveUp: []
  moveDown: []
  askRemove: [stepId: string]
  'update:step': [step: MacroStep]
}>()
</script>

<template>
  <div
    class="group grid grid-cols-[2rem_minmax(12rem,1fr)_auto] items-center gap-2 rounded-md border border-transparent p-1 transition-all duration-200"
  >
    <div class="text-xs text-(--ui-text-muted) font-mono text-right">
      #{{ idx + 1 }}
    </div>
    <UFormField :error="stepError?.(step) ?? undefined">
      <ActionPickerModal
        :model-value="step.keystroke"
        :allow-macros="true"
        :excluded-macro-id="macroId"
        :placeholder="$t('macros.stepPh')"
        :invalid="!!stepError?.(step)"
        @update:model-value="(v: string | null) => emit('update:step', { ...step, keystroke: v ?? '' })"
      />
      <p
        v-if="stepWarning?.(step) && !stepError?.(step)"
        class="text-xs text-(--ui-warning) mt-0.5"
      >
        {{ stepWarning(step) }}
      </p>
    </UFormField>
    <div class="flex items-center gap-1 opacity-0 group-hover:opacity-100 transition-opacity duration-150">
      <AppTooltip :text="$t('common.moveUp')">
        <UButton
          icon="i-lucide-chevron-up"
          size="xs"
          variant="ghost"
          color="neutral"
          square
          :disabled="isFirst"
          :aria-label="$t('macros.moveUp')"
          @click="emit('moveUp')"
        />
      </AppTooltip>
      <AppTooltip :text="$t('common.moveDown')">
        <UButton
          icon="i-lucide-chevron-down"
          size="xs"
          variant="ghost"
          color="neutral"
          square
          :disabled="isLast"
          :aria-label="$t('macros.moveDown')"
          @click="emit('moveDown')"
        />
      </AppTooltip>
      <AppTooltip :text="$t('macros.deleteStep')">
        <UButton
          icon="i-lucide-trash-2"
          size="xs"
          variant="ghost"
          color="error"
          square
          :aria-label="$t('macros.deleteStep')"
          @click="emit('askRemove', step.id)"
        />
      </AppTooltip>
    </div>
  </div>
</template>
