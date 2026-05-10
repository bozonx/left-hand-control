<script setup lang="ts">
import AppTooltip from '~/components/shared/AppTooltip.vue'
import NumericInput from '~/components/shared/NumericInput.vue'
import type { MacroStep } from '~/types/config'
import { parsePauseAction } from '~/utils/actionSyntax'

const props = defineProps<{
  step: MacroStep
  idx: number
  isFirst: boolean
  isLast: boolean
  excludedMacroId?: string
  stepError?: (step: MacroStep, excludedMacroId?: string) => string | null
  stepWarning?: (step: MacroStep) => string | null
}>()

const emit = defineEmits<{
  moveUp: []
  moveDown: []
  askRemove: [stepId: string]
  'update:step': [step: MacroStep]
}>()

const isPauseStep = computed(() => props.step.action.startsWith('pause:'))
const pauseMs = computed({
  get: () => parsePauseAction(props.step.action) ?? 0,
  set: (value: string | number) => {
    const parsed = Number(value)
    const next = Number.isFinite(parsed) ? Math.min(10000, Math.max(0, Math.round(parsed))) : 0
    emit('update:step', { ...props.step, action: `pause:${next}` })
  },
})

</script>

<template>
  <div
    class="group grid grid-cols-[2rem_minmax(12rem,1fr)_auto] items-center gap-2 rounded-md border border-transparent p-1 transition-all duration-200"
  >
    <div class="text-xs text-(--ui-text-muted) font-mono text-right">
      #{{ idx + 1 }}
    </div>
    <UFormField :error="stepError?.(step, excludedMacroId) ?? undefined">
      <ActionPickerModal
        v-if="!isPauseStep"
        :model-value="step.action"
        :excluded-macro-id="excludedMacroId"
        :placeholder="$t('macros.stepPh')"
        :invalid="!!stepError?.(step, excludedMacroId)"
        @update:model-value="(v: string | null) => emit('update:step', { ...step, action: v ?? '' })"
      />
      <div v-else class="flex items-center gap-2">
        <NumericInput
          v-model="pauseMs"
          class="w-28 font-mono"
          size="sm"
          :min="0"
          :max="10000"
        />
        <span class="text-xs uppercase text-(--ui-text-muted)">{{ $t('common.ms') }}</span>
      </div>
      <p
        v-if="stepWarning?.(step) && !stepError?.(step, excludedMacroId)"
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
