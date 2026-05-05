<script setup lang="ts">
import AppTooltip from '~/components/shared/AppTooltip.vue'
import MacroStepRow from '~/components/features/macros/MacroStepRow.vue'
import type { Macro, MacroStep } from '~/types/config'
import SettingTimeoutField from '~/components/SettingTimeoutField.vue'

const props = defineProps<{
  idError?: string
  usage: string[]
  defaultStepPauseMs: number
  defaultModifierDelayMs: number
  uiKey: string
  nameInputId: string
  stepError?: (step: MacroStep) => string | null
  stepWarning?: (step: MacroStep) => string | null
  isFirst?: boolean
  isLast?: boolean
  focusName?: boolean
}>()

const macro = defineModel<Macro>('macro', { required: true })

const emit = defineEmits<{
  remove: [payload: { uiKey: string, id: string }]
  moveUp: [uiKey: string]
  moveDown: [uiKey: string]
  addStep: [macro: Macro]
  moveStep: [macro: Macro, index: number, delta: number]
  removeStep: [macro: Macro, stepId: string]
  nameFocused: [uiKey: string]
}>()

const { copy: copyToClipboard } = useClipboardCopy()
const stepConfirmOpen = ref(false)
const pendingStepId = ref<string | null>(null)

function askRemoveStep(stepId: string) {
  pendingStepId.value = stepId
  stepConfirmOpen.value = true
}

function confirmRemoveStep() {
  if (pendingStepId.value) {
    emit('removeStep', macro.value, pendingStepId.value)
  }
  pendingStepId.value = null
  stepConfirmOpen.value = false
}

function cancelRemoveStep() {
  pendingStepId.value = null
  stepConfirmOpen.value = false
}

watch(
  () => props.focusName,
  (value) => {
    if (!value) return
    const input = document.getElementById(props.nameInputId) as HTMLInputElement | null
    if (!input) return
    input.focus()
    input.select()
    emit('nameFocused', props.uiKey)
  },
  { immediate: true, flush: 'post' },
)

async function copyMacroId() {
  if (!macro.value.id) return
  await copyToClipboard(macro.value.id)
}
</script>

<template>
  <div
    class="relative flex gap-6 rounded-xl border border-(--ui-border) bg-(--ui-bg-muted)/40 p-4 transition-all duration-150 group hover:border-(--ui-primary)/50 hover:bg-(--ui-bg-muted)/60 hover:shadow-lg hover:shadow-(--ui-primary)/5"
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
            <AppTooltip :text="$t('macros.copyId')">
              <UButton
                icon="i-lucide-copy"
                size="sm"
                color="neutral"
                variant="ghost"
                class="shrink-0 opacity-70 transition-opacity"
                :aria-label="$t('macros.copyId')"
                :disabled="!macro.id"
                @click="copyMacroId"
              />
            </AppTooltip>
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
            class="opacity-0 group-hover:opacity-100 transition-opacity"
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
          <MacroStepRow
            v-for="(step, idx) in macro.steps"
            :key="step.id"
            :step="step"
            :idx="idx"
            :is-first="idx === 0"
            :is-last="idx === macro.steps.length - 1"
            :step-error="stepError"
            :step-warning="stepWarning"
            @move-up="$emit('moveStep', macro, idx, -1)"
            @move-down="$emit('moveStep', macro, idx, 1)"
            @ask-remove="askRemoveStep"
            @update:step="(v: MacroStep) => { macro.steps[idx] = v }"
          />
        </div>
      </div>
    </div>

    <div class="w-px bg-(--ui-border) self-stretch"></div>

    <div class="min-w-[14rem] max-w-[18rem] flex flex-col gap-4">
      <div class="flex items-center justify-between">
        <div class="flex gap-1 opacity-0 group-hover:opacity-100 transition-opacity duration-150">
          <AppTooltip :text="$t('common.moveUp')">
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
          </AppTooltip>
          <AppTooltip :text="$t('common.moveDown')">
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
          </AppTooltip>
        </div>

        <AppTooltip :text="$t('macros.deleteMacro')">
          <UButton
            icon="i-lucide-trash-2"
            color="neutral"
            variant="ghost"
            size="sm"
            square
            class="opacity-0 group-hover:opacity-100 transition-opacity duration-150"
            :aria-label="$t('macros.deleteMacro')"
            @click="$emit('remove', { uiKey: props.uiKey, id: macro.id })"
          />
        </AppTooltip>
      </div>

      <div class="flex flex-col gap-1 mt-1">
        <SettingTimeoutField
          v-model="macro.stepPauseMs"
          :label="$t('macros.stepPauseLabel')"
          :hint="$t('macros.stepPauseHint')"
          hint-visible-on="group-hover"
          :default-value="defaultStepPauseMs"
          :suffix="$t('common.ms')"
          :max="2000"
        />
        <SettingTimeoutField
          v-model="macro.modifierDelayMs"
          :label="$t('macros.modDelayLabel')"
          :hint="$t('macros.modDelayHint')"
          hint-visible-on="group-hover"
          :default-value="defaultModifierDelayMs"
          :suffix="$t('common.ms')"
          :max="2000"
        />
      </div>
    </div>
  </div>

  <UModal v-model:open="stepConfirmOpen" :title="$t('macros.confirmDeleteStepTitle')">
    <template #body>
      <p class="text-sm">{{ $t('macros.confirmDeleteStepBody') }}</p>
    </template>
    <template #footer>
      <div class="flex gap-2 justify-end w-full">
        <UButton variant="ghost" color="neutral" @click="cancelRemoveStep">
          {{ $t('common.cancel') }}
        </UButton>
        <UButton color="error" icon="i-lucide-trash-2" @click="confirmRemoveStep">
          {{ $t('common.delete') }}
        </UButton>
      </div>
    </template>
  </UModal>
</template>
