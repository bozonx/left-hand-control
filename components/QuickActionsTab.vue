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
const { getActionInfo } = useMacros()

const actions = computed(() => config.value.quickActions || [])
const quickActionPages = computed(() => config.value.quickActionPages || [])
const pickerValue = ref<string | null>('')
const selectedIndex = ref<number | null>(null)
const selectedPageIndex = ref(0)
const confirmDeletePageOpen = ref(false)
const pendingDeletePageIndex = ref<number | null>(null)
const deletePageConfirm = ref<{ $el?: HTMLButtonElement } | null>(null)
const pageSize = LEFT_HAND_HOTKEYS.length
const pageCount = computed(() =>
  Math.max(1, Math.ceil(actions.value.length / pageSize), quickActionPages.value.length),
)
const pageStart = computed(() => selectedPageIndex.value * pageSize)
const selectedPage = computed(() =>
  quickActionPages.value[selectedPageIndex.value] ?? null,
)
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
const selectedAction = computed(() => {
  if (selectedIndex.value === null) return null
  const action = actions.value[selectedIndex.value] ?? null
  return action?.action.trim() ? action : null
})
const selectedHotkey = computed(() => {
  if (selectedIndex.value === null) return null
  return LEFT_HAND_HOTKEYS[selectedIndex.value % pageSize] ?? null
})

function createPageMeta(n: number = (config.value.quickActionPages?.length ?? 0) + 1) {
  return {
    id: crypto.randomUUID(),
    name: t('quickActions.pageName', { n }),
  }
}

function ensurePages() {
  if (!config.value.quickActionPages) {
    config.value.quickActionPages = []
  }
  while (config.value.quickActionPages.length < pageCount.value) {
    config.value.quickActionPages.push(createPageMeta())
  }
  if (selectedPageIndex.value >= config.value.quickActionPages.length) {
    selectedPageIndex.value = Math.max(0, config.value.quickActionPages.length - 1)
  }
}

function clampSelection() {
  ensurePages()
  if (actions.value.length === 0) {
    selectedIndex.value = null
    selectedPageIndex.value = Math.min(selectedPageIndex.value, pageCount.value - 1)
    return
  }
  if (
    selectedIndex.value === null ||
    selectedIndex.value >= actions.value.length ||
    !actions.value[selectedIndex.value]?.action.trim()
  ) {
    const firstFilledIndex = actions.value.findIndex((action) => action.action.trim())
    selectedIndex.value = firstFilledIndex === -1 ? null : firstFilledIndex
  }
  if (selectedIndex.value === null) {
    selectedPageIndex.value = Math.min(selectedPageIndex.value, pageCount.value - 1)
    return
  }
  selectedPageIndex.value = Math.min(
    Math.floor(selectedIndex.value / pageSize),
    pageCount.value - 1,
  )
}

function createEmptyAction(): QuickAction {
  return {
    id: crypto.randomUUID(),
    name: t('quickActions.defaultName'),
    action: '',
  }
}

function addPage() {
  if (!config.value.quickActions) {
    config.value.quickActions = []
  }
  ensurePages()
  const newPageIndex = config.value.quickActionPages.length
  const targetLength = (newPageIndex + 1) * pageSize
  while (config.value.quickActions.length < targetLength) {
    config.value.quickActions.push(createEmptyAction())
  }
  config.value.quickActionPages.push(createPageMeta())
  selectedPageIndex.value = newPageIndex
  selectedIndex.value = null
}

function removePage(index: number) {
  ensurePages()
  if (!config.value.quickActions || !config.value.quickActionPages) return
  if (config.value.quickActionPages.length <= 1) {
    config.value.quickActions = []
    config.value.quickActionPages = [createPageMeta(1)]
    selectedPageIndex.value = 0
    selectedIndex.value = null
    return
  }
  config.value.quickActions.splice(index * pageSize, pageSize)
  config.value.quickActionPages.splice(index, 1)
  selectedPageIndex.value = Math.min(index, config.value.quickActionPages.length - 1)
  selectedIndex.value = null
  clampSelection()
}

function askRemovePage(index: number) {
  pendingDeletePageIndex.value = index
  confirmDeletePageOpen.value = true
}

