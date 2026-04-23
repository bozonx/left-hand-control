<script setup lang="ts">
import FieldResetButton from '~/components/shared/FieldResetButton.vue'
import ResettableSelectMenu from '~/components/shared/ResettableSelectMenu.vue'

type ModeKind = 'native' | 'none' | 'action'

const props = defineProps<{
  placeholder: string
  keyOnly?: boolean
}>()

const model = defineModel<string | null>({ default: '' })
const { t } = useI18n()
const { displayAction } = useMacros()

const pickerOpen = ref(false)
const pickerValue = ref('')
const pendingMode = ref<ModeKind | null>(null)

const modeItems = computed(() => [
  { label: t('rules.modeNativeDefault'), value: 'native' },
  { label: t('rules.modeNone'), value: 'none' },
  { label: t('rules.modeAction'), value: 'action' },
])

const currentMode = computed<ModeKind>(() => {
  if (model.value === null) return 'none'
  if (model.value) return 'action'
  return 'native'
})

const selectMode = computed<ModeKind>({
  get: () => pendingMode.value ?? currentMode.value,
  set: (value) => {
    if (value === 'action') {
      pendingMode.value = 'action'
      pickerValue.value = typeof model.value === 'string' ? model.value : ''
      pickerOpen.value = true
      return
    }

    pendingMode.value = null
    pickerOpen.value = false
    model.value = value === 'none' ? null : ''
  },
})

const actionLabel = computed(() => displayAction(model.value) || model.value || '')

function resetToDefault() {
  pendingMode.value = null
  pickerOpen.value = false
  pickerValue.value = ''
  model.value = ''
}

function editAction() {
  pendingMode.value = 'action'
  pickerValue.value = typeof model.value === 'string' ? model.value : ''
  pickerOpen.value = true
}

function applyAction(value: string) {
  const next = value.trim()
  pendingMode.value = null
  pickerOpen.value = false
  if (!next) return
  model.value = next
}

function cancelAction() {
  pendingMode.value = null
  pickerOpen.value = false
  pickerValue.value = typeof model.value === 'string' ? model.value : ''
}
</script>

<template>
  <div class="space-y-1.5">
    <div v-if="currentMode === 'action'" class="flex items-center gap-1">
      <button
        type="button"
        class="flex-1 min-w-0 h-8 px-2.5 flex items-center gap-2 rounded-md border border-(--ui-border) bg-(--ui-bg) hover:bg-(--ui-bg-elevated) text-left text-sm transition-colors"
        @click="editAction"
      >
        <UIcon
          name="i-lucide-square-mouse-pointer"
          class="shrink-0 w-4 h-4 text-(--ui-text-muted)"
        />
        <span class="truncate">{{ actionLabel }}</span>
      </button>
      <FieldResetButton :label="$t('common.reset')" @click="resetToDefault" />
    </div>
    <ResettableSelectMenu
      v-else
      v-model="selectMode"
      :items="modeItems"
      value-key="value"
      :reset-value="'native'"
      :searchable="false"
    />
    <ActionPickerModal
      v-model="pickerValue"
      v-model:open="pickerOpen"
      hide-trigger
      :require-value="true"
      :key-only="props.keyOnly"
      :placeholder="placeholder"
      @apply="applyAction"
      @cancel="cancelAction"
    />
  </div>
</template>
