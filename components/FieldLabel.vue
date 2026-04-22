<script setup lang="ts">
defineProps<{
  label: string
  hint?: string
}>()
</script>

<template>
  <span class="inline-flex items-center gap-1.5">
    <span>{{ label }}</span>
    <UTooltip v-if="hint">
      <UIcon
        name="i-lucide-info"
        class="w-3.5 h-3.5 text-(--ui-text-muted) cursor-help"
      />
      <template #content>
        <div class="max-w-72 whitespace-pre-wrap text-left p-1">{{ hint }}</div>
      </template>
    </UTooltip>
  </span>
</template>
