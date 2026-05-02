<script setup lang="ts">
import type { MacroStep } from '~/types/config'

const props = defineProps<{
  step: MacroStep
  idx: number
  isFirst: boolean
  isLast: boolean
  macroId: string
  selected?: boolean
  stepError?: (step: MacroStep) => string | null
  stepWarning?: (step: MacroStep) => string | null
}>()

const emit = defineEmits<{
  select: []
  moveUp: []
  moveDown: []
  askRemove: [stepId: string]
}>()
</script>

<template>
  <div
    class="grid grid-cols-[2rem_minmax(12rem,1fr)_auto_auto_auto] gap-2 items-center p-1 rounded-md transition-all duration-200"
    :class="[
      selected
        ? 'border border-(--ui-primary) ring-1 ring-(--ui-primary) bg-(--ui-bg-muted)/60 shadow-md shadow-(--ui-primary)/5'
        : 'border border-transparent',
    ]"
    @click="emit('select')"
  >
    <div class="text-xs text-(--ui-text-muted) font-mono text-right">
      #{{ idx + 1 }}
    </div>
    <UFormField :error="stepError?.(step) ?? undefined">
      <ActionPickerModal
        v-model="step.keystroke"
        :allow-macros="false"
        :excluded-macro-id="macroId"
        :placeholder="$t('macros.stepPh')"
        :invalid="!!stepError?.(step)"
      />
      <p
        v-if="stepWarning?.(step) && !stepError?.(step)"
        class="text-xs text-(--ui-warning) mt-0.5"
      >
        {{ stepWarning(step) }}
      </p>
    </UFormField>
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
</template>
