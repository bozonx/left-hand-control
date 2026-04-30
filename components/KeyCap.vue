<script setup lang="ts">
const props = defineProps<{
  label: string
  action?: string | null
}>()

defineEmits<{
  edit: []
}>()

const { getActionInfo } = useMacros()
const { t } = useI18n()
const actionInfo = computed(() => getActionInfo(props.action))
const isSwallow = computed(() => props.action === null)
const actionLabel = computed(() =>
  isSwallow.value ? t('keymap.swallowLabel') : (actionInfo.value.label || '—'),
)
</script>

<template>
  <button
    type="button"
    class="min-w-0 rounded-md border border-(--ui-border) bg-(--ui-bg) px-2 py-1.5 flex flex-col items-stretch gap-0.5 text-left transition-colors hover:border-(--ui-border-accented) cursor-pointer"
    :aria-label="$t('keymap.editKeyAria', { label })"
    :title="actionLabel"
    @click="$emit('edit')"
  >
    <div class="text-[0.8125rem] font-semibold text-center leading-tight">
      {{ label }}
    </div>
    <div
      class="text-[10px] leading-tight text-center min-h-[1.5em] break-words flex items-center justify-center gap-1"
      :class="isSwallow || action ? 'text-(--ui-primary) font-medium' : 'text-(--ui-text-muted) italic'"
    >
      <UIcon v-if="actionInfo.icon" :name="actionInfo.icon" class="w-3 h-3 shrink-0" />
      <span class="truncate">{{ actionLabel }}</span>
    </div>
  </button>
</template>
