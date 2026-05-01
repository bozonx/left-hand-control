<script setup lang="ts">
import AppTooltip from '~/components/shared/AppTooltip.vue'

const props = withDefaults(defineProps<{
  label: string
  disabled?: boolean
}>(), {
  disabled: false,
})

defineEmits<{
  click: []
}>()
</script>

<template>
  <AppTooltip :text="props.label" :disabled="props.disabled">
    <UButton
      icon="i-lucide-eraser"
      variant="ghost"
      color="neutral"
      size="xs"
      square
      :disabled="props.disabled"
      :aria-label="props.label"
      @mousedown.stop
      @click.stop="$emit('click')"
    />
  </AppTooltip>
</template>
