<script setup lang="ts">
import type { Macro } from '~/types/config'
import SettingTimeoutField from '~/components/SettingTimeoutField.vue'

const props = defineProps<{
  macro: Macro
  idError?: string
  usage: string[]
  defaultStepPauseMs: number
  defaultModifierDelayMs: number
  uiKey: string
}>()

defineEmits<{
  remove: [payload: { uiKey: string, id: string }]
  addStep: [macro: Macro]
  moveStep: [macro: Macro, index: number, delta: number]
  removeStep: [macro: Macro, stepId: string]
}>()
</script>

<template>
  <div class="rounded-md border border-(--ui-border) bg-(--ui-bg-muted) p-4 space-y-4">
    <div class="grid grid-cols-[1fr_1fr_auto] gap-3">
      <UFormField>
        <template #label>
          <FieldLabel
            :label="$t('macros.nameLabel')"
            :hint="$t('macros.nameHint')"
          />
        </template>
        <UInput
          v-model="macro.name"
          :placeholder="$t('macros.namePh')"
          class="w-full"
        />
      </UFormField>
      <UFormField :error="idError">
        <template #label>
          <FieldLabel
            :label="$t('macros.idLabel')"
            :hint="$t('macros.idHint')"
          />
        </template>
        <UInput
          v-model="macro.id"
          :color="idError ? 'error' : undefined"
          :highlight="!!idError"
          class="w-full font-mono"
          :placeholder="$t('macros.idPh')"
        />
      </UFormField>
      <div class="flex items-end">
        <UButton
          icon="i-lucide-trash-2"
          color="error"
          variant="ghost"
          square
          :aria-label="$t('macros.deleteMacro')"
          @click="$emit('remove', { uiKey: props.uiKey, id: macro.id })"
        />
      </div>
    </div>

    <div class="grid grid-cols-2 gap-x-6 gap-y-1">
      <SettingTimeoutField
        v-model="macro.stepPauseMs"
        :label="$t('macros.stepPauseLabel')"
        :hint="$t('macros.stepPauseHint')"
        :default-value="defaultStepPauseMs"
        :suffix="$t('common.ms')"
      />
      <SettingTimeoutField
        v-model="macro.modifierDelayMs"
        :label="$t('macros.modDelayLabel')"
        :hint="$t('macros.modDelayHint')"
        :default-value="defaultModifierDelayMs"
        :suffix="$t('common.ms')"
      />
    </div>

    <div
      v-if="usage.length"
      class="flex flex-wrap gap-1 text-xs"
    >
      <span class="text-(--ui-text-muted)">{{ $t('macros.usedIn') }}</span>
      <UBadge
        v-for="place in usage"
        :key="place"
        color="neutral"
        variant="subtle"
        class="font-mono"
      >
        {{ place }}
      </UBadge>
    </div>

    <div class="border-t border-(--ui-border) pt-3">
      <div class="flex items-center justify-between mb-2">
        <div class="text-sm font-medium text-(--ui-text-muted)">{{ $t('macros.steps') }}</div>
        <UButton
          size="xs"
          icon="i-lucide-plus"
          variant="outline"
          @click="$emit('addStep', macro)"
        >
          {{ $t('macros.addStep') }}
        </UButton>
      </div>

      <div
        v-if="macro.steps.length === 0"
        class="text-sm text-(--ui-text-muted) italic"
      >
        {{ $t('macros.stepsEmpty') }}
      </div>

      <div v-else class="space-y-2">
        <div
          v-for="(step, idx) in macro.steps"
          :key="step.id"
          class="grid grid-cols-[2rem_minmax(16rem,1fr)_auto_auto_auto] gap-2 items-center"
        >
          <div class="text-xs text-(--ui-text-muted) font-mono text-right">
            #{{ idx + 1 }}
          </div>
          <ActionPickerModal
            v-model="step.keystroke"
            :placeholder="$t('macros.stepPh')"
          />
          <UButton
            icon="i-lucide-chevron-up"
            size="xs"
            variant="ghost"
            color="neutral"
            square
            :disabled="idx === 0"
            :aria-label="$t('macros.moveUp')"
            @click="$emit('moveStep', macro, idx, -1)"
          />
          <UButton
            icon="i-lucide-chevron-down"
            size="xs"
            variant="ghost"
            color="neutral"
            square
            :disabled="idx === macro.steps.length - 1"
            :aria-label="$t('macros.moveDown')"
            @click="$emit('moveStep', macro, idx, 1)"
          />
          <UButton
            icon="i-lucide-trash-2"
            size="xs"
            variant="ghost"
            color="error"
            square
            :aria-label="$t('macros.deleteStep')"
            @click="$emit('removeStep', macro, step.id)"
          />
        </div>
      </div>
    </div>

    <div class="text-xs text-(--ui-text-muted)">
      <i18n-t keypath="macros.assignHint" tag="span">
        <template #ref>
          <code class="font-mono">macro:{{ macro.id }}</code>
        </template>
      </i18n-t>
    </div>
  </div>
</template>
