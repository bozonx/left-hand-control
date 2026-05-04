<script setup lang="ts">
import { useConfig } from '~/composables/useConfig'
import type { QuickAction } from '~/types/config'
import ActionPickerModal from '~/components/ActionPickerModal.vue'

const { config, applyConfigUpdate } = useConfig()
const { t } = useI18n()
const toast = useToast()

const actions = computed(() => config.value.quickActions)

function addAction() {
  const newAction: QuickAction = {
    id: crypto.randomUUID(),
    name: 'New Action',
    action: '',
  }
  applyConfigUpdate((draft) => {
    if (!draft.quickActions) draft.quickActions = []
    draft.quickActions.push(newAction)
  })
}

function updateAction(index: number, updates: Partial<QuickAction>) {
  applyConfigUpdate((draft) => {
    if (!draft.quickActions) return
    draft.quickActions[index] = { ...draft.quickActions[index]!, ...updates }
  })
}

function removeAction(index: number) {
  applyConfigUpdate((draft) => {
    if (!draft.quickActions) return
    draft.quickActions.splice(index, 1)
  })
}

function moveAction(index: number, delta: number) {
  const newIndex = index + delta
  if (newIndex < 0 || newIndex >= actions.value.length) return
  applyConfigUpdate((draft) => {
    if (!draft.quickActions) return
    const item = draft.quickActions[index]!
    draft.quickActions.splice(index, 1)
    draft.quickActions.splice(newIndex, 0, item)
  })
}

// Action picker state
const pickerState = ref<{
  isOpen: boolean
  index: number | null
  currentValue: string
}>({
  isOpen: false,
  index: null,
  currentValue: '',
})

function openPicker(index: number, currentValue: string) {
  pickerState.value = {
    isOpen: true,
    index,
    currentValue,
  }
}

function onPickerSelect(value: string | null) {
  if (pickerState.value.index !== null) {
    updateAction(pickerState.value.index, { action: value || '' })
  }
  pickerState.value.isOpen = false
}
</script>

<template>
  <div class="p-6 max-w-5xl mx-auto flex flex-col gap-6 h-[calc(100vh-var(--app-header-height))] overflow-y-auto min-h-0">
    <div class="flex items-start justify-between gap-4">
      <div>
        <h1 class="text-2xl font-bold tracking-tight text-(--ui-text-highlighted) mb-1">
          {{ $t('quickActions.title') }}
        </h1>
        <p class="text-(--ui-text-muted)">
          {{ $t('quickActions.subtitle') }}
        </p>
      </div>
      <UButton icon="i-lucide-plus" @click="addAction">
        {{ $t('quickActions.addBtn') }}
      </UButton>
    </div>

    <div v-if="actions.length === 0" class="flex flex-col items-center justify-center p-12 text-center bg-(--ui-bg-elevated) border border-(--ui-border-muted) rounded-lg border-dashed">
      <UIcon name="i-lucide-zap-fast" class="w-12 h-12 text-(--ui-text-muted) mb-4 opacity-50" />
      <p class="text-(--ui-text-muted)">
        {{ $t('quickActions.empty') }}
      </p>
    </div>

    <div v-else class="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4">
      <div v-for="(action, i) in actions" :key="action.id" class="flex flex-col gap-3 p-4 rounded-lg bg-(--ui-bg-elevated) border border-(--ui-border) shadow-sm">
        <div class="flex justify-between items-start gap-2">
          <UInput
            :model-value="action.name"
            :placeholder="$t('quickActions.namePh')"
            size="sm"
            class="flex-1 font-semibold"
            @update:model-value="updateAction(i, { name: $event })"
          />
          <div class="flex gap-1 shrink-0">
            <UButton
              color="neutral"
              variant="ghost"
              icon="i-lucide-arrow-left"
              size="xs"
              :disabled="i === 0"
              :aria-label="$t('quickActions.moveUp')"
              @click="moveAction(i, -1)"
            />
            <UButton
              color="neutral"
              variant="ghost"
              icon="i-lucide-arrow-right"
              size="xs"
              :disabled="i === actions.length - 1"
              :aria-label="$t('quickActions.moveDown')"
              @click="moveAction(i, 1)"
            />
            <UButton
              color="error"
              variant="ghost"
              icon="i-lucide-trash-2"
              size="xs"
              :aria-label="$t('quickActions.deleteAction')"
              @click="removeAction(i)"
            />
          </div>
        </div>

        <div>
          <label class="text-xs text-(--ui-text-muted) mb-1 block">{{ $t('quickActions.iconLabel') }}</label>
          <UInput
            :model-value="action.icon"
            :placeholder="$t('quickActions.iconPh')"
            size="sm"
            icon="i-lucide-image"
            @update:model-value="updateAction(i, { icon: $event })"
          />
        </div>

        <div>
          <label class="text-xs text-(--ui-text-muted) mb-1 block">{{ $t('quickActions.actionLabel') }}</label>
          <UButton
            color="neutral"
            variant="subtle"
            size="sm"
            class="w-full justify-start font-mono text-xs overflow-hidden"
            :icon="action.action ? undefined : 'i-lucide-plus'"
            @click="openPicker(i, action.action)"
          >
            <span class="truncate">
              {{ action.action || $t('quickActions.actionPh') }}
            </span>
          </UButton>
        </div>
      </div>
    </div>

    <ActionPickerModal
      v-model:open="pickerState.isOpen"
      :current-value="pickerState.currentValue"
      @select="onPickerSelect"
    />
  </div>
</template>