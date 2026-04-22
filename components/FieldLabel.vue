<script setup lang="ts">
import AppTooltip from '~/components/shared/AppTooltip.vue'

defineProps<{
  label: string
  hint?: string
  required?: boolean
}>()
</script>

<template>
  <span class="inline-flex items-center gap-1.5">
    <span class="inline-flex items-center gap-1">
      <span class="text-xs text-(--ui-text-muted)">{{ label }}</span>
      <span v-if="required" class="text-(--ui-primary)">*</span>
    </span>
    <AppTooltip v-if="hint" :text="hint" align="start">
      <span class="inline-flex items-center justify-center cursor-help text-(--ui-text-muted)">
        <UIcon name="i-lucide-info" class="w-3.5 h-3.5" />
      </span>
    </AppTooltip>
  </span>
</template>
