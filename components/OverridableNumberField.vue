<script setup lang="ts">
const props = defineProps<{
  defaultValue: number
  suffix?: string
  min?: number
  inputClass?: string
}>()

const model = defineModel<number | undefined>({ default: undefined })

const isOverridden = computed(() => model.value !== undefined)
const draft = ref('')

watch(
  model,
  (value) => {
    draft.value = value === undefined ? '' : String(value)
  },
  { immediate: true },
)

function startEdit() {
  model.value = props.defaultValue
}

function reset() {
  model.value = undefined
}

function updateDraft(value: string | number) {
  const next = String(value)
  draft.value = next
  if (next.trim() === '') return
  const parsed = Number(next)
  if (!Number.isFinite(parsed)) return
  model.value = Math.max(props.min ?? 0, Math.round(parsed))
}
</script>

<template>
  <div class="flex items-center gap-2 min-h-[32px]">
    <template v-if="!isOverridden">
      <span class="text-sm text-(--ui-text-muted) font-mono">
        {{ defaultValue }}<span v-if="suffix"> {{ suffix }}</span>
      </span>
      <UButton
        icon="i-lucide-pencil"
        size="xs"
        variant="ghost"
        color="neutral"
        square
        :aria-label="$t('common.override')"
        @click="startEdit"
      />
    </template>
    <template v-else>
      <UInput
        :model-value="draft"
        type="number"
        :min="min ?? 0"
        :class="inputClass ?? 'w-28'"
        @update:model-value="updateDraft"
        @blur="draft.trim() === '' && reset()"
      />
      <span v-if="suffix" class="text-xs text-(--ui-text-muted)">{{ suffix }}</span>
      <UButton
        icon="i-lucide-rotate-ccw"
        size="xs"
        variant="ghost"
        color="neutral"
        square
        :aria-label="$t('common.reset')"
        @click="reset"
      />
    </template>
  </div>
</template>
