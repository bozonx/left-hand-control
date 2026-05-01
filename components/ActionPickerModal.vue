<script setup lang="ts">
import AppTooltip from '~/components/shared/AppTooltip.vue'
import FieldResetButton from '~/components/shared/FieldResetButton.vue'
import { parseTextAction } from '~/types/config'
import { isCanonicalAction } from '~/utils/actionSyntax'

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
}>(), {
  allowMacros: true,
})

const emit = defineEmits<{
  'update:open': [value: boolean]
  apply: [value: string]
  cancel: []
  clear: []
}>()

const model = defineModel<string>({ default: '' })

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

const pickerRef = ref<HTMLElement | null>(null)

watch(modalOpen, (isOpen, wasOpen) => {
  if (isOpen && !wasOpen) {
    draft.value = model.value
    closeReason.value = null
    nextTick(() => pickerRef.value?.focus())
    return
  }

  if (!isOpen && wasOpen) {
    const reason = closeReason.value ?? 'cancel'
    if (reason === 'cancel') emit('cancel')
    if (reason === 'apply') emit('apply', model.value)
    if (reason === 'clear') emit('clear')
    closeReason.value = null
  }
})

function openModal() {
  modalOpen.value = true
}

function apply() {
  const next = normalizedDraft.value
  if (props.requireValue && !next) return
  if (!isCanonicalAction(next)) return
  model.value = next
  closeReason.value = 'apply'
  modalOpen.value = false
}

function pickAndApply(value: string) {
  draft.value = value
  const next = parseTextAction(value) !== null ? value : value.trim()
  if (props.requireValue && !next) return
  if (!isCanonicalAction(next)) return
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

const FOCUSABLE_SELECTOR = 'button, [href], input, select, textarea, [tabindex]:not([tabindex="-1"])'

function onDocumentTab(event: KeyboardEvent) {
  if (event.key !== 'Tab' || !modalOpen.value || !pickerRef.value) return
  const active = document.activeElement
  if (!active || !pickerRef.value.contains(active)) return
  const focusables = Array.from(pickerRef.value.querySelectorAll(FOCUSABLE_SELECTOR)) as HTMLElement[]
  if (focusables.length === 0) return
  const first = focusables[0]!
  const last = focusables[focusables.length - 1]!
  if (event.shiftKey && active === first) {
    event.preventDefault()
    last.focus()
  } else if (!event.shiftKey && active === last) {
    event.preventDefault()
    first.focus()
  }
}

watch(modalOpen, (isOpen) => {
  if (isOpen) {
    document.addEventListener('keydown', onDocumentTab, true)
  } else {
    document.removeEventListener('keydown', onDocumentTab, true)
  }
})

onBeforeUnmount(() => {
  document.removeEventListener('keydown', onDocumentTab, true)
})
</script>

<template>
  <div v-if="!props.hideTrigger" class="flex items-center gap-1 w-full">
    <button
      type="button"
      class="flex-1 min-w-0 h-8 px-2.5 flex items-center gap-2 rounded-md border border-(--ui-border) bg-(--ui-bg) hover:bg-(--ui-bg-elevated) text-left text-sm transition-colors"
      :class="props.invalid ? 'border-(--ui-error) ring-1 ring-(--ui-error)' : ''"
      @click="openModal"
    >
      <UIcon
        :name="actionInfo.icon || (model ? 'i-lucide-square-mouse-pointer' : 'i-lucide-plus')"
        class="shrink-0 w-4 h-4 text-(--ui-text-muted)"
      />
      <AppTooltip v-if="actionInfo.label" class="truncate min-w-0" :text="model">
        <span>{{ actionInfo.label }}</span>
      </AppTooltip>
      <span v-else class="text-(--ui-text-muted) truncate">
        {{ placeholder ?? $t('picker.chooseAction') }}
      </span>
    </button>
    <FieldResetButton
      v-if="allowEmpty && model"
      :label="props.clearLabel ?? $t('common.clear')"
      @click="model = ''"
    />
  </div>

  <Teleport to="body">
    <div
      v-if="modalOpen"
      ref="pickerRef"
      class="fixed inset-0 z-50 flex flex-col bg-(--ui-bg) text-(--ui-text) outline-none"
      role="dialog"
      aria-modal="true"
      :aria-label="pickerTitle"
      tabindex="-1"
      data-testid="action-picker-view"
      @keydown.esc="cancel"
    >
      <header class="shrink-0 border-b border-(--ui-border) bg-(--ui-bg-elevated)/80 px-4 py-3">
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
      </header>

      <main class="min-h-0 flex-1 overflow-hidden px-4 py-4">
        <div class="mx-auto flex h-full w-full max-w-7xl flex-col">
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
      </main>
    </div>
  </Teleport>
</template>
