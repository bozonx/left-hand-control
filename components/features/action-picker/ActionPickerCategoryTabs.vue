<script setup lang="ts">
import type { StaticCategory } from '~/utils/actionCategories'

const props = defineProps<{
  activeCategory: string
  categories: StaticCategory[]
  keyOnly?: boolean
}>()

const emit = defineEmits<{
  'update:activeCategory': [value: string]
}>()

const catBarRef = ref<HTMLElement | null>(null)

function selectCategory(id: string) {
  emit('update:activeCategory', id)
}

function handleCategoryKeydown(event: KeyboardEvent, id: string) {
  if (!catBarRef.value) return
  const buttons = Array.from(catBarRef.value.querySelectorAll<HTMLElement>('button'))
  const idx = buttons.indexOf(event.currentTarget as HTMLElement)
  if (idx === -1) return

  if (event.key === 'Enter' || event.key === ' ') {
    event.preventDefault()
    selectCategory(id)
    return
  }

  let nextIdx = -1
  if (event.key === 'ArrowLeft') {
    nextIdx = idx - 1
  } else if (event.key === 'ArrowRight') {
    nextIdx = idx + 1
  } else if (event.key === 'Home') {
    nextIdx = 0
  } else if (event.key === 'End') {
    nextIdx = buttons.length - 1
  }

  if (nextIdx >= 0 && nextIdx < buttons.length) {
    event.preventDefault()
    const nextId = buttons[nextIdx]?.dataset.categoryId
    if (nextId) selectCategory(nextId)
    buttons[nextIdx]?.focus()
  }
}
</script>

<template>
  <div ref="catBarRef" role="tablist" class="flex flex-wrap gap-1.5 border-b border-(--ui-border) pb-2">
    <UButton
      v-if="!props.keyOnly"
      role="tab"
      data-category-id="text"
      :aria-selected="props.activeCategory === 'text'"
      :tabindex="props.activeCategory === 'text' ? 0 : -1"
      icon="i-lucide-text-cursor-input"
      size="xs"
      :color="props.activeCategory === 'text' ? 'primary' : 'neutral'"
      :variant="props.activeCategory === 'text' ? 'soft' : 'ghost'"
      @click="selectCategory('text')"
      @keydown="handleCategoryKeydown($event, 'text')"
    >
      {{ $t('categories.text') }}
    </UButton>
    <UButton
      v-for="cat in props.categories"
      :key="cat.id"
      role="tab"
      :data-category-id="cat.id"
      :aria-selected="props.activeCategory === cat.id"
      :tabindex="props.activeCategory === cat.id ? 0 : -1"
      :icon="cat.icon"
      size="xs"
      :color="props.activeCategory === cat.id ? 'primary' : 'neutral'"
      :variant="props.activeCategory === cat.id ? 'soft' : 'ghost'"
      @click="selectCategory(cat.id)"
      @keydown="handleCategoryKeydown($event, cat.id)"
    >
      {{ $t(cat.labelKey) }}
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
</template>
