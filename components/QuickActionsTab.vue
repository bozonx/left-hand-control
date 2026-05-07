<script setup lang="ts">
import {
  LEFT_HAND_HOTKEYS,
  LEFT_HAND_HOTKEY_LABELS,
  type LeftHandHotkey,
  type QuickAction,
} from '~/types/config'
import ActionPickerModal from '~/components/ActionPickerModal.vue'

const { config } = useConfig()
const { t } = useI18n()

const actions = computed(() => config.value.quickActions || [])
const pickerValue = ref<string | null>('')
const selectedIndex = ref<number | null>(null)
const selectedPageIndex = ref(0)
const pageSize = LEFT_HAND_HOTKEYS.length
const pageCount = computed(() =>
  Math.max(1, Math.ceil(actions.value.length / pageSize)),
)
const pageStart = computed(() => selectedPageIndex.value * pageSize)
const pageItems = computed(() =>
  LEFT_HAND_HOTKEYS.map((key, cellIndex) => {
    const actionIndex = pageStart.value + cellIndex
    return {
      key,
      actionIndex,
      action: actions.value[actionIndex] ?? null,
    }
  }),
)
const selectedAction = computed(() =>
  selectedIndex.value === null ? null : actions.value[selectedIndex.value] ?? null,
)
const selectedHotkey = computed(() => {
  if (selectedIndex.value === null) return null
  return LEFT_HAND_HOTKEYS[selectedIndex.value % pageSize] ?? null
})

function clampSelection() {
  if (actions.value.length === 0) {
    selectedIndex.value = null
    selectedPageIndex.value = 0
    return
  }
  if (selectedIndex.value === null || selectedIndex.value >= actions.value.length) {
    selectedIndex.value = actions.value.length - 1
  }
  selectedPageIndex.value = Math.min(
    Math.floor(selectedIndex.value / pageSize),
    pageCount.value - 1,
  )
}

function appendAction(action: string): number {
  const newAction: QuickAction = {
    id: crypto.randomUUID(),
    name: t('quickActions.defaultName'),
    action,
  }
  if (!config.value.quickActions) {
    config.value.quickActions = [newAction]
  } else {
    config.value.quickActions.push(newAction)
  }
  return config.value.quickActions.length - 1
}

function removeAction(index: number) {
  if (!config.value.quickActions) return
  config.value.quickActions.splice(index, 1)
  if (selectedIndex.value === index) {
    selectedIndex.value = Math.min(index, config.value.quickActions.length - 1)
  } else if (selectedIndex.value !== null && selectedIndex.value > index) {
    selectedIndex.value -= 1
  }
  clampSelection()
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
  selectedIndex.value = newIndex
  selectedPageIndex.value = Math.floor(newIndex / pageSize)
}

const pickerState = ref<{
  isOpen: boolean
  index: number | null
}>({
  isOpen: false,
  index: null,
})

function addAction() {
  pickerValue.value = ''
  pickerState.value = {
    isOpen: true,
    index: null,
  }
}

function openPicker(index: number, currentValue: string) {
  pickerValue.value = currentValue
  pickerState.value = {
    isOpen: true,
    index,
  }
}

function onPickerApply(value: string) {
  if (pickerState.value.index === null) {
    selectedIndex.value = appendAction(value)
    selectedPageIndex.value = Math.floor(selectedIndex.value / pageSize)
  } else if (config.value.quickActions) {
    config.value.quickActions[pickerState.value.index].action = value
  }
  pickerState.value.isOpen = false
}

function onPickerCancel() {
  pickerState.value.isOpen = false
}

function selectAction(index: number) {
  selectedIndex.value = index
}

function setPage(index: number) {
  selectedPageIndex.value = index
  const firstActionIndex = index * pageSize
  if (actions.value[firstActionIndex]) {
    selectedIndex.value = firstActionIndex
  }
}

