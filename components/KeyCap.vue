<script setup lang="ts">
const props = defineProps<{
  label: string
  action?: string
}>()

defineEmits<{
  edit: []
}>()

const { displayAction } = useMacros()
const actionLabel = computed(() => displayAction(props.action))
</script>

<template>
  <div
    class="flex-1 basis-20 min-w-[4rem] max-w-[7rem] rounded-md border border-(--ui-border) bg-(--ui-bg) px-2 py-1.5 flex flex-col items-stretch gap-1 shadow-sm"
  >
    <div class="text-sm font-semibold text-center leading-none">
      {{ label }}
    </div>
    <div
      class="text-[10px] leading-tight text-center min-h-[1.75em] break-words"
      :class="action ? 'text-(--ui-primary)' : 'text-(--ui-text-muted) italic'"
    >
      {{ actionLabel || '—' }}
    </div>
    <UButton
      icon="i-lucide-pencil"
      size="xs"
      variant="ghost"
      color="neutral"
      block
      :aria-label="$t('keymap.editKeyAria', { label })"
      @click="$emit('edit')"
    />
  </div>
</template>
