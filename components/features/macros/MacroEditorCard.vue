<script setup lang="ts">
import type { Macro, MacroStep } from '~/types/config'
import SettingTimeoutField from '~/components/SettingTimeoutField.vue'

const props = defineProps<{
  macro: Macro
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
  selected?: boolean
}>()

const emit = defineEmits<{
  select: []
  remove: [payload: { uiKey: string, id: string }]
  moveUp: [uiKey: string]
  moveDown: [uiKey: string]
  addStep: [macro: Macro]
  moveStep: [macro: Macro, index: number, delta: number]
  removeStep: [macro: Macro, stepId: string]
  nameFocused: [uiKey: string]
}>()

const stepsContainerRef = useTemplateRef<HTMLElement>('stepsContainerRef')
const stepIds = computed(() => props.macro.steps.map((s) => s.id))
const { selectedId: selectedStepId } = useListKeyboardNavigation({
  ids: stepIds,
  move: (id, delta) => {
    const idx = props.macro.steps.findIndex((s) => s.id === id)
    if (idx < 0) return
    emit('moveStep', props.macro, idx, delta)
  },
})

function onCardClick(event: MouseEvent) {
  const target = event.target as HTMLElement | null
  if (target?.closest('input, textarea, select, button, [role="dialog"], [role="listbox"]')) return
  emit('select')
}

const { copy: copyToClipboard } = useClipboardCopy()
const nameInputRef = useTemplateRef<{ inputRef?: { value?: HTMLInputElement } }>('nameInputRef')

const stepConfirmOpen = ref(false)
const pendingStepId = ref<string | null>(null)

function askRemoveStep(stepId: string) {
  pendingStepId.value = stepId
  stepConfirmOpen.value = true
}

function confirmRemoveStep() {
  if (pendingStepId.value) {
    emit('removeStep', props.macro, pendingStepId.value)
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
  async (value) => {
    if (!value) return
    await new Promise((resolve) => requestAnimationFrame(() => resolve(undefined)))
    const input = nameInputRef.value?.inputRef?.value
    if (!input) return
    input.focus()
    input.select()
    emit('nameFocused', props.uiKey)
  },
  { immediate: true, flush: 'post' },
)

async function copyMacroId() {
  if (!props.macro.id) return
  await copyToClipboard(props.macro.id)
}
</script>

<template>
  <div
    class="relative p-4 rounded-xl border flex gap-6 group transition-all duration-150 cursor-pointer"
    :class="[
      selected
        ? 'border-(--ui-primary) ring-1 ring-(--ui-primary) bg-(--ui-bg-muted)/60 shadow-lg shadow-(--ui-primary)/5'
        : 'border-(--ui-border) bg-(--ui-bg-muted)/40 hover:bg-(--ui-bg-muted)/60 hover:border-(--ui-primary)/50 hover:shadow-lg hover:shadow-(--ui-primary)/5',
    ]"
    @click="onCardClick"
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
            ref="nameInputRef"
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

        <div v-else ref="stepsContainerRef" class="space-y-2">
          <div
            v-for="(step, idx) in macro.steps"
            :key="step.id"
            class="grid grid-cols-[2rem_minmax(12rem,1fr)_auto_auto_auto] gap-2 items-center p-1 rounded-md transition-all duration-200"
            :class="[
              selectedStepId === step.id
                ? 'border border-(--ui-primary) ring-1 ring-(--ui-primary) bg-(--ui-bg-muted)/60 shadow-md shadow-(--ui-primary)/5'
                : 'border border-transparent',
            ]"
            @click="selectedStepId = step.id"
          >
            <div class="text-xs text-(--ui-text-muted) font-mono text-right">
              #{{ idx + 1 }}
            </div>
            <UFormField :error="stepError?.(step) ?? undefined">
              <ActionPickerModal
                v-model="step.keystroke"
                :allow-macros="false"
                :excluded-macro-id="macro.id"
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
              @click="askRemoveStep(step.id)"
            />
          </div>
        </div>
      </div>
    </div>

    <div class="w-px bg-(--ui-border) self-stretch"></div>

    <div class="min-w-[12rem] max-w-[16rem] flex flex-col gap-4">
      <div class="flex items-center justify-between">
        <div class="flex gap-1 opacity-0 group-hover:opacity-100 transition-opacity duration-150">
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
          class="opacity-0 group-hover:opacity-100 transition-opacity duration-150"
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