watch(actions, clampSelection, { immediate: true })
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

        <div v-else class="grid gap-4 xl:grid-cols-[minmax(0,1fr)_360px]">
          <div class="space-y-3">
            <div class="flex flex-wrap items-center gap-2">
              <UButton
                v-for="index in pageCount"
                :key="index"
                :color="index - 1 === selectedPageIndex ? 'primary' : 'neutral'"
                :variant="index - 1 === selectedPageIndex ? 'soft' : 'outline'"
                size="sm"
                @click="setPage(index - 1)"
              >
                {{ $t('quickActions.pageName', { n: index }) }}
              </UButton>
            </div>

            <div class="grid grid-cols-5 gap-2 rounded-lg border border-(--ui-border-muted) bg-(--ui-bg-elevated) p-3">
              <button
                v-for="item in pageItems"
                :key="item.key"
                type="button"
                class="flex h-28 min-w-0 flex-col items-start justify-between rounded-md border bg-(--ui-bg) p-3 text-left transition hover:border-primary hover:bg-primary/10 disabled:cursor-default disabled:opacity-45"
                :class="
                  item.actionIndex === selectedIndex
                    ? 'border-primary ring-1 ring-primary/35'
                    : 'border-(--ui-border-muted)'
                "
                :disabled="!item.action"
                @click="item.action && selectAction(item.actionIndex)"
              >
                <span class="flex min-w-0 items-center gap-2 self-stretch">
                  <UIcon
                    :name="item.action?.icon || 'i-lucide-zap'"
                    class="h-4 w-4 shrink-0 text-(--ui-text-muted)"
                  />
                  <span class="min-w-0 truncate text-sm font-medium">
                    {{ item.action?.name || $t('quickActions.emptyCell') }}
                  </span>
                </span>
                <span class="min-w-0 self-stretch truncate font-mono text-[11px] text-(--ui-text-muted)">
                  {{ item.action?.action || '—' }}
                </span>
                <span class="font-mono text-xs uppercase text-(--ui-primary)">
                  {{ LEFT_HAND_HOTKEY_LABELS[item.key as LeftHandHotkey] }}
                </span>
              </button>
            </div>
          </div>

          <UCard v-if="selectedAction">
            <template #header>
              <div class="flex items-center justify-between gap-2">
                <div class="min-w-0">
                  <h3 class="truncate text-sm font-semibold">
                    {{ $t('quickActions.cellLabel', {
                      key: selectedHotkey ? LEFT_HAND_HOTKEY_LABELS[selectedHotkey] : '',
                    }) }}
                  </h3>
                  <p class="mt-0.5 text-xs text-(--ui-text-muted)">
                    {{ $t('quickActions.cellHint') }}
                  </p>
                </div>
                <UBadge color="primary" variant="soft" size="sm">
                  {{ selectedIndex !== null ? selectedIndex + 1 : '' }}
                </UBadge>
              </div>
            </template>

            <div class="space-y-3">
              <UFormField>
                <template #label>
                  <FieldLabel :label="$t('quickActions.nameLabel')" />
                </template>
                <UInput
                  v-model="selectedAction.name"
                  :placeholder="$t('quickActions.namePh')"
                  class="w-full"
                />
              </UFormField>

              <UFormField>
                <template #label>
                  <FieldLabel :label="$t('quickActions.actionLabel')" />
                </template>
                <UButton
                  color="neutral"
                  variant="subtle"
                  class="h-9 w-full justify-start overflow-hidden font-mono text-xs"
                  :icon="selectedAction.action ? undefined : 'i-lucide-plus'"
                  @click="selectedIndex !== null && openPicker(selectedIndex, selectedAction.action)"
                >
                  <span class="truncate">
                    {{ selectedAction.action || $t('quickActions.actionPh') }}
                  </span>
                </UButton>
              </UFormField>

              <div class="flex justify-end gap-1 border-t border-(--ui-border-muted) pt-3">
                <UButton
                  icon="i-lucide-arrow-up"
                  variant="ghost"
                  color="neutral"
                  size="sm"
                  square
                  :disabled="selectedIndex === null || selectedIndex === 0"
                  :aria-label="$t('quickActions.moveUp')"
                  @click="selectedIndex !== null && moveAction(selectedIndex, -1)"
                />
                <UButton
                  icon="i-lucide-arrow-down"
                  variant="ghost"
                  color="neutral"
                  size="sm"
                  square
                  :disabled="selectedIndex === null || selectedIndex === actions.length - 1"
                  :aria-label="$t('quickActions.moveDown')"
                  @click="selectedIndex !== null && moveAction(selectedIndex, 1)"
                />
                <UButton
                  icon="i-lucide-trash-2"
                  variant="ghost"
                  color="error"
                  size="sm"
                  square
                  :aria-label="$t('quickActions.deleteAction')"
                  @click="selectedIndex !== null && removeAction(selectedIndex)"
                />
              </div>
            </div>
          </UCard>
        </div>
      </div>
    </UCard>

    <ActionPickerModal
      v-model="pickerValue"
      v-model:open="pickerState.isOpen"
      hide-trigger
      require-value
      @apply="onPickerApply"
      @cancel="onPickerCancel"
    />
  </div>
</template>