function confirmRemovePage() {
  if (pendingDeletePageIndex.value !== null) {
    removePage(pendingDeletePageIndex.value)
  }
  pendingDeletePageIndex.value = null
  confirmDeletePageOpen.value = false
}

function cancelRemovePage() {
  pendingDeletePageIndex.value = null
  confirmDeletePageOpen.value = false
}

function actionNameFor(action: string): string {
  return getActionInfo(action).label || action || t('quickActions.defaultName')
}

function setActionAt(index: number, action: string): number {
  const newAction: QuickAction = {
    id: crypto.randomUUID(),
    name: actionNameFor(action),
    action,
  }
  if (!config.value.quickActions) {
    config.value.quickActions = []
  }
  while (config.value.quickActions.length < index) {
    config.value.quickActions.push(createEmptyAction())
  }
  if (config.value.quickActions[index]) {
    config.value.quickActions[index] = newAction
  } else {
    config.value.quickActions.push(newAction)
  }
  return index
}

function clearAction(index: number) {
  if (!config.value.quickActions) return
  config.value.quickActions[index] = createEmptyAction()
  selectedIndex.value = null
  clampSelection()
}

const pickerState = ref<{
  isOpen: boolean
  index: number | null
  insertIndex: number | null
}>({
  isOpen: false,
  index: null,
  insertIndex: null,
})

function openPicker(index: number, currentValue: string) {
  pickerValue.value = currentValue
  pickerState.value = {
    isOpen: true,
    index,
    insertIndex: null,
  }
}

function openEmptyCell(index: number) {
  pickerValue.value = ''
  pickerState.value = {
    isOpen: true,
    index: null,
    insertIndex: index,
  }
}

function onPickerApply(value: string) {
  if (pickerState.value.index !== null) {
    if (config.value.quickActions) {
      const action = config.value.quickActions[pickerState.value.index]
      action.action = value
      if (!action.name.trim() || action.name === t('quickActions.defaultName')) {
        action.name = actionNameFor(value)
      }
    }
  } else {
    selectedIndex.value = setActionAt(
      pickerState.value.insertIndex ?? actions.value.length,
      value,
    )
    selectedPageIndex.value = Math.floor(selectedIndex.value / pageSize)
  }
  pickerState.value.isOpen = false
}

function onPickerCancel() {
  pickerState.value.isOpen = false
}

function selectAction(index: number) {
  selectedIndex.value = index
}

function onCellClick(index: number, action: QuickAction | null) {
  if (action?.action.trim()) {
    selectAction(index)
  } else {
    openEmptyCell(index)
  }
}

function setPage(index: number) {
  ensurePages()
  selectedPageIndex.value = index
  const firstActionIndex = index * pageSize
  if (actions.value[firstActionIndex]) {
    selectedIndex.value = firstActionIndex
  }
}

watch(actions, clampSelection, { immediate: true })
watch(confirmDeletePageOpen, async (open) => {
  if (!open) return
  await nextTick()
  deletePageConfirm.value?.$el?.focus()
})
onMounted(ensurePages)
</script>

