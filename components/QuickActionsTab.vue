<script setup lang="ts">
import type { QuickAction } from '~/types/config'
import ActionPickerModal from '~/components/ActionPickerModal.vue'
import QuickActionEditorCard from '~/components/features/quickActions/QuickActionEditorCard.vue'

const { config } = useConfig()
const { t } = useI18n()

const actions = computed(() => config.value.quickActions || [])

function addAction() {
  const newAction: QuickAction = {
    id: crypto.randomUUID(),
    name: t('quickActions.defaultName'),
    action: '',
  }
  if (!config.value.quickActions) {
    config.value.quickActions = [newAction]
  } else {
    config.value.quickActions.push(newAction)
  }
}

function removeAction(index: number) {
  if (!config.value.quickActions) return
  config.value.quickActions.splice(index, 1)
}

function moveAction(index: number, delta: number) {
  const newIndex = index + delta
  if (!config.value.quickActions || newIndex < 0 || newIndex >= config.value.quickActions.length) return

  const arr = [...config.value.quickActions]
  const item = arr[index]
  if (!item) return
  arr.splice(index, 1)
  arr.splice(newIndex, 0, item)
  config.value.quickActions = arr
}

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
  if (pickerState.value.index !== null && config.value.quickActions) {
    config.value.quickActions[pickerState.value.index].action = value || ''
  }
  pickerState.value.isOpen = false
}
</script>

<template>
  <div class="space-y-4">
    <UCard>
      <template #header>
        <div class="flex items-center justify-between gap-3">
          <div>
            <h2 class="text-sm font-semibold">{{ $t('quickActions.title') }}</h2>
            <p class="mt-0.5 text-xs text-(--ui-text-muted)">
              {{ $t('quickActions.subtitle') }}
            </p>
          </div>
          <UButton
            icon="i-lucide-plus"
            size="sm"
            class="whitespace-nowrap"
            @click="addAction"
          >
            {{ $t('quickActions.addBtn') }}
          </UButton>
        </div>
      </template>

      <div class="space-y-4">
        <div v-if="actions.length === 0" class="flex flex-col items-center justify-center p-12 text-center bg-(--ui-bg-elevated) border border-(--ui-border-muted) rounded-lg border-dashed">
          <UIcon name="i-lucide-zap" class="w-12 h-12 text-(--ui-text-muted) mb-4 opacity-50" />
          <p class="text-(--ui-text-muted)">
            {{ $t('quickActions.empty') }}
          </p>
        </div>

        <template v-else>
          <QuickActionEditorCard
            v-for="(_, i) in actions"
            :key="actions[i].id"
            v-model:action="config.quickActions[i]"
            :is-first="i === 0"
            :is-last="i === actions.length - 1"
            @remove="removeAction(i)"
            @move-up="moveAction(i, -1)"
            @move-down="moveAction(i, 1)"
            @pick-action="openPicker(i, actions[i].action)"
          />
        </template>
      </div>
    </UCard>

    <ActionPickerModal
      v-model:open="pickerState.isOpen"
      :current-value="pickerState.currentValue"
      @select="onPickerSelect"
    />
  </div>
</template>
