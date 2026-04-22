<script setup lang="ts">
import AppTooltip from '~/components/shared/AppTooltip.vue'

const props = defineProps<{
  allowEmpty?: boolean
  placeholder?: string
  keyOnly?: boolean
  title?: string
  clearLabel?: string
  open?: boolean
  hideTrigger?: boolean
  requireValue?: boolean
}>()

const emit = defineEmits<{
  'update:open': [value: boolean]
  apply: [value: string]
  cancel: []
  clear: []
}>()

const model = defineModel<string>({ default: '' })

const { displayAction } = useMacros()
const instance = getCurrentInstance()

const uncontrolledOpen = ref(false)
const draft = ref('')
const closeReason = ref<'apply' | 'clear' | 'cancel' | null>(null)

const displayLabel = computed(() => displayAction(model.value) || model.value)
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
const applyDisabled = computed(() => props.requireValue && !draft.value.trim())

watch(modalOpen, (isOpen, wasOpen) => {
  if (isOpen && !wasOpen) {
    draft.value = model.value
    closeReason.value = null
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
  const next = (draft.value ?? '').trim()
  if (props.requireValue && !next) return
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
    <button
      type="button"
      class="flex-1 min-w-0 h-8 px-2.5 flex items-center gap-2 rounded-md border border-(--ui-border) bg-(--ui-bg) hover:bg-(--ui-bg-elevated) text-left text-sm transition-colors"
      @click="openModal"
    >
      <UIcon
        :name="model ? 'i-lucide-square-mouse-pointer' : 'i-lucide-plus'"
        class="shrink-0 w-4 h-4 text-(--ui-text-muted)"
      />
      <AppTooltip v-if="displayLabel" class="truncate min-w-0" :text="model">
        <span>{{ displayLabel }}</span>
      </AppTooltip>
      <span v-else class="text-(--ui-text-muted) truncate">
        {{ placeholder ?? $t('picker.chooseAction') }}
      </span>
    </button>
    <UButton
      v-if="allowEmpty && model"
      :icon="props.clearLabel ? undefined : 'i-lucide-x'"
      size="xs"
      color="neutral"
      variant="ghost"
      :square="!props.clearLabel"
      :aria-label="$t('picker.clearAria')"
      @click="model = ''"
    >
      <template v-if="props.clearLabel">{{ props.clearLabel }}</template>
    </UButton>
  </div>

  <UModal
    v-model:open="modalOpen"
    :title="title ?? (keyOnly ? $t('picker.titleKey') : $t('picker.titleAction'))"
    :ui="{ content: 'max-w-3xl' }"
  >
    <template #body>
      <ActionPickerBody v-model="draft" :key-only="keyOnly" />
    </template>
    <template #footer>
      <div class="flex justify-between w-full gap-2">
        <UButton
          v-if="allowEmpty && (model || draft)"
          color="error"
          variant="ghost"
          icon="i-lucide-trash-2"
          @click="clear"
        >
          {{ props.clearLabel ?? $t('common.clear') }}
        </UButton>
        <div class="flex gap-2 ml-auto">
          <UButton color="neutral" variant="ghost" @click="cancel">
            {{ $t('common.cancel') }}
          </UButton>
          <UButton icon="i-lucide-check" :disabled="applyDisabled" @click="apply">
            {{ $t('common.apply') }}
          </UButton>
        </div>
      </div>
    </template>
  </UModal>
</template>