<template>
  <div class="space-y-4">
    <UCard>
      <template #header>
        <div class="flex items-center justify-between gap-3">
          <div class="min-w-0">
            <h2 class="text-sm font-semibold">{{ $t('quickActions.title') }}</h2>
            <p class="mt-0.5 text-xs text-(--ui-text-muted)">
              {{ $t('quickActions.subtitle') }}
            </p>
          </div>
          <UButton
            icon="i-lucide-plus"
            size="sm"
            class="whitespace-nowrap"
            @click="addPage"
          >
            {{ $t('quickActions.addPage') }}
          </UButton>
        </div>
      </template>

      <div class="space-y-4">
        <div class="grid gap-4 xl:grid-cols-[minmax(0,1fr)_360px]">
          <div class="space-y-3">
            <div class="flex flex-wrap gap-2">
              <div class="flex h-8 items-center px-1 text-xs font-medium text-(--ui-text-muted)">
                {{ $t('common.page') }}
              </div>
              <UButton
                v-for="(page, index) in quickActionPages"
                :key="page.id"
                :color="index === selectedPageIndex ? 'primary' : 'neutral'"
                :variant="index === selectedPageIndex ? 'soft' : 'outline'"
                size="sm"
                @click="setPage(index)"
              >
                {{ page.name }}
              </UButton>
            </div>

            <div class="grid grid-cols-5 gap-2 rounded-lg border border-(--ui-border-muted) bg-(--ui-bg-elevated) p-3">
              <button
                v-for="item in pageItems"
                :key="item.key"
                type="button"
                class="flex h-28 min-w-0 flex-col items-start gap-2 rounded-md border bg-(--ui-bg) p-3 text-left transition hover:border-primary hover:bg-primary/10"
                :class="
                  item.actionIndex === selectedIndex
                    ? 'border-primary ring-1 ring-primary/35'
                    : 'border-(--ui-border-muted)'
                "
                @click="onCellClick(item.actionIndex, item.action)"
              >
                <span class="font-mono text-xs uppercase text-(--ui-primary)">
                  {{ LEFT_HAND_HOTKEY_LABELS[item.key as LeftHandHotkey] }}
                </span>
                <span class="flex min-w-0 items-center gap-2 self-stretch">
                  <UIcon
                    v-if="item.action?.action.trim()"
                    :name="item.action.icon || 'i-lucide-zap'"
                    class="h-4 w-4 shrink-0 text-(--ui-text-muted)"
                  />
                  <span class="min-w-0 truncate text-sm font-medium">
                    {{ item.action?.action.trim() ? item.action.name : $t('quickActions.emptyCell') }}
                  </span>
                </span>
                <span class="min-w-0 self-stretch truncate font-mono text-[11px] text-(--ui-text-muted)">
                  {{ item.action?.action || '—' }}
                </span>
              </button>
            </div>
          </div>

          <UCard v-if="selectedPage">
            <template #header>
              <div class="flex items-center justify-between gap-2">
                <UFormField class="min-w-0 flex-1">
                  <template #label>
                    <FieldLabel :label="$t('common.page')" />
                  </template>
                  <UInput
                    v-model="selectedPage.name"
                    size="sm"
                    :aria-label="$t('quickActions.pageLabel')"
                    class="w-full"
                  />
                </UFormField>
                <UButton
                  icon="i-lucide-trash-2"
                  color="neutral"
                  variant="ghost"
                  size="sm"
                  :title="$t('quickActions.deletePageTitle')"
                  :aria-label="$t('quickActions.deletePage')"
                  @click="askRemovePage(selectedPageIndex)"
                />
              </div>
            </template>

            <div v-if="selectedAction" class="space-y-3">
              <div>
                <h3 class="truncate text-sm font-semibold">
                  {{ $t('quickActions.cellLabel', {
                    key: selectedHotkey ? LEFT_HAND_HOTKEY_LABELS[selectedHotkey] : '',
                  }) }}
                </h3>
                <p class="mt-0.5 text-xs text-(--ui-text-muted)">
                  {{ $t('quickActions.cellHint') }}
                </p>
              </div>

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
                <div class="flex gap-2">
                  <UButton
                    color="neutral"
                    variant="subtle"
                    class="h-9 min-w-0 flex-1 justify-start overflow-hidden font-mono text-xs"
                    :icon="selectedAction.action ? undefined : 'i-lucide-plus'"
                    @click="selectedIndex !== null && openPicker(selectedIndex, selectedAction.action)"
                  >
                    <span class="truncate">
                      {{ selectedAction.action || $t('quickActions.actionPh') }}
                    </span>
                  </UButton>
                  <UButton
                    icon="i-lucide-eraser"
                    variant="ghost"
                    color="neutral"
                    square
                    :aria-label="$t('quickActions.clearAction')"
                    @click="selectedIndex !== null && clearAction(selectedIndex)"
                  />
                </div>
              </UFormField>
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

    <UModal v-model:open="confirmDeletePageOpen" :title="$t('quickActions.confirmDeletePageTitle')">
      <template #body>
        <p class="text-sm">{{ $t('quickActions.confirmDeletePageBody') }}</p>
      </template>
      <template #footer>
        <div class="flex w-full justify-end gap-2">
          <UButton color="neutral" variant="ghost" @click="cancelRemovePage">
            {{ $t('common.cancel') }}
          </UButton>
          <UButton
            ref="deletePageConfirm"
            color="error"
            icon="i-lucide-trash-2"
            @click="confirmRemovePage"
          >
            {{ $t('common.delete') }}
          </UButton>
        </div>
      </template>
    </UModal>
  </div>
</template>
