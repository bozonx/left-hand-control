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

function handleCategoryKeydown(event: KeyboardEvent) {
  if (!catBarRef.value) return
  const buttons = Array.from(catBarRef.value.querySelectorAll<HTMLElement>('button'))
  const idx = buttons.indexOf(event.currentTarget as HTMLElement)
  if (idx === -1) return

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
    buttons[nextIdx]?.focus()
  }
}
</script>

<template>
  <div ref="catBarRef" class="flex flex-wrap gap-1.5 border-b border-(--ui-border) pb-2">
    <UButton
      v-if="!props.keyOnly"
      icon="i-lucide-text-cursor-input"
      size="xs"
      :color="props.activeCategory === 'text' ? 'primary' : 'neutral'"
      :variant="props.activeCategory === 'text' ? 'soft' : 'ghost'"
      @click="selectCategory('text')"
      @keydown="handleCategoryKeydown"
    >
      {{ $t('categories.text') }}
    </UButton>
    <UButton
      v-for="cat in props.categories"
      :key="cat.id"
      :icon="cat.icon"
      size="xs"
      :color="props.activeCategory === cat.id ? 'primary' : 'neutral'"
      :variant="props.activeCategory === cat.id ? 'soft' : 'ghost'"
      @click="selectCategory(cat.id)"
      @keydown="handleCategoryKeydown"
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
