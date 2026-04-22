<script setup lang="ts">
const hintTooltipUi = {
  content: 'h-auto max-w-72 items-start py-2',
}

defineProps<{
  label: string
  hint?: string
}>()
</script>

<template>
  <span class="inline-flex items-center gap-1.5">
    <span>{{ label }}</span>
    <UTooltip v-if="hint" :ui="hintTooltipUi">
      <UIcon
        name="i-lucide-info"
        class="w-3.5 h-3.5 text-(--ui-text-muted) cursor-help"
      />
      <template #content>
        <div class="whitespace-pre-wrap text-left">{{ hint }}</div>
      </template>
    </UTooltip>
  </span>
</template>
