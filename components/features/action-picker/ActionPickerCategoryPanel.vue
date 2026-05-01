<script setup lang="ts">
import { textActionRef } from '~/types/config'
import type { ActionItem } from '~/utils/actionCategories'

const props = defineProps<{
  activeCategory: string
  draft: string
  items: ActionItem[]
  spacious?: boolean
}>()

const emit = defineEmits<{
  pick: [item: ActionItem]
}>()

const isTextCategory = computed(() => props.activeCategory === 'text')
const showPhysicalKeyHint = computed(() => ['lettersSymbols'].includes(props.activeCategory))
const listGridClass = computed(() =>
  ['commands', 'macros', 'system-macros', 'system'].includes(props.activeCategory)
    ? [
        'grid grid-cols-1 sm:grid-cols-2 xl:grid-cols-3 gap-1.5 overflow-y-auto pr-1',
        props.spacious ? 'min-h-0 flex-1 content-start' : 'max-h-80',
      ].join(' ')
    : [
        'grid grid-cols-2 sm:grid-cols-4 xl:grid-cols-6 gap-1.5 overflow-y-auto pr-1',
        props.spacious ? 'min-h-0 flex-1 content-start' : 'max-h-80',
      ].join(' '),
)
</script>

<template>
  <div
    v-if="isTextCategory"
    class="rounded-lg border border-(--ui-border) bg-(--ui-bg-elevated)/40 p-4"
  >
    <div class="space-y-3">
      <p class="text-sm text-(--ui-text-muted)">{{ $t('picker.textTabBody') }}</p>
      <div class="rounded-md border border-(--ui-border) bg-(--ui-bg) px-3 py-2 font-mono text-sm">
        {{ props.draft || textActionRef('') }}
      </div>
    </div>
  </div>
  <div
    v-else-if="props.items.length === 0"
    class="text-sm text-(--ui-text-muted) italic px-1 py-6 text-center"
  >
    {{ $t('picker.emptyCategory') }}
  </div>
  <div
    v-else
    :class="listGridClass"
  >
    <button
      v-for="item in props.items"
      :key="item.value"
      type="button"
      class="text-left px-2.5 py-1.5 rounded-md border text-sm transition-colors hover:bg-(--ui-bg-elevated)"
      :class="props.draft === item.value
        ? 'border-(--ui-primary) bg-(--ui-primary)/10'
        : 'border-(--ui-border) bg-(--ui-bg)'"
      @click="emit('pick', item)"
    >
      <div class="truncate font-medium">{{ item.label }}</div>
      <div
        v-if="item.hint && item.hint !== item.label"
        class="truncate text-[11px] text-(--ui-text-muted) font-mono"
      >
        {{ item.hint }}
      </div>
    </button>
  </div>

  <p
    v-if="showPhysicalKeyHint"
    class="text-xs text-(--ui-text-muted)"
  >
    {{ $t('picker.physicalKeyHint') }}
  </p>
</template>
