<script setup lang="ts">
import FieldResetButton from '~/components/shared/FieldResetButton.vue'
import ResettableSelectMenu from '~/components/shared/ResettableSelectMenu.vue'
import { parseTextAction } from '~/types/config'
import { isCanonicalAction } from '~/utils/actionSyntax'

type ModeKind = 'native' | 'none' | 'action'

const props = defineProps<{
  placeholder: string
  keyOnly?: boolean
  modeKind?: 'tap' | 'hold'
  ghost?: boolean
}>()

const model = defineModel<string | null>({ default: '' })
const { t } = useI18n()
const { getActionInfo } = useMacros()

const pickerOpen = ref(false)
const pickerValue = ref('')
const pendingMode = ref<ModeKind | null>(null)
const ghostOpen = ref(false)

const modeItems = computed(() => {
  if (props.modeKind === 'hold') {
    return [
      { label: t('rules.holdModeNativeDefault'), value: 'native' },
      { label: t('rules.holdModeNone'), value: 'none' },
      { label: t('rules.holdModeAction'), value: 'action' },
    ]
  }

  return [
    { label: t('rules.modeNativeDefault'), value: 'native' },
    { label: t('rules.modeNone'), value: 'none' },
    { label: t('rules.modeAction'), value: 'action' },
  ]
})

const currentMode = computed<ModeKind>(() => {
  if (model.value === null) return 'none'
  if (model.value) return 'action'
  return 'native'
})

const showGhost = computed(() => props.ghost && (currentMode.value === 'native' || currentMode.value === 'none'))

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

const actionInfo = computed(() => getActionInfo(model.value))

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
  const next = parseTextAction(value) !== null ? value : value.trim()
  pendingMode.value = null
  pickerOpen.value = false
  if (!next) return
  if (!isCanonicalAction(next)) return
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
          :name="actionInfo.icon || 'i-lucide-square-mouse-pointer'"
          class="shrink-0 w-4 h-4 text-(--ui-text-muted)"
        />
        <span class="truncate">{{ actionInfo.label || model }}</span>
      </button>
      <FieldResetButton :label="$t('common.reset')" @click="resetToDefault" />
    </div>
    <template v-else-if="showGhost && !ghostOpen">
      <UButton
        variant="ghost"
        color="neutral"
        class="flex-1 min-w-0 h-8 px-2.5 justify-start border border-dashed border-(--ui-border) text-(--ui-text-muted) hover:text-(--ui-text) hover:border-(--ui-border-accent) hover:bg-(--ui-bg-elevated)/50"
        @click="ghostOpen = true"
      >
        <UIcon name="i-lucide-plus" class="shrink-0 w-4 h-4 mr-1.5" />
        <span class="truncate">{{ props.placeholder }}</span>
      </UButton>
    </template>
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
