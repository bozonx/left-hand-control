<script setup lang="ts">
import AppTooltip from '~/components/shared/AppTooltip.vue'
import FieldResetButton from '~/components/shared/FieldResetButton.vue'
import { parseTextAction } from '~/types/config'
import { isCanonicalAction, normalizeActionValue } from '~/utils/actionSyntax'

const props = withDefaults(defineProps<{
  allowEmpty?: boolean
  placeholder?: string
  keyOnly?: boolean
  allowMacros?: boolean
  title?: string
  clearLabel?: string
  open?: boolean
  hideTrigger?: boolean
  requireValue?: boolean
  invalid?: boolean
  excludedMacroId?: string
  ghost?: boolean
}>(), {
  allowMacros: true,
  placeholder: undefined,
  title: undefined,
  clearLabel: undefined,
  excludedMacroId: undefined,
  ghost: false,
})

const emit = defineEmits<{
  'update:open': [value: boolean]
  apply: [value: string]
  cancel: []
  clear: []
}>()

const model = defineModel<string | null>({ default: '' })

const { getActionInfo } = useMacros()
const instance = getCurrentInstance()

const uncontrolledOpen = ref(false)
const draft = ref('')
const closeReason = ref<'apply' | 'clear' | 'cancel' | null>(null)

const actionInfo = computed(() => getActionInfo(model.value))
const isControlled = computed(() => {
  const vnodeProps = instance?.vnode.props ?? {}
  return Object.prototype.hasOwnProperty.call(vnodeProps, 'open')
    || Object.prototype.hasOwnProperty.call(vnodeProps, 'onUpdate:open')
})
const modalOpen = computed({
  get: () => (isControlled.value ? !!props.open : uncontrolledOpen.value),
  set: (value: boolean) => {
    if (isControlled.value) {
      emit('update:open', value)
      return
    }
    uncontrolledOpen.value = value
  },
})
const normalizedDraft = computed(() => {
  const raw = draft.value ?? ''
  return parseTextAction(raw) !== null ? raw : raw.trim()
})
const draftInvalid = computed(() => !isCanonicalAction(normalizedDraft.value))
const applyDisabled = computed(() =>
  (props.requireValue && !normalizedDraft.value)
  || draftInvalid.value,
)
const { t } = useI18n()
const pickerTitle = computed(() =>
  props.title ?? (props.keyOnly ? t('picker.titleKey') : t('picker.titleAction')),
)

watch(modalOpen, (isOpen, wasOpen) => {
  if (isOpen && !wasOpen) {
    draft.value = model.value ?? ''
    closeReason.value = null
    return
  }

  if (!isOpen && wasOpen) {
    const reason = closeReason.value ?? 'cancel'
    if (reason === 'cancel') emit('cancel')
    if (reason === 'apply' && model.value) emit('apply', model.value)
    if (reason === 'clear') emit('clear')
    closeReason.value = null
  }
})

function openModal() {
  modalOpen.value = true
}

function apply() {
  const next = normalizeActionValue(draft.value, props.requireValue)
  if (next === null) return
  model.value = next
  closeReason.value = 'apply'
  modalOpen.value = false
}

function pickAndApply(value: string) {
  draft.value = value
  const next = normalizeActionValue(value, props.requireValue)
  if (next === null) return
  model.value = next
  closeReason.value = 'apply'
  modalOpen.value = false
}

function clear() {
  draft.value = ''
  model.value = ''
  closeReason.value = 'clear'
  modalOpen.value = false
}

function cancel() {
  closeReason.value = 'cancel'
  modalOpen.value = false
}

</script>

<template>
  <div v-if="!props.hideTrigger" class="flex items-center gap-1 w-full">
    <UButton
      :variant="props.ghost && !model ? 'ghost' : 'outline'"
      color="neutral"
      class="flex-1 min-w-0 h-8 px-2.5 justify-start"
      :class="[
        props.invalid ? 'border-(--ui-error) ring-1 ring-(--ui-error)' : '',
        props.ghost && !model ? 'border border-dashed border-(--ui-border) text-(--ui-text-muted) hover:text-(--ui-text) hover:border-(--ui-border-accent) hover:bg-(--ui-bg-elevated)/50' : ''
      ]"
      @click="openModal"
    >
      <UIcon
        :name="actionInfo.icon || (model ? 'i-lucide-square-mouse-pointer' : 'i-lucide-plus')"
        class="shrink-0 w-4 h-4 text-(--ui-text-muted)"
      />
      <AppTooltip v-if="actionInfo.label" class="truncate min-w-0" :text="model ?? ''">
        <span>{{ actionInfo.label }}</span>
      </AppTooltip>
      <span v-else class="text-(--ui-text-muted) truncate">
        {{ placeholder ?? $t('picker.chooseAction') }}
      </span>
    </UButton>
    <FieldResetButton
      v-if="allowEmpty && model"
      :label="props.clearLabel ?? $t('common.clear')"
      @click="model = ''"
    />
  </div>

  <UModal v-model:open="modalOpen" fullscreen :ui="{ body: 'overflow-y-hidden' }">
    <template #header>
      <div class="mx-auto flex w-full max-w-7xl items-center gap-3">
        <UButton
          icon="i-lucide-arrow-left"
          color="neutral"
          variant="ghost"
          :aria-label="$t('common.cancel')"
          @click="cancel"
        />
        <div class="min-w-0 flex-1">
          <h2 class="truncate text-base font-semibold">{{ pickerTitle }}</h2>
          <p class="truncate text-xs text-(--ui-text-muted)">
            {{ draft || $t('picker.valuePh') }}
          </p>
        </div>
        <FieldResetButton
          v-if="allowEmpty && (model || draft)"
          :label="props.clearLabel ?? $t('common.clear')"
          @click="clear"
        />
        <UButton color="neutral" variant="ghost" @click="cancel">
          {{ $t('common.cancel') }}
        </UButton>
        <UButton icon="i-lucide-check" :disabled="applyDisabled" @click="apply">
          {{ $t('common.apply') }}
        </UButton>
      </div>
    </template>

    <template #body>
      <div
        v-if="modalOpen"
        class="mx-auto flex h-full w-full max-w-7xl flex-col"
        data-testid="action-picker-view"
      >
        <ActionPickerBody
          v-model="draft"
          :key-only="keyOnly"
          :allow-macros="allowMacros"
          :excluded-macro-id="excludedMacroId"
          spacious
          @pick="pickAndApply"
        />
        <p
          v-if="draftInvalid"
          class="mt-3 text-sm text-(--ui-error)"
        >
          {{ $t('picker.invalidValue') }}
        </p>
      </div>
    </template>
  </UModal>
</template>
