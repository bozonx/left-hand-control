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
  <button
    type="button"
    class="min-w-0 rounded-md border border-(--ui-border) bg-(--ui-bg) px-2 py-1.5 flex flex-col items-stretch gap-0.5 text-left transition-colors hover:border-(--ui-border-accented) cursor-pointer"
    :aria-label="$t('keymap.editKeyAria', { label })"
    @click="$emit('edit')"
  >
    <div class="text-[0.8125rem] font-semibold text-center leading-tight">
      {{ label }}
    </div>
    <div
      class="text-[10px] leading-tight text-center min-h-[1.5em] break-words"
      :class="action ? 'text-(--ui-primary) font-medium' : 'text-(--ui-text-muted) italic'"
    >
      {{ actionLabel || '—' }}
    </div>
  </button>
</template>
