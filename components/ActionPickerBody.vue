<script setup lang="ts">
import { macroActionRef, systemActionRef } from '~/types/config'
import { SYSTEM_ACTIONS } from '~/utils/systemActions'
import {
  STATIC_CATEGORIES,
  type ActionItem,
  type StaticCategory,
} from '~/utils/actionCategories'

const props = defineProps<{
  keyOnly?: boolean
}>()

const draft = defineModel<string>({ default: '' })

const { macros } = useMacros()

const dynamicCategories = computed<StaticCategory[]>(() => [
  {
    id: 'macros',
    label: 'Макросы',
    icon: 'i-lucide-zap',
    items: macros.value.map((m) => ({
      label: m.name || m.id,
      value: macroActionRef(m.id),
      hint: m.id,
    })),
  },
  {
    id: 'system',
    label: 'Системные',
    icon: 'i-lucide-settings-2',
    items: SYSTEM_ACTIONS.map((a) => ({
      label: a.name,
      value: systemActionRef(a.id),
      hint: a.id,
    })),
  },
])

const allCategories = computed<StaticCategory[]>(() =>
  props.keyOnly
    ? STATIC_CATEGORIES
    : [...dynamicCategories.value, ...STATIC_CATEGORIES],
)

const activeCategory = ref<string>(allCategories.value[0]?.id ?? 'special')

watchEffect(() => {
  if (!allCategories.value.some((c) => c.id === activeCategory.value)) {
    activeCategory.value = allCategories.value[0]?.id ?? 'special'
  }
})

const categoryItems = computed(() => {
  const cat = allCategories.value.find((c) => c.id === activeCategory.value)
  return cat?.items ?? []
})

function pick(item: ActionItem) {
  draft.value = item.value
}
</script>

<template>
  <div class="space-y-4">
    <UFormField label="Текущее значение">
      <UInput
        v-model="draft"
        placeholder="Например: Ctrl+C, Escape, macro:copyLine"
        class="w-full font-mono"
      />
    </UFormField>

    <div class="flex flex-wrap gap-1.5 border-b border-(--ui-border) pb-2">
      <UButton
        v-for="cat in allCategories"
        :key="cat.id"
        :icon="cat.icon"
        size="xs"
        :color="activeCategory === cat.id ? 'primary' : 'neutral'"
        :variant="activeCategory === cat.id ? 'soft' : 'ghost'"
        @click="activeCategory = cat.id"
      >
        {{ cat.label }}
        <UBadge
          v-if="cat.items.length"
          size="sm"
          variant="subtle"
          color="neutral"
          class="ml-1"
        >
          {{ cat.items.length }}
        </UBadge>
      </UButton>
    </div>

    <div
      v-if="categoryItems.length === 0"
      class="text-sm text-(--ui-text-muted) italic px-1 py-6 text-center"
    >
      В этой категории пока нет элементов.
    </div>
    <div
      v-else
      class="grid grid-cols-2 sm:grid-cols-3 md:grid-cols-4 gap-1.5 max-h-80 overflow-y-auto pr-1"
    >
      <button
        v-for="item in categoryItems"
        :key="item.value"
        type="button"
        class="text-left px-2.5 py-1.5 rounded-md border text-sm transition-colors hover:bg-(--ui-bg-elevated)"
        :class="draft === item.value
          ? 'border-(--ui-primary) bg-(--ui-primary)/10'
          : 'border-(--ui-border) bg-(--ui-bg)'"
        @click="pick(item)"
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
  </div>
</template>
