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
  nameInputId: string
  isFirst?: boolean
  isLast?: boolean
}>()

defineEmits<{
  remove: [payload: { uiKey: string, id: string }]
  moveUp: [uiKey: string]
  moveDown: [uiKey: string]
  addStep: [macro: Macro]
  moveStep: [macro: Macro, index: number, delta: number]
  removeStep: [macro: Macro, stepId: string]
}>()

const toast = useToast()
const { t } = useI18n()

async function copyMacroId() {
  if (!props.macro.id) return
  try {
    await navigator.clipboard.writeText(props.macro.id)
    toast.add({
      title: t('common.copied'),
      description: props.macro.id,
      icon: 'i-lucide-copy-check',
      close: true,
    })
  } catch (error) {
    toast.add({
      title: t('common.copy'),
      description: error instanceof Error ? error.message : String(error),
      color: 'error',
      icon: 'i-lucide-circle-alert',
      close: true,
    })
  }
}
</script>

<template>
  <div
    class="relative p-4 rounded-xl border border-(--ui-border) bg-(--ui-bg-muted)/40 flex gap-6 group transition-all duration-300 hover:bg-(--ui-bg-muted)/60 hover:border-sky-500/50 hover:shadow-lg hover:shadow-sky-500/5"
  >
    <div class="flex-1 flex flex-col gap-4 min-w-0">
      <div class="grid grid-cols-2 gap-3">
        <UFormField :error="idError">
          <template #label>
            <FieldLabel
              :label="$t('macros.idLabel')"
              :hint="$t('macros.idHint')"
              hint-visible-on="group-hover"
            />
          </template>
          <div class="flex items-center gap-2">
            <UInput
              v-model="macro.id"
              :color="idError ? 'error' : undefined"
              :highlight="!!idError"
              class="w-full font-mono"
              :placeholder="$t('macros.idPh')"
            />
            <UButton
              icon="i-lucide-copy"
              size="sm"
              color="neutral"
              variant="ghost"
              class="shrink-0 opacity-70 transition-opacity sm:opacity-0 sm:group-hover:opacity-100 sm:focus-within:opacity-100"
              :aria-label="$t('macros.copyId')"
              :disabled="!macro.id"
              @click="copyMacroId"
            />
          </div>
        </UFormField>

        <UFormField>
          <template #label>
            <FieldLabel
              :label="$t('macros.nameLabel')"
              :hint="$t('macros.nameHint')"
              hint-visible-on="group-hover"
            />
          </template>
          <UInput
            :id="nameInputId"
            v-model="macro.name"
            :placeholder="$t('macros.namePh')"
            class="w-full"
          />
        </UFormField>
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
          <div class="text-xs font-medium text-(--ui-text-muted)">{{ $t('macros.steps') }}</div>
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
            class="grid grid-cols-[2rem_minmax(12rem,1fr)_auto_auto_auto] gap-2 items-center"
          >
            <div class="text-xs text-(--ui-text-muted) font-mono text-right">
              #{{ idx + 1 }}
            </div>
            <ActionPickerModal
              v-model="step.keystroke"
              :excluded-macro-id="macro.id"
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
    </div>

    <div class="w-px bg-(--ui-border) self-stretch"></div>

    <div class="w-52 flex flex-col gap-4">
      <div class="flex items-center justify-between">
        <div class="flex gap-1 opacity-0 group-hover:opacity-100 transition-opacity duration-300">
          <UButton
            icon="i-lucide-arrow-up"
            variant="ghost"
            color="neutral"
            size="sm"
            square
            :disabled="isFirst"
            :aria-label="$t('macros.moveUp')"
            @click="$emit('moveUp', props.uiKey)"
          />
          <UButton
            icon="i-lucide-arrow-down"
            variant="ghost"
            color="neutral"
            size="sm"
            square
            :disabled="isLast"
            :aria-label="$t('macros.moveDown')"
            @click="$emit('moveDown', props.uiKey)"
          />
        </div>

        <UButton
          icon="i-lucide-trash-2"
          color="neutral"
          variant="ghost"
          size="sm"
          square
          class="opacity-0 group-hover:opacity-100 transition-opacity duration-300"
          :aria-label="$t('macros.deleteMacro')"
          @click="$emit('remove', { uiKey: props.uiKey, id: macro.id })"
        />
      </div>

      <div class="flex flex-col gap-1 mt-1">
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
    </div>
  </div>
</template>
