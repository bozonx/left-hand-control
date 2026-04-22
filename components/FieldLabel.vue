<script setup lang="ts">
defineProps<{
  label: string
  hint?: string
}>()
</script>

<template>
  <span class="inline-flex items-center gap-1.5">
    <span>{{ label }}</span>
    <AppTooltip v-if="hint" :text="hint" align="start">
      <UIcon
        name="i-lucide-info"
        class="w-3.5 h-3.5 text-(--ui-text-muted) cursor-help"
      />
    </AppTooltip>
  </span>
</template>
